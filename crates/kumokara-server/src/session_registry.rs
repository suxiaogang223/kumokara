//! Registry for tmux-owned terminal sessions.

use crate::output_history::OutputHistory;
use crate::process_discovery;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use kumokara_engine::tmux::{PaneSnapshot, Tmux, TmuxSession};
use kumokara_protocol::session::SessionInfo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::{broadcast, Mutex};

const OUTPUT_CHANNEL_CAPACITY: usize = 512;

/// Prefix for tmux session names created by Kumokara.
const TMUX_SESSION_PREFIX: &str = "kumokara_";

#[derive(Clone, Debug)]
pub(crate) struct TerminalChunk {
    pub session_id: String,
    pub seq: u64,
    pub data: Vec<u8>,
}

pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionEntry>>,
    tmux: Tmux,
}

struct SessionEntry {
    info: SessionInfo,
    session: TmuxSession,
    history: OutputHistory,
    output_tx: broadcast::Sender<TerminalChunk>,
    recovery_gap: bool,
}

pub(crate) struct SessionAttachment {
    pub replay: Vec<TerminalChunk>,
    pub live_from_seq: u64,
    pub gap_detected: bool,
    pub live: broadcast::Receiver<TerminalChunk>,
}

impl SessionRegistry {
    pub fn new(tmux: Tmux) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            tmux,
        }
    }

    /// Spawn a shell rooted in `cwd`. The registry owns the PTY, so a browser
    /// disconnect never terminates the process.
    ///
    /// Every shell is created in the required, detached tmux runtime so it can
    /// survive browser and server restarts.
    pub async fn create_shell_session(
        &self,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
    ) -> Result<SessionInfo> {
        validate_dimensions(cols, rows)?;
        let cwd = cwd
            .canonicalize()
            .with_context(|| format!("invalid working directory: {}", cwd.display()))?;
        if !cwd.is_dir() {
            bail!("working directory is not a directory: {}", cwd.display());
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let env = HashMap::from([("KUMOKARA_SESSION_ID".to_string(), session_id.clone())]);

        let now = Utc::now().to_rfc3339();
        let cwd_str = cwd.to_string_lossy().to_string();
        let name = tmux_session_name(&session_id);
        let mut session = TmuxSession::create(&self.tmux, name.clone(), cwd, cols, rows, env)?;
        if let Err(error) =
            persist_tmux_metadata(&self.tmux, &name, &session_id, &cwd_str, &now, cols, rows)
        {
            let _ = self.tmux.kill_session(&name);
            drop(session);
            return Err(error).context("failed to persist required tmux session metadata");
        }
        let initial_snapshot = capture_initial_snapshot(&self.tmux, &name).await;

        let mut output_rx = session
            .take_output_rx()
            .ok_or_else(|| anyhow::anyhow!("terminal output channel missing"))?;
        let history = OutputHistory::new();
        if let Some(snapshot) = initial_snapshot.filter(|snapshot| !snapshot.bytes.is_empty()) {
            // The control client has also queued the prompt bytes used to
            // produce this snapshot. Discard only those already-buffered
            // bytes so the initial screen is not replayed twice.
            while output_rx.try_recv().is_ok() {}
            history.push(&snapshot.bytes);
        }
        let output_history = history.clone();
        let (output_tx, _) = broadcast::channel(OUTPUT_CHANNEL_CAPACITY);
        let live_output = output_tx.clone();
        let output_session_id = session_id.clone();

        tokio::spawn(async move {
            while let Some(data) = output_rx.recv().await {
                let seq = output_history.push(&data);
                let _ = live_output.send(TerminalChunk {
                    session_id: output_session_id.clone(),
                    seq,
                    data,
                });
            }
        });

        let info = SessionInfo {
            id: session_id.clone(),
            title: directory_name(&cwd_str),
            cwd: cwd_str,
            agent: None,
            created_at: now.clone(),
            last_active_at: now,
            cols,
            rows,
        };
        self.sessions.lock().await.insert(
            session_id,
            SessionEntry {
                info: info.clone(),
                session,
                history,
                output_tx,
                recovery_gap: false,
            },
        );
        Ok(info)
    }

    /// Recover tmux sessions from a previous server run.
    ///
    /// Enumerates all tmux sessions, filters for those tagged with Kumokara
    /// metadata (`@kumokara_session_id`), and rebuilds `SessionEntry` for
    /// each. The visible pane is reconstructed as a replacement screen and
    /// every attachment receives an explicit output-history gap notification.
    pub async fn recover_sessions(&self) -> Result<Vec<SessionInfo>> {
        let tmux = self.tmux.clone();
        let tmux_sessions = tmux.list_sessions()?;
        let mut recovered = Vec::new();

        for name in tmux_sessions {
            // Only look at sessions with the Kumokara prefix
            if !name.starts_with(TMUX_SESSION_PREFIX) {
                // Also check for metadata on non-prefixed sessions
                // (in case user renamed or created externally)
                let has_meta = tmux.get_session_metadata(&name, "session_id")?;
                if has_meta.is_none() {
                    continue;
                }
            }

            let Some(session_id) = tmux.get_session_metadata(&name, "session_id")? else {
                continue;
            };

            // Read stored metadata
            let cwd = tmux
                .get_session_metadata(&name, "cwd")?
                .unwrap_or_else(|| "/".to_string());
            let created_at = tmux
                .get_session_metadata(&name, "created_at")?
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let cols: u16 = tmux
                .get_session_metadata(&name, "cols")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(100);
            let rows: u16 = tmux
                .get_session_metadata(&name, "rows")?
                .and_then(|v| v.parse().ok())
                .unwrap_or(30);

            let mut session = TmuxSession::attach(&tmux, name.clone(), cols, rows)
                .with_context(|| format!("failed to recover tmux session '{name}'"))?;

            let mut output_rx = session
                .take_output_rx()
                .ok_or_else(|| anyhow::anyhow!("recovered terminal output channel missing"))?;
            let history = OutputHistory::new();

            // Reconstruct the visible screen as a standalone ANSI snapshot.
            // Raw output sequence numbers and scrollback do not survive the
            // server process, so every recovered attachment reports a gap.
            match tmux.capture_visible_snapshot(&name) {
                Ok(snapshot) if !snapshot.bytes.is_empty() => {
                    history.push(&snapshot.bytes);
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%name, %error, "failed to capture recovered tmux screen");
                }
            }

            let output_history = history.clone();
            let (output_tx, _) = broadcast::channel(OUTPUT_CHANNEL_CAPACITY);
            let live_output = output_tx.clone();
            let output_session_id = session_id.clone();

            tokio::spawn(async move {
                while let Some(data) = output_rx.recv().await {
                    let seq = output_history.push(&data);
                    let _ = live_output.send(TerminalChunk {
                        session_id: output_session_id.clone(),
                        seq,
                        data,
                    });
                }
            });

            let now = Utc::now().to_rfc3339();
            let info = SessionInfo {
                id: session_id.clone(),
                title: directory_name(&cwd),
                cwd: cwd.clone(),
                agent: None,
                created_at,
                last_active_at: now,
                cols,
                rows,
            };
            self.sessions.lock().await.insert(
                session_id,
                SessionEntry {
                    info: info.clone(),
                    session,
                    history,
                    output_tx,
                    recovery_gap: true,
                },
            );

            tracing::info!(%name, session_id = %info.id, cwd = %info.cwd,
                "recovered tmux session");
            recovered.push(info);
        }

        Ok(recovered)
    }

    pub async fn list(&self) -> Vec<SessionInfo> {
        let roots = self
            .sessions
            .lock()
            .await
            .values()
            .filter_map(|entry| {
                entry
                    .session
                    .process_id()
                    .map(|pid| (entry.info.id.clone(), pid))
            })
            .collect::<Vec<_>>();
        let contexts = tokio::task::spawn_blocking(move || process_discovery::discover(&roots))
            .await
            .unwrap_or_default();

        let mut entries = self.sessions.lock().await;
        for context in contexts {
            if let Some(entry) = entries.get_mut(&context.session_id) {
                if let Some(cwd) = context.cwd {
                    let cwd = cwd.to_string_lossy().to_string();
                    if entry.info.cwd != cwd {
                        entry.info.cwd = cwd.clone();
                        let tmux_name = entry.session.name();
                        if let Err(error) = self.tmux.set_session_metadata(tmux_name, "cwd", &cwd) {
                            tracing::warn!(%tmux_name, %error, "failed to update tmux cwd metadata");
                        }
                    }
                }
                entry.info.agent = context.agent;
                entry.info.title = entry
                    .info
                    .agent
                    .as_ref()
                    .map(|agent| agent.provider.clone())
                    .unwrap_or_else(|| directory_name(&entry.info.cwd));
            }
        }

        let mut sessions = entries
            .values()
            .map(|entry| entry.info.clone())
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        sessions
    }

    pub async fn count(&self) -> usize {
        self.sessions.lock().await.len()
    }

    pub(crate) async fn attach(
        &self,
        session_id: &str,
        last_seq: Option<u64>,
    ) -> Result<SessionAttachment> {
        let sessions = self.sessions.lock().await;
        let entry = sessions
            .get(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;

        // Subscribe before taking the snapshot, then ignore already-replayed
        // live chunks. This closes the replay/live race without losing output.
        let live = entry.output_tx.subscribe();
        let replay_after = if entry.recovery_gap { None } else { last_seq };
        let (chunks, live_from_seq, history_gap) = entry.history.since(replay_after);
        let replay = chunks
            .into_iter()
            .map(|chunk| TerminalChunk {
                session_id: session_id.to_string(),
                seq: chunk.seq,
                data: chunk.data,
            })
            .collect();
        Ok(SessionAttachment {
            replay,
            live_from_seq,
            gap_detected: entry.recovery_gap || history_gap,
            live,
        })
    }

    pub async fn write_input(&self, session_id: &str, data: &[u8]) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        entry.session.write_input(data)?;
        entry.info.last_active_at = Utc::now().to_rfc3339();
        Ok(())
    }

    pub async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        validate_dimensions(cols, rows)?;
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        entry.session.resize(cols, rows)?;
        entry.info.cols = cols;
        entry.info.rows = rows;
        entry.info.last_active_at = Utc::now().to_rfc3339();
        let tmux_name = entry.session.name();
        if let Err(error) = self
            .tmux
            .set_session_metadata(tmux_name, "cols", &cols.to_string())
            .and_then(|_| {
                self.tmux
                    .set_session_metadata(tmux_name, "rows", &rows.to_string())
            })
        {
            tracing::warn!(%tmux_name, %error, "failed to update tmux dimensions metadata");
        }
        Ok(())
    }

    /// Remove a session by killing the tmux-owned process first, then dropping
    /// Kumokara's control connection.
    pub async fn remove(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.lock().await;
        if let Some(entry) = sessions.get(session_id) {
            let tmux_name = entry.session.name();
            if let Err(error) = self.tmux.kill_session(tmux_name) {
                tracing::warn!(%tmux_name, %error, "failed to kill tmux session during remove");
            }
        }
        sessions.remove(session_id).is_some()
    }
}

async fn capture_initial_snapshot(tmux: &Tmux, name: &str) -> Option<PaneSnapshot> {
    const MAX_ATTEMPTS: usize = 80;
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

    let mut previous = None;
    let mut last = None;
    for attempt in 0..MAX_ATTEMPTS {
        match tmux.capture_visible_snapshot(name) {
            Ok(snapshot) => {
                let stable = snapshot.content_present
                    && previous
                        .as_deref()
                        .is_some_and(|bytes| bytes == snapshot.bytes.as_slice());
                previous = Some(snapshot.bytes.clone());
                last = Some(snapshot);
                if stable {
                    break;
                }
            }
            Err(error) => {
                tracing::warn!(%name, %error, "failed to capture initial tmux screen");
                return None;
            }
        }
        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
    last
}

fn validate_dimensions(cols: u16, rows: u16) -> Result<()> {
    if cols == 0 || rows == 0 {
        bail!("terminal dimensions must be positive");
    }
    Ok(())
}

fn directory_name(cwd: &str) -> String {
    Path::new(cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("shell")
        .to_string()
}

fn tmux_session_name(session_id: &str) -> String {
    format!("{TMUX_SESSION_PREFIX}{session_id}")
}

fn persist_tmux_metadata(
    tmux: &Tmux,
    name: &str,
    session_id: &str,
    cwd: &str,
    created_at: &str,
    cols: u16,
    rows: u16,
) -> Result<()> {
    tmux.set_session_metadata(name, "session_id", session_id)?;
    tmux.set_session_metadata(name, "cwd", cwd)?;
    tmux.set_session_metadata(name, "created_at", created_at)?;
    tmux.set_session_metadata(name, "cols", &cols.to_string())?;
    tmux.set_session_metadata(name, "rows", &rows.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    struct TmuxTestServer(Tmux);

    impl Drop for TmuxTestServer {
        fn drop(&mut self) {
            let _ = self.0.kill_server();
        }
    }

    async fn wait_for_live_output(attachment: &mut SessionAttachment, expected: &str) {
        let mut observed = Vec::new();
        let result = timeout(Duration::from_secs(5), async {
            loop {
                let chunk = attachment.live.recv().await.unwrap();
                observed.extend_from_slice(&chunk.data);
                if String::from_utf8_lossy(&observed).contains(expected) {
                    break;
                }
            }
        })
        .await;
        if result.is_err() {
            panic!(
                "terminal output did not contain {expected:?}; observed {:?}",
                String::from_utf8_lossy(&observed)
            );
        }
    }

    #[tokio::test]
    async fn session_survives_creation_and_tracks_shell_cwd() {
        let tmux = isolated_tmux();
        let _server = TmuxTestServer(tmux.clone());
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new(tmux);
        let session = registry
            .create_shell_session(temp_dir.path().to_path_buf(), 80, 24)
            .await
            .unwrap();
        let mut attachment = registry.attach(&session.id, None).await.unwrap();

        registry
            .write_input(&session.id, b"printf '__kumokara_alive__\\n'\n")
            .await
            .unwrap();
        timeout(Duration::from_secs(5), async {
            loop {
                let chunk = attachment.live.recv().await.unwrap();
                if String::from_utf8_lossy(&chunk.data).contains("__kumokara_alive__") {
                    break;
                }
            }
        })
        .await
        .expect("shell process exited before accepting input");

        let nested = temp_dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        registry
            .write_input(&session.id, format!("cd {}\n", nested.display()).as_bytes())
            .await
            .unwrap();
        let expected_cwd = nested.canonicalize().unwrap().to_string_lossy().to_string();
        timeout(Duration::from_secs(5), async {
            loop {
                if registry
                    .list()
                    .await
                    .iter()
                    .any(|item| item.id == session.id && item.cwd == expected_cwd)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("session cwd did not follow the shell");

        assert!(registry.remove(&session.id).await);
    }

    #[tokio::test]
    async fn tmux_session_round_trips_input_and_survives_registry_restart() {
        let tmux = isolated_tmux();
        let _server = TmuxTestServer(tmux.clone());
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new(tmux.clone());
        let session = registry
            .create_shell_session(temp_dir.path().to_path_buf(), 80, 24)
            .await
            .unwrap();
        let tmux_name = tmux_session_name(&session.id);
        let mut attachment = registry.attach(&session.id, None).await.unwrap();

        assert!(attachment
            .replay
            .first()
            .is_some_and(|chunk| chunk.data.starts_with(b"\x1bc")));

        let command = "printf '__KUMOKARA_QUOTE__%s__UTF8__世界__ENV__%s\\n' 'hello world' \"$KUMOKARA_SESSION_ID\"\r";
        registry
            .write_input(&session.id, command.as_bytes())
            .await
            .unwrap();
        wait_for_live_output(
            &mut attachment,
            &format!(
                "__KUMOKARA_QUOTE__hello world__UTF8__世界__ENV__{}",
                session.id
            ),
        )
        .await;
        drop(attachment);
        drop(registry);

        assert!(tmux.session_exists(&tmux_name));

        let recovered_registry = SessionRegistry::new(tmux.clone());
        let recovered = recovered_registry.recover_sessions().await.unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, session.id);

        let mut recovered_attachment = recovered_registry
            .attach(&session.id, Some(u64::MAX - 1))
            .await
            .unwrap();
        assert!(recovered_attachment.gap_detected);
        assert!(recovered_attachment
            .replay
            .first()
            .is_some_and(|chunk| chunk.data.starts_with(b"\x1bc")));

        recovered_registry
            .write_input(&session.id, b"printf '__KUMOKARA_RECOVERED__\\n'\r")
            .await
            .unwrap();
        wait_for_live_output(&mut recovered_attachment, "__KUMOKARA_RECOVERED__").await;

        assert!(recovered_registry.remove(&session.id).await);
        assert!(!tmux.session_exists(&tmux_name));
    }

    fn isolated_tmux() -> Tmux {
        Tmux::new(format!("kumokara-test-{}", uuid::Uuid::new_v4().simple()))
    }
}

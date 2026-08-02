//! Process-owned terminal session runtime.

use crate::output_history::OutputHistory;
use crate::process_discovery;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use kumokara_engine::PtySession;
use kumokara_protocol::session::SessionInfo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::{broadcast, Mutex};

const OUTPUT_CHANNEL_CAPACITY: usize = 512;

#[derive(Clone, Debug)]
pub(crate) struct TerminalChunk {
    pub session_id: String,
    pub seq: u64,
    pub data: Vec<u8>,
}

pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, SessionEntry>>,
}

struct SessionEntry {
    info: SessionInfo,
    pty: PtySession,
    history: OutputHistory,
    output_tx: broadcast::Sender<TerminalChunk>,
}

pub(crate) struct SessionAttachment {
    pub replay: Vec<TerminalChunk>,
    pub live_from_seq: u64,
    pub gap_detected: bool,
    pub live: broadcast::Receiver<TerminalChunk>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Spawn a shell rooted in `cwd`. The registry owns the PTY, so a browser
    /// disconnect never terminates the process.
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
        let mut pty = PtySession::spawn(cwd.clone(), cols, rows, None, env).await?;
        let mut output_rx = pty
            .take_output_rx()
            .ok_or_else(|| anyhow::anyhow!("PTY output channel missing"))?;
        let history = OutputHistory::new();
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
        let cwd = cwd.to_string_lossy().to_string();
        let info = SessionInfo {
            id: session_id.clone(),
            title: directory_name(&cwd),
            cwd,
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
                pty,
                history,
                output_tx,
            },
        );
        Ok(info)
    }

    pub async fn list(&self) -> Vec<SessionInfo> {
        let roots = self
            .sessions
            .lock()
            .await
            .values()
            .filter_map(|entry| {
                entry
                    .pty
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
                    entry.info.cwd = cwd.to_string_lossy().to_string();
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
        let (chunks, live_from_seq, gap_detected) = entry.history.since(last_seq);
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
            gap_detected,
            live,
        })
    }

    pub async fn write_input(&self, session_id: &str, data: &[u8]) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        entry.pty.write_input(data)?;
        entry.info.last_active_at = Utc::now().to_rfc3339();
        Ok(())
    }

    pub async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        validate_dimensions(cols, rows)?;
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        entry.pty.resize(cols, rows)?;
        entry.info.cols = cols;
        entry.info.rows = rows;
        entry.info.last_active_at = Utc::now().to_rfc3339();
        Ok(())
    }

    /// Removing the entry drops the PTY and terminates its child process.
    pub async fn remove(&self, session_id: &str) -> bool {
        self.sessions.lock().await.remove(session_id).is_some()
    }
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

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn pty_survives_creation_and_tracks_shell_cwd() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new();
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
}

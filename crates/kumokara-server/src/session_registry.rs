//! Registry for process-owned terminal sessions.

use crate::output_history::OutputHistory;
use crate::process_discovery;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use kumokara_agent::AgentAdapterRegistry;
use kumokara_engine::PtySession;
use kumokara_protocol::session::{AgentInfo, AgentStatus, SessionInfo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
    agent_adapters: Arc<AgentAdapterRegistry>,
}

struct SessionEntry {
    info: SessionInfo,
    pty: PtySession,
    history: OutputHistory,
    output_tx: broadcast::Sender<TerminalChunk>,
    detected_agent: Option<AgentInfo>,
    detected_title: Option<String>,
    reported_agent: Option<AgentInfo>,
    agent_title: Option<String>,
    terminal_title: Option<String>,
}

pub(crate) struct AgentUpdate {
    pub code_agent: String,
    pub session_title: Option<String>,
    pub status: Option<AgentStatus>,
    pub detail: Option<String>,
    pub mode: Option<String>,
    pub task_progress: Option<String>,
}

pub(crate) struct SessionAttachment {
    pub replay: Vec<TerminalChunk>,
    pub live_from_seq: u64,
    pub gap_detected: bool,
    pub live: broadcast::Receiver<TerminalChunk>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::with_agent_adapters(AgentAdapterRegistry::with_builtins())
    }

    pub fn with_agent_adapters(agent_adapters: AgentAdapterRegistry) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            agent_adapters: Arc::new(agent_adapters),
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
        let mut pty = PtySession::spawn(cwd.clone(), cols, rows, None, env)?;
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
                detected_agent: None,
                detected_title: None,
                reported_agent: None,
                agent_title: None,
                terminal_title: None,
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
        let adapters = Arc::clone(&self.agent_adapters);
        let contexts =
            tokio::task::spawn_blocking(move || process_discovery::discover(&roots, &adapters))
                .await
                .unwrap_or_default();

        let mut entries = self.sessions.lock().await;
        for context in contexts {
            if let Some(entry) = entries.get_mut(&context.session_id) {
                if let Some(cwd) = context.cwd {
                    let cwd = cwd.to_string_lossy().to_string();
                    if entry.info.cwd != cwd {
                        entry.info.cwd = cwd;
                    }
                }
                entry.detected_agent = context.agent;
                entry.detected_title = context.title_hint;
                refresh_display(entry);
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

    pub async fn set_terminal_title(&self, session_id: &str, title: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        entry.terminal_title = sanitized_text(title, 160);
        refresh_display(entry);
        Ok(())
    }

    pub(crate) async fn apply_agent_update(
        &self,
        session_id: &str,
        update: AgentUpdate,
    ) -> Result<()> {
        let code_agent = sanitize_identifier(&update.code_agent)
            .ok_or_else(|| anyhow::anyhow!("invalid code agent identifier"))?;
        let presentation = self.agent_adapters.resolve(&code_agent);
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;

        let (provider, display_name, icon) = presentation
            .map(|agent| (agent.provider, agent.display_name, agent.icon))
            .unwrap_or_else(|| {
                (
                    code_agent.clone(),
                    humanize_identifier(&code_agent),
                    "◆".to_string(),
                )
            });
        entry.reported_agent = Some(AgentInfo {
            provider,
            display_name,
            icon,
            status: update.status,
            detail: update.detail.and_then(|value| sanitized_text(&value, 160)),
            mode: update.mode.and_then(|value| sanitized_text(&value, 80)),
            task_progress: update
                .task_progress
                .and_then(|value| sanitized_text(&value, 32)),
        });
        entry.agent_title = update
            .session_title
            .and_then(|value| sanitized_text(&value, 160));
        refresh_display(entry);
        Ok(())
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
        self.write_input_at_size(session_id, data, None).await
    }

    pub async fn write_input_at_size(
        &self,
        session_id: &str,
        data: &[u8],
        size: Option<(u16, u16)>,
    ) -> Result<()> {
        if let Some((cols, rows)) = size {
            validate_dimensions(cols, rows)?;
        }
        let mut sessions = self.sessions.lock().await;
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        if let Some((cols, rows)) = size {
            if entry.info.cols != cols || entry.info.rows != rows {
                // Resize and input share one ordered PTY command queue. This
                // makes the viewport that produced the input the temporary
                // controller without letting passive browser resizes win.
                entry.pty.resize(cols, rows)?;
                entry.info.cols = cols;
                entry.info.rows = rows;
            }
        }
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

fn refresh_display(entry: &mut SessionEntry) {
    let reported_is_live = entry
        .reported_agent
        .as_ref()
        .is_some_and(|agent| agent.status != Some(AgentStatus::Finished));
    entry.info.agent = if reported_is_live
        || entry
            .reported_agent
            .as_ref()
            .zip(entry.detected_agent.as_ref())
            .is_some_and(|(reported, detected)| reported.provider == detected.provider)
    {
        entry.reported_agent.clone()
    } else {
        entry.detected_agent.clone()
    };
    entry.info.title = entry
        .terminal_title
        .clone()
        .or_else(|| entry.agent_title.clone())
        .or_else(|| entry.detected_title.clone())
        .or_else(|| {
            entry
                .info
                .agent
                .as_ref()
                .map(|agent| agent.display_name.clone())
        })
        .unwrap_or_else(|| directory_name(&entry.info.cwd));
}

fn sanitized_text(value: &str, max_chars: usize) -> Option<String> {
    let value = value
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect::<String>();
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn sanitize_identifier(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    (!value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character)))
    .then_some(value)
}

fn humanize_identifier(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
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

    #[tokio::test]
    async fn pty_sets_terminal_capabilities_and_round_trips_bytes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new();
        let session = registry
            .create_shell_session(temp_dir.path().to_path_buf(), 80, 24)
            .await
            .unwrap();
        let mut attachment = registry.attach(&session.id, None).await.unwrap();

        let command = "printf '__KUMOKARA_QUOTE__%s__UTF8__世界__ENV__%s__TERM__%s__COLOR__%s__NO_COLOR__%s__CLICOLOR__%s\\n' 'hello world' \"$KUMOKARA_SESSION_ID\" \"$TERM\" \"$COLORTERM\" \"${NO_COLOR-unset}\" \"$CLICOLOR\"\r";
        registry
            .write_input_at_size(&session.id, command.as_bytes(), Some((96, 28)))
            .await
            .unwrap();
        wait_for_live_output(
            &mut attachment,
            &format!(
                "__KUMOKARA_QUOTE__hello world__UTF8__世界__ENV__{}__TERM__xterm-256color__COLOR__truecolor__NO_COLOR__unset__CLICOLOR__1",
                session.id
            ),
        )
        .await;
        let resized = registry
            .list()
            .await
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!((resized.cols, resized.rows), (96, 28));
        assert!(registry.remove(&session.id).await);
    }

    #[tokio::test]
    async fn agent_adapter_metadata_and_terminal_titles_follow_precedence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = SessionRegistry::new();
        let session = registry
            .create_shell_session(temp_dir.path().to_path_buf(), 80, 24)
            .await
            .unwrap();

        registry
            .apply_agent_update(
                &session.id,
                AgentUpdate {
                    code_agent: "claude".to_string(),
                    session_title: Some("Review the adapter API".to_string()),
                    status: Some(AgentStatus::Running),
                    detail: Some("thinking".to_string()),
                    mode: Some("opus".to_string()),
                    task_progress: Some("1/3".to_string()),
                },
            )
            .await
            .unwrap();

        let adapted = registry
            .list()
            .await
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        let agent = adapted.agent.unwrap();
        assert_eq!(adapted.title, "Review the adapter API");
        assert_eq!(agent.provider, "claude_code");
        assert_eq!(agent.display_name, "Claude Code");
        assert_eq!(agent.icon, "✦");
        assert_eq!(agent.status, Some(AgentStatus::Running));

        registry
            .set_terminal_title(&session.id, "OC | Product review\u{1b}\u{7}")
            .await
            .unwrap();
        let titled = registry
            .list()
            .await
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!(titled.title, "OC | Product review");

        registry.set_terminal_title(&session.id, "").await.unwrap();
        let fallback = registry
            .list()
            .await
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!(fallback.title, "Review the adapter API");
        assert!(registry.remove(&session.id).await);
    }
}

//! Session registry — manages active PTY sessions and their WebSocket connections.

use anyhow::Result;
use axum::extract::ws::Message;
use futures::stream::SplitSink;
use futures::SinkExt;
use kumokara_engine::PtySession;
use kumokara_protocol::messages::ServerMessage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

type WsSender = Arc<Mutex<SplitSink<axum::extract::ws::WebSocket, Message>>>;

/// Manages active PTY sessions and their WebSocket connections.
pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<String, SessionEntry>>>,
}

struct SessionEntry {
    /// Channel for sending input to the PTY
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Channel for sending resize events to the PTY
    resize_tx: mpsc::UnboundedSender<(u16, u16)>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new shell session — spawns a PTY and starts output forwarding to WS.
    pub async fn create_shell_session(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
        ws_sender: WsSender,
    ) -> Result<()> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        // Spawn the PTY
        let mut pty = PtySession::spawn(cwd, cols, rows, None).await?;

        // Extract I/O channels from PTY
        let input_tx = pty.input_sender().ok_or_else(|| anyhow::anyhow!("PTY input channel missing"))?;
        let resize_tx = pty.resize_sender().ok_or_else(|| anyhow::anyhow!("PTY resize channel missing"))?;
        let mut output_rx = pty.take_output_rx().ok_or_else(|| anyhow::anyhow!("PTY output channel missing"))?;

        // Spawn output forwarding task (owns the output receiver and WS sender)
        let sid = session_id.to_string();
        tokio::spawn(async move {
            while let Some(chunk) = output_rx.recv().await {
                let msg = ServerMessage::TerminalOutput {
                    session_id: sid.clone(),
                    seq: 0,
                    data: String::from_utf8_lossy(&chunk).to_string(),
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let mut sender = ws_sender.lock().await;
                    let _ = sender.send(Message::Text(json.into())).await;
                }
            }
        });

        let entry = SessionEntry {
            input_tx,
            resize_tx,
        };

        self.sessions
            .lock()
            .await
            .insert(session_id.to_string(), entry);

        tracing::info!("Session {} PTY spawned ({}x{})", session_id, cols, rows);
        Ok(())
    }

    /// Write input to a session's PTY.
    pub async fn write_input(&self, session_id: &str, data: &[u8]) -> Result<()> {
        let sessions = self.sessions.lock().await;
        if let Some(entry) = sessions.get(session_id) {
            entry.input_tx.send(data.to_vec()).map_err(|_| anyhow::anyhow!("Session input channel closed"))?;
        }
        Ok(())
    }

    /// Resize a session's PTY.
    pub async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        let sessions = self.sessions.lock().await;
        if let Some(entry) = sessions.get(session_id) {
            entry.resize_tx.send((cols, rows)).map_err(|_| anyhow::anyhow!("Session resize channel closed"))?;
        }
        Ok(())
    }

    /// Remove a session.
    pub async fn remove(&self, session_id: &str) {
        self.sessions.lock().await.remove(session_id);
        tracing::info!("Session {} removed", session_id);
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

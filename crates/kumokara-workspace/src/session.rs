//! Session lifecycle management.
//!
//! Sessions are PTY terminal sessions within a workspace.
//! They can be either shell (plain terminal) or agent (AI agent) sessions.

use anyhow::Result;
use chrono::Utc;
use kumokara_protocol::workspace::{AgentSessionInfo, SessionInfo};
use kumokara_event::OutputBuffer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages all sessions within a workspace.
pub struct SessionManager {
    /// Active sessions, keyed by session ID.
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
}

/// Runtime state for an active session.
pub struct SessionState {
    pub info: SessionInfo,
    pub output_buffer: OutputBuffer,
}

impl SessionManager {
    /// Create a new empty session manager.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session.
    pub async fn create_session(
        &self,
        workspace_id: &str,
        session_type: &str,
        title: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<SessionInfo> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let title = title.unwrap_or(match session_type {
            "agent" => "agent",
            _ => "shell",
        }).to_string();

        let agent = if session_type == "agent" {
            Some(AgentSessionInfo {
                provider: "claude_code".to_string(),
                cli_session_id: None,
                model: None,
            })
        } else {
            None
        };

        let info = SessionInfo {
            id: id.clone(),
            workspace_id: workspace_id.to_string(),
            session_type: session_type.to_string(),
            agent,
            title,
            state: "active".to_string(),
            created_at: now.clone(),
            last_active_at: now,
            cols,
            rows,
        };

        let state = SessionState {
            info: info.clone(),
            output_buffer: OutputBuffer::new(),
        };

        self.sessions.write().await.insert(id, state);

        Ok(info)
    }

    /// Get a session by ID.
    pub async fn get_session(&self, session_id: &str) -> Option<SessionInfo> {
        self.sessions
            .read()
            .await
            .get(session_id)
            .map(|s| s.info.clone())
    }

    /// Get the output buffer for a session.
    pub async fn get_output_buffer(&self, session_id: &str) -> Option<OutputBuffer> {
        self.sessions
            .read()
            .await
            .get(session_id)
            .map(|s| s.output_buffer.clone())
    }

    /// List all sessions in a workspace.
    pub async fn list_sessions(&self, workspace_id: &str) -> Vec<SessionInfo> {
        self.sessions
            .read()
            .await
            .values()
            .filter(|s| s.info.workspace_id == workspace_id)
            .map(|s| s.info.clone())
            .collect()
    }

    /// Remove a session.
    pub async fn destroy_session(&self, session_id: &str) -> Option<SessionInfo> {
        self.sessions
            .write()
            .await
            .remove(session_id)
            .map(|s| s.info)
    }

    /// Update session title.
    pub async fn update_title(&self, session_id: &str, title: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(state) = sessions.get_mut(session_id) {
            state.info.title = title.to_string();
            state.info.last_active_at = Utc::now().to_rfc3339();
        }
        Ok(())
    }

    /// Update session state.
    pub async fn update_state(&self, session_id: &str, state: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(s) = sessions.get_mut(session_id) {
            s.info.state = state.to_string();
            s.info.last_active_at = Utc::now().to_rfc3339();
        }
        Ok(())
    }

    /// Update terminal dimensions.
    pub async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(state) = sessions.get_mut(session_id) {
            state.info.cols = cols;
            state.info.rows = rows;
        }
        Ok(())
    }

    /// Get session count for a workspace.
    pub async fn session_count(&self, workspace_id: &str) -> usize {
        self.sessions
            .read()
            .await
            .values()
            .filter(|s| s.info.workspace_id == workspace_id)
            .count()
    }

    /// Check if workspace has any active agent sessions.
    pub async fn has_agent_running(&self, workspace_id: &str) -> bool {
        self.sessions
            .read()
            .await
            .values()
            .any(|s| s.info.workspace_id == workspace_id
                && s.info.session_type == "agent"
                && s.info.state == "active")
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

use serde::{Deserialize, Serialize};

/// Information about a workspace, used in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    /// Aggregated status: "ready" | "agent_running" | "agent_waiting" | "error"
    pub status: String,
    pub work_dir: String,
    pub created_at: String,
    pub updated_at: String,
    pub session_count: usize,
}

/// Information about a session within a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub workspace_id: String,
    /// "shell" | "agent"
    pub session_type: String,
    /// Agent metadata (only present for agent sessions)
    pub agent: Option<AgentSessionInfo>,
    /// Title — follows foreground process via OSC, can be renamed
    pub title: String,
    /// "active" | "background" | "exited"
    pub state: String,
    pub created_at: String,
    pub last_active_at: String,
    /// Terminal dimensions
    pub cols: u16,
    pub rows: u16,
}

/// Agent-specific session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionInfo {
    pub provider: String,
    pub cli_session_id: Option<String>,
    pub model: Option<String>,
}

/// Agent configuration for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub provider: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub permissions: Option<AgentPermissions>,
}

/// Permissions configuration (generates CLI-native permission config).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermissions {
    pub allow_shell: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: "claude_code".to_string(),
            model: None,
            system_prompt: None,
            permissions: Some(AgentPermissions {
                allow_shell: true,
            }),
        }
    }
}

/// Request to create a new workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub agent_config: Option<AgentConfig>,
}

/// Request to update a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub name: Option<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub agent_config: Option<AgentConfig>,
}

/// Screen dump returned on session attach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenDump {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub content: String,
    pub seq: u64,
    /// True if the client's last_seq was too old and history is incomplete.
    pub gap_detected: bool,
}

/// Attachment for agent prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data_base64: String,
}

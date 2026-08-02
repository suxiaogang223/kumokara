use serde::{Deserialize, Serialize};

/// The complete client-visible state of a process-owned terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub cwd: String,
    pub agent: Option<AgentInfo>,
    pub title: String,
    pub created_at: String,
    pub last_active_at: String,
    pub cols: u16,
    pub rows: u16,
}

/// Metadata inferred from processes running inside a terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub provider: String,
}

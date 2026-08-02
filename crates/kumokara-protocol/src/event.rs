use serde::{Deserialize, Serialize};

/// Kumokara event types — the structured layer of terminal observability.
///
/// These are persisted in SQLite and broadcast to connected clients.
/// Every event carries a `source` field indicating where the signal originated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    // --- Session lifecycle ---
    SessionCreated {
        session_id: String,
        workspace_id: String,
        session_type: String, // "shell" | "agent"
        title: String,
    },
    SessionDestroyed {
        session_id: String,
    },

    // --- Command boundaries (from OSC 133) ---
    CommandStarted {
        session_id: String,
        command: String,
        cwd: Option<String>,
    },
    CommandFinished {
        session_id: String,
        exit_code: i32,
        duration_ms: u64,
    },

    // --- CWD tracking (from OSC 7) ---
    CwdChanged {
        session_id: String,
        path: String,
    },

    // --- Agent events ---
    AgentStarted {
        session_id: String,
        provider: String,
        model: Option<String>,
        cli_session_id: String,
    },
    AgentStateChanged {
        session_id: String,
        state: String, // "processing" | "idle" | "awaiting_input"
    },
    AgentTask {
        session_id: String,
        task_id: String,
        prompt: String,
    },
    AgentApproval {
        session_id: String,
        action_id: String,
        description: String,
        decision: Option<String>,
    },
    AgentCompleted {
        session_id: String,
        task_id: Option<String>,
        summary: Option<String>,
    },
    AgentError {
        session_id: String,
        task_id: Option<String>,
        error: String,
    },

    // --- Catch-all workspace event ---
    WorkspaceEvent {
        workspace_id: String,
        description: String,
    },
}

/// Source of an event signal. Used for debugging state mismatches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// From OSC sequence parsing (command boundaries, exit codes, cwd)
    Shellint,
    /// From Agent CLI hook callbacks (state changes, task events)
    AgentHook,
    /// From Kumokara server's own state derivation
    Server,
}

impl std::fmt::Display for EventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSource::Shellint => write!(f, "shellint"),
            EventSource::AgentHook => write!(f, "agent_hook"),
            EventSource::Server => write!(f, "server"),
        }
    }
}

/// A persisted event entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    /// Monotonic sequence number within the workspace
    pub seq: i64,
    /// When the event occurred (UTC)
    pub timestamp: String,
    /// Which session this event belongs to (if any)
    pub session_id: Option<String>,
    /// Which workspace this event belongs to
    pub workspace_id: String,
    /// Where the signal came from
    pub source: String,
    /// The event payload
    pub event: Event,
}

/// Output chunk from a session's raw terminal output buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputChunk {
    pub session_id: String,
    pub seq: u64,
    pub data: Vec<u8>,
}

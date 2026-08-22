//! JSON control messages exchanged over the WebSocket connection.
//!
//! Terminal I/O uses binary frames with a 24-byte header: a 16-byte session
//! UUID, an 8-byte big-endian sequence field, then raw terminal bytes.

use crate::session::{AgentStatus, SessionInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Auth {
        token: String,
    },
    SessionCreate {
        request_id: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default = "default_cols")]
        cols: u16,
        #[serde(default = "default_rows")]
        rows: u16,
    },
    SessionList {
        request_id: String,
    },
    DirectoryList {
        request_id: String,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        show_hidden: bool,
    },
    DirectoryCreate {
        request_id: String,
        parent: String,
        name: String,
    },
    SessionAttach {
        request_id: String,
        session_id: String,
        #[serde(default)]
        last_seq: Option<u64>,
    },
    SessionDetach {
        session_id: String,
    },
    SessionDestroy {
        request_id: String,
        session_id: String,
    },
    TerminalResize {
        session_id: String,
        cols: u16,
        rows: u16,
        active: bool,
    },
    TerminalTitle {
        session_id: String,
        title: String,
    },
    AgentUpdate {
        session_id: String,
        code_agent: String,
        #[serde(default)]
        session_title: Option<String>,
        #[serde(default)]
        status: Option<AgentStatus>,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        task_progress: Option<String>,
    },
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    AuthOk {
        server_version: String,
    },
    AuthError {
        code: String,
        message: String,
    },
    SessionCreated {
        request_id: String,
        session: Box<SessionInfo>,
    },
    SessionList {
        request_id: String,
        sessions: Vec<SessionInfo>,
    },
    DirectoryListing {
        request_id: String,
        home: String,
        path: String,
        parent: Option<String>,
        entries: Vec<DirectoryEntry>,
    },
    DirectoryCreated {
        request_id: String,
        path: String,
    },
    SessionDestroyed {
        session_id: String,
    },
    ServerNotification {
        message: String,
    },
    Error {
        request_id: Option<String>,
        code: String,
        message: String,
    },
}

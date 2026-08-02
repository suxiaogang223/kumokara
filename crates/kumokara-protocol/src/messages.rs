//! JSON control messages exchanged over the WebSocket connection.
//!
//! Terminal input may also use binary frames with a 24-byte header:
//! a 16-byte session UUID, an 8-byte sequence number, then raw input bytes.

use crate::session::SessionInfo;
use serde::{Deserialize, Serialize};

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
    TerminalInput {
        session_id: String,
        data_base64: String,
    },
    TerminalResize {
        session_id: String,
        cols: u16,
        rows: u16,
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
        session: SessionInfo,
    },
    SessionList {
        request_id: String,
        sessions: Vec<SessionInfo>,
    },
    TerminalOutput {
        session_id: String,
        seq: u64,
        data_base64: String,
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

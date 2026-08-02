//! WebSocket message types for the Kumokara protocol.
//!
//! All control messages use JSON-encoded text frames.
//! Terminal I/O uses binary frames with a 24-byte fixed header:
//!   - 16 bytes: session_id UUID (big-endian)
//!   - 8 bytes: seq u64 (big-endian)
//!   - remainder: raw terminal output bytes

use serde::{Deserialize, Serialize};
use crate::workspace::*;
use crate::event::*;

// ============================================================================
// Client → Server messages
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    // --- Auth ---
    Auth {
        token: String,
    },

    // --- Workspace ---
    CreateWorkspace {
        request_id: String,
        name: String,
        #[serde(default)]
        env: Option<std::collections::HashMap<String, String>>,
        #[serde(default)]
        agent_config: Option<AgentConfig>,
    },
    ListWorkspaces {
        request_id: String,
    },
    GetWorkspace {
        request_id: String,
        workspace_id: String,
    },
    DestroyWorkspace {
        request_id: String,
        workspace_id: String,
    },
    UpdateWorkspace {
        request_id: String,
        workspace_id: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        env: Option<std::collections::HashMap<String, String>>,
        #[serde(default)]
        agent_config: Option<AgentConfig>,
    },

    // --- Session ---
    SessionCreate {
        request_id: String,
        workspace_id: String,
        #[serde(rename = "session_type")]
        session_type: String, // "shell" | "agent"
        #[serde(default = "default_cols")]
        cols: u16,
        #[serde(default = "default_rows")]
        rows: u16,
    },
    SessionList {
        request_id: String,
        workspace_id: String,
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
        /// Raw input bytes (base64-encoded for JSON; use binary frames in production)
        data: String,
    },
    TerminalResize {
        session_id: String,
        cols: u16,
        rows: u16,
    },

    // --- Agent ---
    AgentStart {
        request_id: String,
        workspace_id: String,
        #[serde(default)]
        provider: Option<String>,
    },
    AgentStop {
        request_id: String,
        session_id: String,
    },
    AgentSendPrompt {
        request_id: String,
        session_id: String,
        prompt: String,
        #[serde(default)]
        attachments: Option<Vec<Attachment>>,
    },
    AgentApprove {
        request_id: String,
        session_id: String,
        action_id: String,
    },
    AgentReject {
        request_id: String,
        session_id: String,
        action_id: String,
        #[serde(default)]
        reason: Option<String>,
    },
    AgentCancelTask {
        request_id: String,
        session_id: String,
        task_id: String,
    },

    // --- Event Stream ---
    EventSubscribe {
        workspace_id: String,
    },
    EventUnsubscribe {
        workspace_id: String,
    },
    EventQuery {
        request_id: String,
        workspace_id: String,
        #[serde(default)]
        after_seq: Option<i64>,
        #[serde(default)]
        limit: Option<i64>,
        #[serde(default)]
        types: Option<Vec<String>>,
    },
}

fn default_cols() -> u16 { 80 }
fn default_rows() -> u16 { 24 }

// ============================================================================
// Server → Client messages
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    // --- Auth ---
    AuthOk {
        server_version: String,
    },
    AuthError {
        code: String,
        message: String,
    },

    // --- Workspace ---
    WorkspaceCreated {
        request_id: String,
        workspace: WorkspaceInfo,
    },
    WorkspaceList {
        request_id: String,
        workspaces: Vec<WorkspaceInfo>,
    },
    WorkspaceUpdated {
        workspace_id: String,
        workspace: WorkspaceInfo,
    },
    WorkspaceDestroyed {
        workspace_id: String,
    },
    WorkspaceError {
        request_id: Option<String>,
        code: String,
        message: String,
    },

    // --- Session ---
    SessionCreated {
        request_id: String,
        workspace_id: String,
        session: SessionInfo,
    },
    SessionList {
        request_id: String,
        sessions: Vec<SessionInfo>,
    },
    ScreenDump {
        session_id: String,
        cols: u16,
        rows: u16,
        cursor_x: u16,
        cursor_y: u16,
        content: String,
        seq: u64,
        gap_detected: bool,
    },
    SessionDestroyed {
        session_id: String,
    },

    // --- Terminal Output (text frame for compatibility; prefer binary frames) ---
    TerminalOutput {
        session_id: String,
        seq: u64,
        data: String,
    },

    // --- Agent ---
    AgentStatus {
        session_id: String,
        status: String,
        #[serde(default)]
        current_task: Option<String>,
        #[serde(default)]
        queue_length: usize,
    },
    AgentProgress {
        session_id: String,
        task_id: String,
        step: Option<String>,
        output: Option<String>,
    },
    AgentApprovalNeeded {
        session_id: String,
        action_id: String,
        description: String,
        risk_level: String,
        context: Option<String>,
    },
    AgentTaskCompleted {
        session_id: String,
        task_id: String,
        summary: Option<String>,
    },
    AgentTaskFailed {
        session_id: String,
        task_id: String,
        error: String,
    },
    AgentMessage {
        session_id: String,
        role: String,
        content: String,
    },

    // --- Events ---
    EventBatch {
        request_id: String,
        workspace_id: String,
        events: Vec<EventEntry>,
    },
    EventLive {
        workspace_id: String,
        event: EventEntry,
    },

    // --- Server notifications ---
    ServerNotification {
        message: String,
    },
    Error {
        request_id: Option<String>,
        code: String,
        message: String,
    },
}

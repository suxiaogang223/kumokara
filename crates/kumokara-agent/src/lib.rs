//! kumokara-agent — Agent provider abstraction and lifecycle management.
//!
//! Phase 0: defines the `AgentProvider` trait and provides stub implementations.
//! Phase 1+: real Claude Code / Codex / OpenCode providers.

pub mod providers;

use anyhow::Result;
use async_trait::async_trait;
use kumokara_protocol::workspace::WorkspaceInfo;
use std::path::PathBuf;

/// Mode for launching an agent.
#[derive(Debug, Clone)]
pub enum LaunchMode {
    /// Fresh start
    Start,
    /// Resume an existing session by CLI session ID
    Resume { cli_session_id: String },
    /// Fork an existing session
    Fork { cli_session_id: String },
}

/// The trait each Agent provider must implement.
///
/// Following Otty's architecture: Kumokara manages the process and
/// receives state reports via hooks; the provider trait handles
/// detection, integration installation, and CLI command construction.
#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Provider identifier: "claude_code" | "codex" | "opencode".
    fn name(&self) -> &str;

    /// Check if this agent CLI is installed and available on the system.
    async fn check_available(&self) -> Result<bool>;

    /// Install Kumokara's state-reporting integration into the agent's
    /// own config (hooks / plugin). Only touches Kumokara-owned entries.
    async fn install_integration(&self, workspace: &WorkspaceInfo) -> Result<()>;

    /// Remove Kumokara's entries, leaving the rest of the config untouched.
    async fn uninstall_integration(&self, workspace: &WorkspaceInfo) -> Result<()>;

    /// Build the launch command for start / resume / fork.
    fn launch_command(&self, mode: LaunchMode) -> Vec<String>;

    /// Locate the agent's transcript/session files (for History & audit).
    fn transcript_paths(&self, workspace: &WorkspaceInfo) -> Result<Vec<PathBuf>>;
}

/// Get a provider by name.
pub fn get_provider(name: &str) -> Option<Box<dyn AgentProvider>> {
    match name {
        "claude_code" | "claude" => Some(Box::new(providers::claude::ClaudeCodeProvider::new())),
        _ => None,
    }
}

/// List all available provider names.
pub fn provider_names() -> Vec<&'static str> {
    vec!["claude_code"]
}

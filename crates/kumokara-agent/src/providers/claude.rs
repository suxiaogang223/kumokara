//! Claude Code provider — stub implementation for Phase 0.
//!
//! In Phase 1, this will have full integration: hooks, resume/fork,
//! state reporting, and transcript parsing.

use anyhow::Result;
use async_trait::async_trait;
use kumokara_protocol::workspace::WorkspaceInfo;
use std::path::PathBuf;
use std::process::Command;

use crate::{AgentProvider, LaunchMode};

pub struct ClaudeCodeProvider;

impl ClaudeCodeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeCodeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentProvider for ClaudeCodeProvider {
    fn name(&self) -> &str {
        "claude_code"
    }

    async fn check_available(&self) -> Result<bool> {
        Ok(Command::new("which")
            .arg("claude")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false))
    }

    async fn install_integration(&self, _workspace: &WorkspaceInfo) -> Result<()> {
        // Phase 1: write hooks to ~/.claude/settings.json
        tracing::info!("Claude Code integration not yet implemented (Phase 1)");
        Ok(())
    }

    async fn uninstall_integration(&self, _workspace: &WorkspaceInfo) -> Result<()> {
        tracing::info!("Claude Code integration removal not yet implemented (Phase 1)");
        Ok(())
    }

    fn launch_command(&self, mode: LaunchMode) -> Vec<String> {
        let mut cmd = vec!["claude".to_string()];
        match mode {
            LaunchMode::Start => {
                // Fresh start — no extra flags
            }
            LaunchMode::Resume { cli_session_id } => {
                cmd.push("--resume".to_string());
                cmd.push(cli_session_id);
            }
            LaunchMode::Fork { cli_session_id } => {
                cmd.push("--resume".to_string());
                cmd.push(cli_session_id);
                cmd.push("--fork-session".to_string());
            }
        }
        cmd
    }

    fn transcript_paths(&self, _workspace: &WorkspaceInfo) -> Result<Vec<PathBuf>> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let projects_dir = home.join(".claude").join("projects");
        // Phase 1: enumerate actual project transcript files
        Ok(vec![projects_dir])
    }
}

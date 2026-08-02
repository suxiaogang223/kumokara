//! Per-workspace filesystem layout management.
//!
//! Creates and manages the directory structure:
//! ```text
//! ~/.kumokara/
//! ├── config.yaml
//! ├── kumokara.db
//! ├── workspaces/
//! │   └── {workspace_id}/
//! │       ├── workspace.yaml
//! │       ├── files/          # working directory
//! │       ├── events.db       # structured event log
//! │       └── sessions/
//! │           └── {session_id}.log  # raw output buffer
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Manages the Kumokara data directory and per-workspace filesystem layout.
pub struct WorkspaceFilesystem {
    /// Root data directory (~/.kumokara/)
    root_dir: PathBuf,
}

impl WorkspaceFilesystem {
    /// Create a new filesystem manager rooted at the default location.
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let root_dir = home.join(".kumokara");
        Ok(Self { root_dir })
    }

    /// Create a new filesystem manager with a custom root directory (for testing).
    pub fn with_root(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    /// Get the root data directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Ensure all required directories exist.
    pub fn initialize(&self) -> Result<()> {
        let dirs = vec![
            self.root_dir.clone(),
            self.root_dir.join("workspaces"),
            self.root_dir.join("shellint"),
            self.root_dir.join("auth"),
        ];

        for dir in dirs {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }

        Ok(())
    }

    /// Get the path for a specific workspace.
    pub fn workspace_dir(&self, workspace_id: &str) -> PathBuf {
        self.root_dir.join("workspaces").join(workspace_id)
    }

    /// Get the working directory (files/) for a workspace.
    pub fn work_dir(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("files")
    }

    /// Get the config file path for a workspace.
    pub fn workspace_config_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("workspace.yaml")
    }

    /// Get the event database path for a workspace.
    pub fn events_db_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("events.db")
    }

    /// Get the session log path.
    pub fn session_log_path(&self, workspace_id: &str, session_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id)
            .join("sessions")
            .join(format!("{}.log", session_id))
    }

    /// Create the directory structure for a new workspace.
    pub fn create_workspace_dirs(&self, workspace_id: &str) -> Result<()> {
        let dirs = vec![
            self.workspace_dir(workspace_id),
            self.work_dir(workspace_id),
            self.workspace_dir(workspace_id).join("sessions"),
        ];

        for dir in dirs {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }

        Ok(())
    }

    /// Remove all files for a workspace (irreversible).
    pub fn destroy_workspace(&self, workspace_id: &str) -> Result<()> {
        let dir = self.workspace_dir(workspace_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .with_context(|| format!("Failed to remove workspace directory: {}", dir.display()))?;
        }
        Ok(())
    }

    /// Get the global config file path.
    pub fn global_config_path(&self) -> PathBuf {
        self.root_dir.join("config.yaml")
    }

    /// Get the global database path.
    pub fn global_db_path(&self) -> PathBuf {
        self.root_dir.join("kumokara.db")
    }
}

impl Default for WorkspaceFilesystem {
    fn default() -> Self {
        Self::new().expect("Failed to create WorkspaceFilesystem")
    }
}

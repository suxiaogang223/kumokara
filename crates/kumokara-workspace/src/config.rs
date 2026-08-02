//! Workspace configuration persistence (YAML).

use anyhow::Result;
use kumokara_protocol::workspace::AgentConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Workspace configuration stored in workspace.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Schema version for migration compatibility
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// Workspace name
    pub name: String,

    /// Environment variables (0600 permission enforced on write)
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Agent configuration
    #[serde(default)]
    pub agent_config: Option<AgentConfig>,

    /// Auto-start agent on server boot
    #[serde(default)]
    pub auto_start: bool,

    /// Creation timestamp (UTC)
    #[serde(default)]
    pub created_at: Option<String>,
}

fn default_schema_version() -> u32 {
    1
}

impl WorkspaceConfig {
    /// Create a new workspace config with defaults.
    pub fn new(name: String, env: Option<HashMap<String, String>>, agent_config: Option<AgentConfig>) -> Self {
        Self {
            schema_version: 1,
            name,
            env: env.unwrap_or_default(),
            agent_config,
            auto_start: false,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    /// Read config from a YAML file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    /// Write config to a YAML file with restricted permissions.
    pub fn to_file(&self, path: &Path) -> Result<()> {
        let contents = serde_yaml::to_string(self)?;
        std::fs::write(path, &contents)?;

        // Set file permissions to 0600 (env may contain secrets)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    /// Update the name.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Update environment variables.
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env = env;
    }

    /// Update agent config.
    pub fn set_agent_config(&mut self, config: AgentConfig) {
        self.agent_config = Some(config);
    }
}

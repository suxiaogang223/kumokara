//! Environment variable management for workspaces.
//!
//! Each workspace has its own set of environment variables (API keys,
//! database URLs, etc.) stored with 0600 file permissions.

use std::collections::HashMap;

/// Manages environment variables for a workspace.
#[derive(Clone)]
pub struct EnvManager {
    env: HashMap<String, String>,
}

impl EnvManager {
    /// Create a new EnvManager with the given environment variables.
    pub fn new(env: HashMap<String, String>) -> Self {
        Self { env }
    }

    /// Create an empty EnvManager.
    pub fn empty() -> Self {
        Self {
            env: HashMap::new(),
        }
    }

    /// Get all environment variables.
    pub fn all(&self) -> &HashMap<String, String> {
        &self.env
    }

    /// Get a specific environment variable.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.env.get(key)
    }

    /// Set an environment variable.
    pub fn set(&mut self, key: String, value: String) {
        self.env.insert(key, value);
    }

    /// Remove an environment variable.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.env.remove(key)
    }

    /// Merge additional environment variables.
    pub fn merge(&mut self, other: HashMap<String, String>) {
        self.env.extend(other);
    }
}

impl Default for EnvManager {
    fn default() -> Self {
        Self::empty()
    }
}

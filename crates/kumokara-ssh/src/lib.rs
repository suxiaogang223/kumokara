//! kumokara-ssh — SSH remote connection management.
//!
//! Phase 0: stub crate. Full russh-based implementation in Phase 3.

/// Placeholder for SSH connector.
/// Phase 3 will implement SSH connection management using russh.
pub struct SshConnector;

impl SshConnector {
    /// Create a new SSH connector (stub).
    pub fn new() -> Self {
        tracing::info!("SSH connector is a stub in Phase 0");
        Self
    }
}

impl Default for SshConnector {
    fn default() -> Self {
        Self::new()
    }
}

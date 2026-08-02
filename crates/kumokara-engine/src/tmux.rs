//! tmux detection and control mode integration.
//!
//! This module currently provides capability detection only. A future
//! control-mode backend can add recovery without changing the session model.

use std::process::Command;

/// Check if tmux is available on the system PATH.
pub fn has_tmux() -> bool {
    detect_tmux().is_some()
}

/// Detect tmux and return its version string if available.
///
/// Returns `Some("<version>")` if tmux is found, `None` otherwise.
pub fn detect_tmux() -> Option<String> {
    let output = Command::new("tmux").arg("-V").output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

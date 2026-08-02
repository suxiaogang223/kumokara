//! tmux detection and control mode integration.
//!
//! In Phase 0, this module provides detection only.
//! Phase 1+ will add control mode connection and session management.

use std::process::Command;

/// Check if tmux is available on the system PATH.
pub fn has_tmux() -> bool {
    detect_tmux().is_some()
}

/// Detect tmux and return its version string if available.
///
/// Returns `Some("<version>")` if tmux is found, `None` otherwise.
pub fn detect_tmux() -> Option<String> {
    match Command::new("which").arg("tmux").output() {
        Ok(output) if output.status.success() => {
            // Now get the version
            match Command::new("tmux").arg("-V").output() {
                Ok(version_output) if version_output.status.success() => {
                    let version = String::from_utf8_lossy(&version_output.stdout)
                        .trim()
                        .to_string();
                    Some(version)
                }
                _ => {
                    // tmux exists but couldn't get version — still usable
                    Some("unknown version".to_string())
                }
            }
        }
        _ => None,
    }
}

/// Parse a tmux version string like "tmux 3.5" into major.minor components.
///
/// Handles various formats: "tmux 3.5", "tmux 3.5a", "tmux-3.4", "next-3.5".
pub fn parse_tmux_version(version: &str) -> Option<(u32, u32)> {
    // Strip known prefixes
    let version = version
        .trim_start_matches("tmux ")
        .trim_start_matches("tmux-");

    // Extract only digits and dots
    let version: String = version
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .collect();

    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse().ok()?;
        // Strip trailing non-numeric characters from minor version (e.g., "5a" → "5")
        let minor_str: String = parts[1].chars().take_while(|c| c.is_ascii_digit()).collect();
        let minor = minor_str.parse().ok()?;
        Some((major, minor))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tmux_version() {
        assert_eq!(parse_tmux_version("tmux 3.5"), Some((3, 5)));
        assert_eq!(parse_tmux_version("tmux 3.5a"), Some((3, 5)));
        assert_eq!(parse_tmux_version("tmux-3.4"), Some((3, 4)));
        assert_eq!(parse_tmux_version("next-3.5"), Some((3, 5)));
        assert_eq!(parse_tmux_version("garbage"), None);
    }
}

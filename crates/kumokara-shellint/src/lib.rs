//! kumokara-shellint — Shell integration for terminal observability.
//!
//! Provides OSC 133/7 sequence parsing and shell integration scripts that
//! inject these sequences into zsh, bash, and fish sessions.

pub mod inject;
pub mod parse;

/// Check if shell integration is disabled via environment variable.
pub fn is_disabled() -> bool {
    std::env::var("KUMOKARA_DISABLE_INTEGRATION")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

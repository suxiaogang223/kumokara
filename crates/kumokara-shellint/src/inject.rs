//! Shell integration injection into PTY startup environment.
//!
//! These scripts emit OSC 133/7 sequences that the server parses into
//! structured events — the data foundation for badges, outlines, and notifications.
//!
//! Phase 0: scripts are provided as embedded strings; injection into PTY
//! startup is hooked up in Phase 1.

/// Return the shell integration script for the given shell.
pub fn get_integration_script(shell: &str) -> Option<&'static str> {
    match shell {
        "zsh" => Some(include_str!("scripts/zsh.sh")),
        "bash" => Some(include_str!("scripts/bash.sh")),
        "fish" => Some(include_str!("scripts/fish.fish")),
        _ => None,
    }
}

/// Detect the user's shell from the SHELL environment variable.
pub fn detect_shell() -> String {
    std::env::var("SHELL")
        .unwrap_or_else(|_| "/bin/sh".to_string())
        .split('/')
        .last()
        .unwrap_or("sh")
        .to_string()
}

/// Build the environment variables to inject for shell integration.
pub fn integration_env() -> Vec<(String, String)> {
    vec![
        ("KUMOKARA_INTEGRATION".to_string(), "1".to_string()),
    ]
}

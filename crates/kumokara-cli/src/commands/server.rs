//! Server daemon mode — for Remote deployment.
//!
//! `kumokara server` starts the server without opening a browser.
//! Designed for VPS / home server deployments.

use anyhow::Result;
use kumokara_server::{serve, AppState};
use std::net::SocketAddr;

/// Run Kumokara in server (daemon) mode.
pub async fn run_server(bind: &str, require_token: bool) -> Result<()> {
    // Detect tmux
    match kumokara_engine::detect_tmux() {
        Some(version) => {
            tracing::info!("{version} detected (restart recovery backend: planned)");
        }
        None => {
            tracing::warn!("tmux not found — session recovery disabled");
        }
    }

    let state = AppState::new(super::configure_auth(require_token));

    let addr: SocketAddr = bind.parse()?;
    tracing::info!("Starting server on {}", addr);

    serve(addr, state).await?;

    Ok(())
}

//! Server daemon mode — for Remote deployment.
//!
//! `kumokara server` starts the server without opening a browser.
//! Designed for VPS / home server deployments.

use anyhow::Result;
use kumokara_server::{serve, AppState};
use std::net::SocketAddr;

/// Run Kumokara in server (daemon) mode.
pub async fn run_server(bind: &str, require_token: bool) -> Result<()> {
    let state = AppState::new(super::configure_auth(require_token));
    tracing::info!("PTY sessions ready; they persist while the server is running");

    let addr: SocketAddr = bind.parse()?;
    tracing::info!("Starting server on {}", addr);

    serve(addr, state).await?;

    Ok(())
}

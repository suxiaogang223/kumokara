//! Server daemon mode — for Remote deployment.
//!
//! `kumokara server` starts the server without opening a browser.
//! Designed for VPS / home server deployments.

use anyhow::Result;
use kumokara_auth::AuthManager;
use kumokara_server::{serve, AppState};
use kumokara_workspace::filesystem::WorkspaceFilesystem;
use kumokara_workspace::WorkspaceManager;
use std::net::SocketAddr;

/// Run Kumokara in server (daemon) mode.
pub async fn run_server(bind: &str) -> Result<()> {
    // Detect tmux
    match kumokara_engine::detect_tmux() {
        Some(version) => {
            tracing::info!("{} detected (session recovery: enabled)", version);
        }
        None => {
            tracing::warn!("tmux not found — session recovery disabled");
        }
    }

    // Initialize filesystem
    let fs = WorkspaceFilesystem::new()?;
    fs.initialize()?;
    tracing::info!("Workspace directory: {}", fs.root_dir().display());

    // Set up auth
    let auth_manager = AuthManager::new();
    let token = auth_manager.server_token().to_string();
    println!("→ Token: {}", token);

    // Create workspace manager
    let workspace_manager = WorkspaceManager::new(fs);

    // Build app state
    let state = AppState::new(workspace_manager, auth_manager);

    let addr: SocketAddr = bind.parse()?;
    tracing::info!("Starting server on {}", addr);

    serve(addr, state).await?;

    Ok(())
}

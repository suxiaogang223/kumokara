//! Kumokara CLI — entry point.
//!
//! Two modes:
//! - `kumokara` (no args) — Local mode: start server + open browser
//! - `kumokara server` — Remote mode: start server daemon
//! - `kumokara workspace` — Workspace management commands (Phase 1+)

mod commands;

use clap::{Parser, Subcommand};
use std::process;

#[derive(Parser)]
#[command(
    name = "kumokara",
    version,
    about = "Kumokara（雲殻）— Self-hosted Agent Development Environment",
    long_about = "Agents never sleep in Kumokara.\nA self-hosted Agent Development Environment with persistent workspaces and 24/7 agent runtime."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start Kumokara server as a daemon (Remote mode)
    Server {
        /// Address to bind to
        #[arg(long, default_value = "127.0.0.1:9876")]
        bind: String,
    },
    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    /// Report agent events (used by CLI hooks)
    AgentEvent {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        state: String,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// List all workspaces
    List,
    /// Create a new workspace
    Create { name: String },
    /// Delete a workspace
    Delete { id: String },
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Server { bind }) => {
            if let Err(e) = commands::server::run_server(&bind).await {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::Workspace { action }) => {
            if let Err(e) = commands::workspace::handle(action).await {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::AgentEvent { workspace, session, state }) => {
            // Phase 1: POST to local server
            tracing::info!(
                "Agent event: workspace={}, session={}, state={}",
                workspace, session, state
            );
        }
        None => {
            // Default: Local mode — start server + open browser
            if let Err(e) = commands::local::run_local().await {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }
}

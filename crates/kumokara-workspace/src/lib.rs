//! kumokara-workspace — Workspace and session lifecycle management.
//!
//! Workspaces are the first-class citizen in Kumokara. They own:
//! - A filesystem directory (work_dir)
//! - 0..N terminal sessions (shell or agent)
//! - An event log and event bus
//! - Output buffers for each session

pub mod config;
pub mod env;
pub mod filesystem;
pub mod session;
pub mod workspace;

pub use config::WorkspaceConfig;
pub use env::EnvManager;
pub use filesystem::WorkspaceFilesystem;
pub use session::SessionManager;
pub use workspace::WorkspaceManager;

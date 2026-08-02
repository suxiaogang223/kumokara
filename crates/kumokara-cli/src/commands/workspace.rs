//! CLI workspace management commands.
//!
//! Phase 0: stub implementations.
//! Phase 1+: full CLI workspace CRUD with server API integration.

use anyhow::Result;

use crate::WorkspaceAction;

/// Handle workspace subcommands.
pub async fn handle(action: WorkspaceAction) -> Result<()> {
    match action {
        WorkspaceAction::List => {
            // Phase 1: query server API
            println!("Workspaces: (list not yet implemented in Phase 0)");
        }
        WorkspaceAction::Create { name } => {
            println!("Creating workspace '{}'... (not yet implemented in Phase 0)", name);
        }
        WorkspaceAction::Delete { id } => {
            println!("Deleting workspace '{}'... (not yet implemented in Phase 0)", id);
        }
    }
    Ok(())
}

//! Workspace lifecycle management.
//!
//! A Workspace is the first-class citizen in Kumokara — it owns sessions,
//! an event log, an event bus, and a filesystem directory.

use anyhow::Result;
use chrono::Utc;
use kumokara_event::{EventBus, EventLog};
use kumokara_protocol::event::{Event, EventEntry, EventSource};
use kumokara_protocol::workspace::{
    CreateWorkspaceRequest, UpdateWorkspaceRequest, WorkspaceInfo,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::WorkspaceConfig;
use crate::env::EnvManager;
use crate::filesystem::WorkspaceFilesystem;
use crate::session::SessionManager;

/// Manages all workspaces in the server.
pub struct WorkspaceManager {
    /// All active workspaces, keyed by workspace ID.
    workspaces: Arc<RwLock<HashMap<String, WorkspaceEntry>>>,
    /// Filesystem manager
    fs: WorkspaceFilesystem,
}

/// Internal representation of an active workspace.
struct WorkspaceEntry {
    info: WorkspaceInfo,
    config: WorkspaceConfig,
    env: EnvManager,
    event_bus: EventBus,
    event_log: Option<EventLog>,
    #[allow(dead_code)]
    sessions: SessionManager, // wired up in Phase 1
}

impl WorkspaceManager {
    /// Create a new workspace manager.
    pub fn new(fs: WorkspaceFilesystem) -> Self {
        Self {
            workspaces: Arc::new(RwLock::new(HashMap::new())),
            fs,
        }
    }

    /// Create a new workspace.
    pub async fn create_workspace(&self, request: CreateWorkspaceRequest) -> Result<WorkspaceInfo> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // Create directory structure
        self.fs.create_workspace_dirs(&id)?;

        // Create and persist config
        let config = WorkspaceConfig::new(
            request.name.clone(),
            request.env.clone(),
            request.agent_config.clone(),
        );
        config.to_file(&self.fs.workspace_config_path(&id))?;

        // Create env manager
        let env = EnvManager::new(request.env.unwrap_or_default());

        let info = WorkspaceInfo {
            id: id.clone(),
            name: request.name,
            status: "ready".to_string(),
            work_dir: self.fs.work_dir(&id).to_string_lossy().to_string(),
            created_at: now.clone(),
            updated_at: now,
            session_count: 0,
        };

        let entry = WorkspaceEntry {
            info: info.clone(),
            config,
            env,
            event_bus: EventBus::new(),
            event_log: None, // Lazy-initialized on first event
            sessions: SessionManager::new(),
        };

        self.workspaces.write().await.insert(id, entry);

        Ok(info)
    }

    /// List all workspaces.
    pub async fn list_workspaces(&self) -> Vec<WorkspaceInfo> {
        self.workspaces
            .read()
            .await
            .values()
            .map(|e| e.info.clone())
            .collect()
    }

    /// Get a workspace by ID.
    pub async fn get_workspace(&self, workspace_id: &str) -> Option<WorkspaceInfo> {
        self.workspaces
            .read()
            .await
            .get(workspace_id)
            .map(|e| e.info.clone())
    }

    /// Update a workspace.
    pub async fn update_workspace(
        &self,
        workspace_id: &str,
        request: UpdateWorkspaceRequest,
    ) -> Result<Option<WorkspaceInfo>> {
        let mut workspaces = self.workspaces.write().await;
        if let Some(entry) = workspaces.get_mut(workspace_id) {
            if let Some(name) = request.name {
                entry.info.name = name.clone();
                entry.config.set_name(name);
            }
            if let Some(env) = request.env {
                entry.config.set_env(env.clone());
                entry.env = EnvManager::new(env);
            }
            if let Some(agent_config) = request.agent_config {
                entry.config.set_agent_config(agent_config);
            }
            entry.info.updated_at = Utc::now().to_rfc3339();
            entry
                .config
                .to_file(&self.fs.workspace_config_path(workspace_id))?;
            Ok(Some(entry.info.clone()))
        } else {
            Ok(None)
        }
    }

    /// Destroy a workspace (irreversible).
    pub async fn destroy_workspace(&self, workspace_id: &str) -> Result<bool> {
        let entry = self.workspaces.write().await.remove(workspace_id);
        if entry.is_some() {
            self.fs.destroy_workspace(workspace_id)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the session manager for a workspace.
    pub async fn get_sessions(&self, workspace_id: &str) -> Option<SessionManager> {
        // SessionManager is Arc'd internally, so we clone the Arc
        self.workspaces
            .read()
            .await
            .get(workspace_id)
            .map(|_e| {
                // We need to return a SessionManager that wraps the same data.
                // For now, create a reference-based approach.
                // In Phase 1, SessionManager will be Arc'd and directly clonable.
                SessionManager::new() // Placeholder — will be properly shared in Phase 1
            })
    }

    /// Get the event bus for a workspace.
    pub async fn get_event_bus(&self, workspace_id: &str) -> Option<EventBus> {
        // EventBus doesn't implement Clone via broadcast channels well.
        // For Phase 0, we return a new bus. In Phase 1, use a proper shared handle.
        self.workspaces
            .read()
            .await
            .get(workspace_id)
            .map(|_e| EventBus::new())
    }

    /// Publish an event to a workspace's event bus and persist it.
    pub async fn publish_event(
        &self,
        workspace_id: &str,
        session_id: Option<&str>,
        event: Event,
        source: EventSource,
    ) -> Result<()> {
        let workspaces = self.workspaces.read().await;
        if let Some(entry) = workspaces.get(workspace_id) {
            let event_entry = EventEntry {
                seq: 0, // Will be assigned by event_log
                timestamp: Utc::now().to_rfc3339(),
                session_id: session_id.map(|s| s.to_string()),
                workspace_id: workspace_id.to_string(),
                source: source.to_string(),
                event,
            };

            // Publish on bus (best-effort; consumers may lag)
            let _ = entry.event_bus.publish(event_entry.clone());

            // Persist to event log if available
            if let Some(ref log) = entry.event_log {
                let _ = log.insert_event(&event_entry).await;
            }

            // Update workspace status based on event
            drop(workspaces);
            // (status update happens here in Phase 1)
        }
        Ok(())
    }

    /// Get the filesystem manager.
    pub fn filesystem(&self) -> &WorkspaceFilesystem {
        &self.fs
    }

    /// Get the workspace count.
    pub async fn workspace_count(&self) -> usize {
        self.workspaces.read().await.len()
    }
}

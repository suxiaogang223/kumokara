//! REST API handlers for workspace and session management.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use kumokara_protocol::workspace::{
    CreateWorkspaceRequest, SessionInfo, UpdateWorkspaceRequest, WorkspaceInfo,
};

use crate::AppState;

// ============================================================================
// Workspace endpoints
// ============================================================================

/// GET /api/workspaces — List all workspaces.
pub async fn list_workspaces(
    State(state): State<AppState>,
) -> Json<Vec<WorkspaceInfo>> {
    let workspaces = state.workspace_manager.list_workspaces().await;
    Json(workspaces)
}

/// POST /api/workspaces — Create a new workspace.
pub async fn create_workspace(
    State(state): State<AppState>,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<WorkspaceInfo>), (StatusCode, Json<serde_json::Value>)> {
    match state.workspace_manager.create_workspace(request).await {
        Ok(workspace) => Ok((StatusCode::CREATED, Json(workspace))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": e.to_string(),
                }
            })),
        )),
    }
}

/// GET /api/workspaces/:id — Get a specific workspace.
pub async fn get_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WorkspaceInfo>, (StatusCode, Json<serde_json::Value>)> {
    match state.workspace_manager.get_workspace(&id).await {
        Some(workspace) => Ok(Json(workspace)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": {
                    "code": "WORKSPACE_NOT_FOUND",
                    "message": "Workspace not found",
                }
            })),
        )),
    }
}

/// PUT /api/workspaces/:id — Update a workspace.
pub async fn update_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateWorkspaceRequest>,
) -> Result<Json<WorkspaceInfo>, (StatusCode, Json<serde_json::Value>)> {
    match state.workspace_manager.update_workspace(&id, request).await {
        Ok(Some(workspace)) => Ok(Json(workspace)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": {
                    "code": "WORKSPACE_NOT_FOUND",
                    "message": "Workspace not found",
                }
            })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": e.to_string(),
                }
            })),
        )),
    }
}

/// DELETE /api/workspaces/:id — Destroy a workspace.
pub async fn destroy_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.workspace_manager.destroy_workspace(&id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": {
                    "code": "WORKSPACE_NOT_FOUND",
                    "message": "Workspace not found",
                }
            })),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": e.to_string(),
                }
            })),
        )),
    }
}

// ============================================================================
// Session endpoints (stubs for REST API; full WS-based session mgmt in Phase 1)
// ============================================================================

/// GET /api/workspaces/:workspace_id/sessions — List sessions.
pub async fn list_sessions(
    State(_state): State<AppState>,
    Path(_workspace_id): Path<String>,
) -> Json<Vec<SessionInfo>> {
    // Phase 1: Return actual sessions from workspace
    Json(vec![])
}

/// POST /api/workspaces/:workspace_id/sessions — Create a new session.
pub async fn create_session(
    State(_state): State<AppState>,
    Path(_workspace_id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    // Phase 1: Actual session creation with PTY spawning
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": {
                "code": "INTERNAL_ERROR",
                "message": "Session creation via REST not yet implemented; use WebSocket",
            }
        })),
    ))
}

/// POST /api/sessions/:id/attach — Attach to a session.
pub async fn attach_session(
    Path(_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Phase 1: Screen dump + incremental sync
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": {
                "code": "INTERNAL_ERROR",
                "message": "Session attach not yet implemented in Phase 0",
            }
        })),
    ))
}

/// DELETE /api/sessions/:id — Destroy a session.
pub async fn destroy_session(
    Path(_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    Ok(StatusCode::NO_CONTENT)
}

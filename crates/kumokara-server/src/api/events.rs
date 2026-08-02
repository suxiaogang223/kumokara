//! REST API handlers for event log queries.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use kumokara_protocol::event::EventEntry;
use serde::Deserialize;

use crate::AppState;

/// Query parameters for event listing.
#[derive(Deserialize)]
pub struct EventQueryParams {
    pub after_seq: Option<i64>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub types: Option<String>, // comma-separated event type names
}

/// GET /api/workspaces/:workspace_id/events — Query events for a workspace.
pub async fn query_events(
    State(_state): State<AppState>,
    Path(_workspace_id): Path<String>,
    Query(_params): Query<EventQueryParams>,
) -> Json<Vec<EventEntry>> {
    // Phase 1: Query from event log
    // For Phase 0, return empty list
    Json(vec![])
}

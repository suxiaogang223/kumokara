//! kumokara-server — Axum-based HTTP and WebSocket server.
//!
//! This is the orchestration layer that ties together all Kumokara subsystems:
//! workspace management, PTY engine, event bus, shell integration, and authentication.

pub mod api;
pub mod session_registry;
pub mod ws_handler;

use anyhow::Result;
use kumokara_auth::AuthManager;
use kumokara_engine::has_tmux;
use kumokara_workspace::WorkspaceManager;
use session_registry::SessionRegistry;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Shared application state accessible by all axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// Workspace lifecycle manager
    pub workspace_manager: Arc<WorkspaceManager>,
    /// Authentication manager
    pub auth_manager: Arc<AuthManager>,
    /// Active PTY session registry
    pub session_registry: Arc<SessionRegistry>,
    /// Server version string
    pub version: String,
    /// Whether tmux is available
    pub tmux_available: bool,
}

impl AppState {
    /// Create a new AppState.
    pub fn new(
        workspace_manager: WorkspaceManager,
        auth_manager: AuthManager,
    ) -> Self {
        let tmux_available = has_tmux();
        Self {
            workspace_manager: Arc::new(workspace_manager),
            auth_manager: Arc::new(auth_manager),
            session_registry: Arc::new(SessionRegistry::new()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            tmux_available,
        }
    }
}

/// Create a fallback service that serves static files and falls back to index.html (SPA).
fn serve_spa_fallback(_dist_dir: PathBuf) -> axum::routing::MethodRouter {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use std::io::Read;

    async fn handler(req: Request<Body>) -> Response {
        let path = req.uri().path();
        let file_path = match path {
            "/" => "index.html",
            p => &p[1..], // strip leading /
        };

        // Try to read the file from dist/ and fallback locations
        let candidates = [
            PathBuf::from("web/dist"),
            PathBuf::from("../web/dist"),
            PathBuf::from("../../web/dist"),
        ];

        for dist in &candidates {
            let full_path = dist.join(file_path);
            if full_path.exists() && full_path.is_file() {
                let mut file = std::fs::File::open(&full_path).unwrap();
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).unwrap();
                let content_type = mime_guess(file_path);
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", content_type)
                    .body(Body::from(contents))
                    .unwrap();
            }
        }

        // SPA fallback: serve index.html for any non-file route
        for dist in &candidates {
            let index_path = dist.join("index.html");
            if index_path.exists() {
                let mut file = std::fs::File::open(&index_path).unwrap();
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).unwrap();
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Body::from(contents))
                    .unwrap();
            }
        }

        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap()
    }

    fn mime_guess(path: &str) -> &'static str {
        match path.rsplit('.').next() {
            Some("html") => "text/html; charset=utf-8",
            Some("js") => "application/javascript",
            Some("css") => "text/css",
            Some("json") => "application/json",
            Some("png") => "image/png",
            Some("svg") => "image/svg+xml",
            Some("ico") => "image/x-icon",
            Some("woff2") => "font/woff2",
            _ => "application/octet-stream",
        }
    }

    axum::routing::any(handler)
}

/// Find the frontend dist directory, checking multiple possible locations.
fn find_dist_dir() -> PathBuf {
    // Check relative to current working directory (works for `cargo run`)
    let candidates = [
        PathBuf::from("web/dist"),
        PathBuf::from("../web/dist"),
        PathBuf::from("../../web/dist"),
    ];

    for candidate in &candidates {
        if candidate.exists() && candidate.join("index.html").exists() {
            tracing::info!("Serving frontend from: {}", candidate.display());
            return candidate.clone();
        }
    }

    // Fallback: return the expected path anyway (will 404 if missing)
    tracing::warn!("Frontend dist/ not found. Build it with: cd web && npm run build");
    PathBuf::from("web/dist")
}

/// Start the Kumokara server.
///
/// Binds to `addr` and starts serving HTTP + WebSocket connections.
pub async fn serve(
    addr: SocketAddr,
    state: AppState,
) -> Result<()> {
    // Print the startup banner
    print_banner(&state, &addr);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Find the frontend dist directory
    let dist_dir = find_dist_dir();

    let app = axum::Router::new()
        // REST API (defined first, takes precedence over fallback)
        .route("/api/workspaces", axum::routing::get(api::workspace::list_workspaces))
        .route("/api/workspaces", axum::routing::post(api::workspace::create_workspace))
        .route("/api/workspaces/{id}", axum::routing::get(api::workspace::get_workspace))
        .route("/api/workspaces/{id}", axum::routing::put(api::workspace::update_workspace))
        .route("/api/workspaces/{id}", axum::routing::delete(api::workspace::destroy_workspace))
        // Session API
        .route("/api/workspaces/{workspace_id}/sessions", axum::routing::get(api::workspace::list_sessions))
        .route("/api/workspaces/{workspace_id}/sessions", axum::routing::post(api::workspace::create_session))
        .route("/api/sessions/{id}/attach", axum::routing::post(api::workspace::attach_session))
        .route("/api/sessions/{id}", axum::routing::delete(api::workspace::destroy_session))
        // Event API
        .route("/api/workspaces/{workspace_id}/events", axum::routing::get(api::events::query_events))
        // WebSocket
        .route("/api/ws", axum::routing::get(ws_handler::ws_upgrade))
        // Health check
        .route("/api/health", axum::routing::get(health_check))
        // Serve frontend SPA: all unmatched routes return index.html
        .fallback_service(serve_spa_fallback(dist_dir.clone()))
        .layer(cors)
        .layer(axum::Extension(state.auth_manager.clone()))
        .with_state(state);

    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Print the startup banner (matching DESIGN.md §2).
fn print_banner(state: &AppState, addr: &SocketAddr) {
    println!("Kumokara（雲殻） v{} — Agents never sleep in Kumokara.", state.version);

    if state.tmux_available {
        if let Some(version) = kumokara_engine::detect_tmux() {
            println!("✓ {} detected (session recovery: enabled)", version);
        } else {
            println!("✓ tmux detected (session recovery: enabled)");
        }
    } else {
        println!("⚠ tmux not found — session recovery disabled. Install tmux for 24h agent persistence.");
    }

    println!("✓ Workspace directory: ~/.kumokara/workspaces/");
    println!("→ Server listening on http://{}", addr);
}

/// Health check endpoint.
async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<serde_json::Value> {
    let workspace_count = state.workspace_manager.workspace_count().await;
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": state.version,
        "tmux_available": state.tmux_available,
        "workspace_count": workspace_count,
    }))
}

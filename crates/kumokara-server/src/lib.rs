//! HTTP/WebSocket boundary for the Kumokara terminal runtime.

mod output_history;
mod process_discovery;
pub mod session_registry;
pub mod ws_handler;

use anyhow::Result;
use kumokara_auth::AuthManager;
use kumokara_engine::has_tmux;
use session_registry::SessionRegistry;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

const DEFAULT_TERMINAL_COLS: u16 = 100;
const DEFAULT_TERMINAL_ROWS: u16 = 30;

#[derive(Clone)]
pub struct AppState {
    pub auth_manager: Option<Arc<AuthManager>>,
    pub session_registry: Arc<SessionRegistry>,
    pub version: String,
    pub tmux_available: bool,
}

impl AppState {
    pub fn new(auth_manager: Option<AuthManager>) -> Self {
        Self {
            auth_manager: auth_manager.map(Arc::new),
            session_registry: Arc::new(SessionRegistry::new()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            tmux_available: has_tmux(),
        }
    }

    pub fn auth_required(&self) -> bool {
        self.auth_manager.is_some()
    }
}

pub async fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    ensure_default_session(&state).await?;
    if !state.auth_required() {
        tracing::warn!("Authentication is disabled; use --require-token outside trusted development environments");
    }

    let dist_dir = find_dist_dir();
    let spa =
        ServeDir::new(&dist_dir).not_found_service(ServeFile::new(dist_dir.join("index.html")));
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .route("/api/ws", axum::routing::get(ws_handler::ws_upgrade))
        .route("/api/health", axum::routing::get(health_check))
        .fallback_service(spa)
        .layer(cors)
        .with_state(state);

    tracing::info!(%addr, "Kumokara server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ensure_default_session(state: &AppState) -> Result<()> {
    if state.session_registry.count().await == 0 {
        state
            .session_registry
            .create_shell_session(
                std::env::current_dir()?,
                DEFAULT_TERMINAL_COLS,
                DEFAULT_TERMINAL_ROWS,
            )
            .await?;
    }
    Ok(())
}

fn find_dist_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("web/dist"),
        PathBuf::from("../web/dist"),
        PathBuf::from("../../web/dist"),
    ];
    candidates
        .into_iter()
        .find(|path| path.join("index.html").is_file())
        .unwrap_or_else(|| {
            tracing::warn!("Frontend not built; run `cd web && npm run build`");
            PathBuf::from("web/dist")
        })
}

async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": state.version,
        "tmux_available": state.tmux_available,
        "auth_required": state.auth_required(),
        "session_count": state.session_registry.count().await,
    }))
}

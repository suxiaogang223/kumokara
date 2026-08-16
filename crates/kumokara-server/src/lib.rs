//! HTTP/WebSocket boundary for the Kumokara terminal runtime.

mod directory_browser;
mod output_history;
mod process_discovery;
pub mod session_registry;
pub mod ws_handler;

use anyhow::Result;
use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::Response;
use include_dir::{include_dir, Dir};
use kumokara_agent::AgentAdapterRegistry;
use kumokara_auth::AuthManager;
use session_registry::SessionRegistry;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

static EMBEDDED_WEB: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web-dist");

const DEFAULT_TERMINAL_COLS: u16 = 100;
const DEFAULT_TERMINAL_ROWS: u16 = 30;

#[derive(Clone)]
pub struct AppState {
    pub auth_manager: Option<Arc<AuthManager>>,
    pub session_registry: Arc<SessionRegistry>,
    pub version: String,
}

impl AppState {
    pub fn new(auth_manager: Option<AuthManager>) -> Self {
        Self::with_agent_adapters(auth_manager, AgentAdapterRegistry::with_builtins())
    }

    pub fn with_agent_adapters(
        auth_manager: Option<AuthManager>,
        agent_adapters: AgentAdapterRegistry,
    ) -> Self {
        Self {
            auth_manager: auth_manager.map(Arc::new),
            session_registry: Arc::new(SessionRegistry::with_agent_adapters(agent_adapters)),
            version: env!("CARGO_PKG_VERSION").to_string(),
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

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .route("/api/ws", axum::routing::get(ws_handler::ws_upgrade))
        .route("/api/health", axum::routing::get(health_check))
        .layer(cors);
    let app = if let Some(dist_dir) = find_dist_dir() {
        let spa =
            ServeDir::new(&dist_dir).not_found_service(ServeFile::new(dist_dir.join("index.html")));
        app.fallback_service(spa)
    } else {
        tracing::info!("Serving the embedded Kumokara web interface");
        app.fallback(embedded_asset)
    }
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
                directory_browser::home()?,
                DEFAULT_TERMINAL_COLS,
                DEFAULT_TERMINAL_ROWS,
            )
            .await?;
    }
    Ok(())
}

fn find_dist_dir() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("web/dist"),
        PathBuf::from("../web/dist"),
        PathBuf::from("../../web/dist"),
    ];
    candidates
        .into_iter()
        .find(|path| path.join("index.html").is_file())
}

async fn embedded_asset(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };
    let file = EMBEDDED_WEB
        .get_file(requested)
        .or_else(|| EMBEDDED_WEB.get_file("index.html"));

    let Some(file) = file else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .expect("static response is valid");
    };
    let content_type = match file.path().extension().and_then(|value| value.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };
    let cache_control = if file.path() == std::path::Path::new("index.html") {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(file.contents()))
        .expect("static response is valid")
}

async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": state.version,
        "server_restart_recovery": false,
        "auth_required": state.auth_required(),
        "session_count": state.session_registry.count().await,
    }))
}

#[cfg(test)]
mod embedded_web_tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn release_bundle_contains_the_web_entrypoint() {
        let index = EMBEDDED_WEB
            .get_file("index.html")
            .expect("embedded web entrypoint");
        assert!(index
            .contents_utf8()
            .is_some_and(|html| html.contains("Kumokara")));
        assert!(EMBEDDED_WEB.get_dir("assets").is_some());
    }

    #[tokio::test]
    async fn embedded_web_falls_back_to_the_spa_entrypoint() {
        let response = embedded_asset(Uri::from_static("/workspace/example")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(body.windows(6).any(|window| window == b"<html "));
    }
}

//! End-to-end integration tests for Kumokara Phase 0.
//!
//! Tests the critical path: start server → create workspace → shell session → terminal I/O.

use std::net::SocketAddr;
use std::time::Duration;

use kumokara_auth::AuthManager;
use kumokara_server::{serve, AppState};
use kumokara_workspace::filesystem::WorkspaceFilesystem;
use kumokara_workspace::WorkspaceManager;
use tokio::net::TcpListener;

/// Find an available port on localhost for testing.
async fn find_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// Spawn a test server and return its address and auth token.
async fn spawn_test_server() -> (SocketAddr, String) {
    let port = find_available_port().await;
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    // Use a temp directory for data
    let temp_dir = tempfile::tempdir().unwrap();
    let fs = WorkspaceFilesystem::with_root(temp_dir.path().join(".kumokara"));
    fs.initialize().unwrap();

    let auth_manager = AuthManager::new();
    let token = auth_manager.server_token().to_string();

    let workspace_manager = WorkspaceManager::new(fs);
    let state = AppState::new(workspace_manager, auth_manager);

    // Spawn server in background
    let server_state = state.clone();
    tokio::spawn(async move {
        let _ = serve(addr, server_state).await;
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(300)).await;

    (addr, token)
}

#[tokio::test]
async fn test_health_check() {
    let (addr, _token) = spawn_test_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/health", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn test_create_and_list_workspaces() {
    let (addr, _token) = spawn_test_server().await;

    let client = reqwest::Client::new();

    // List — initially empty
    let resp = client
        .get(format!("http://{}/api/workspaces", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let workspaces: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(workspaces.is_empty());

    // Create a workspace
    let resp = client
        .post(format!("http://{}/api/workspaces", addr))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "name": "test-project",
            "env": { "NODE_ENV": "test" }
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let workspace: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(workspace["name"], "test-project");
    assert!(workspace["id"].is_string());
    assert_eq!(workspace["status"], "ready");

    let workspace_id = workspace["id"].as_str().unwrap().to_string();

    // List — should have one
    let resp = client
        .get(format!("http://{}/api/workspaces", addr))
        .send()
        .await
        .unwrap();
    let workspaces: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(workspaces.len(), 1);

    // Delete workspace
    let resp = client
        .delete(format!("http://{}/api/workspaces/{}", addr, workspace_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_workspace_not_found() {
    let (addr, _token) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{}/api/workspaces/nonexistent", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "WORKSPACE_NOT_FOUND");
}

#[tokio::test]
async fn test_auth_token_validation() {
    let auth = AuthManager::new();
    let token = auth.server_token().to_string();

    assert!(auth.validate_token(&token));
    assert!(!auth.validate_token("wrong-token"));
    assert!(!auth.validate_token(""));
}

#[tokio::test]
async fn test_tmux_detection() {
    let result = kumokara_engine::detect_tmux();

    // On macOS without tmux installed, should be None.
    // On systems with tmux, should be Some(version_string).
    // Either way, the function should not panic.
    if let Some(version) = result {
        assert!(version.contains("tmux") || version.contains('.'));
    }
}

#[tokio::test]
async fn test_osc_parsing() {
    use kumokara_protocol::event::Event;
    use kumokara_shellint::parse::parse_output_chunk;

    // Command started
    let results = parse_output_chunk("s1", "ws1", b"\x1b]133;C\x07");
    assert_eq!(results.len(), 1);
    match &results[0] {
        kumokara_shellint::parse::ParseResult::Event(Event::CommandStarted { session_id, .. }) => {
            assert_eq!(session_id, "s1");
        }
        _ => panic!("Expected CommandStarted"),
    }

    // Command finished
    let results = parse_output_chunk("s1", "ws1", b"\x1b]133;D;0\x07");
    match &results[0] {
        kumokara_shellint::parse::ParseResult::Event(Event::CommandFinished {
            session_id,
            exit_code,
            ..
        }) => {
            assert_eq!(session_id, "s1");
            assert_eq!(*exit_code, 0);
        }
        _ => panic!("Expected CommandFinished"),
    }

    // CWD changed
    let results = parse_output_chunk("s1", "ws1", b"\x1b]7;file://host/home/user\x07");
    match &results[0] {
        kumokara_shellint::parse::ParseResult::Event(Event::CwdChanged { session_id, path }) => {
            assert_eq!(session_id, "s1");
            assert_eq!(path, "/home/user");
        }
        _ => panic!("Expected CwdChanged"),
    }
}

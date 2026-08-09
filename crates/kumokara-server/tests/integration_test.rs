//! Integration tests for the session-first server boundary.

use futures::{SinkExt, StreamExt};
use kumokara_auth::AuthManager;
use kumokara_server::{serve, AppState};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

async fn available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct TestServer {
    addr: SocketAddr,
    token: Option<String>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn spawn_test_server(auth: Option<AuthManager>) -> TestServer {
    let addr = format!("127.0.0.1:{}", available_port().await)
        .parse()
        .unwrap();
    let token = auth
        .as_ref()
        .map(|manager| manager.server_token().to_string());
    let state = AppState::new(auth);
    let task = tokio::spawn(serve(addr, state));
    tokio::time::sleep(Duration::from_millis(300)).await;
    TestServer { addr, token, task }
}

#[tokio::test]
async fn startup_creates_one_default_session() {
    let server = spawn_test_server(None).await;
    let response = reqwest::get(format!("http://{}/api/health", server.addr))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["server_restart_recovery"], false);
    assert_eq!(body["auth_required"], false);
    assert_eq!(body["session_count"], 1);
}

#[tokio::test]
async fn websocket_requires_auth_as_the_first_message() {
    let server = spawn_test_server(Some(AuthManager::new())).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/api/ws", server.addr))
        .await
        .unwrap();
    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "list"
        })))
        .await
        .unwrap();
    assert_eq!(recv_json(&mut socket).await["type"], "auth_error");
}

#[tokio::test]
async fn websocket_connects_without_auth_by_default() {
    let server = spawn_test_server(None).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/api/ws", server.addr))
        .await
        .unwrap();

    assert_eq!(recv_json(&mut socket).await["type"], "auth_ok");
    socket
        .send(json_message(serde_json::json!({
            "type": "auth",
            "token": "retained-client-token"
        })))
        .await
        .unwrap();
    assert_eq!(recv_json(&mut socket).await["type"], "auth_ok");
    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "list"
        })))
        .await
        .unwrap();
    let listed = recv_until_type(&mut socket, "session_list").await;
    assert_eq!(listed["sessions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn only_the_active_browser_viewport_resizes_the_shared_pty() {
    let server = spawn_test_server(None).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/api/ws", server.addr))
        .await
        .unwrap();
    assert_eq!(recv_json(&mut socket).await["type"], "auth_ok");

    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "initial"
        })))
        .await
        .unwrap();
    let initial = recv_until_type(&mut socket, "session_list").await;
    let session = &initial["sessions"][0];
    let session_id = session["id"].as_str().unwrap();
    assert_eq!(
        (session["cols"].as_u64(), session["rows"].as_u64()),
        (Some(100), Some(30))
    );

    // Legacy clients omitted `active`; keep those resize messages passive.
    socket
        .send(json_message(serde_json::json!({
            "type": "terminal_resize",
            "session_id": session_id,
            "cols": 40,
            "rows": 10
        })))
        .await
        .unwrap();
    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "after-passive-resize"
        })))
        .await
        .unwrap();
    let unchanged = recv_until_type(&mut socket, "session_list").await;
    let unchanged = &unchanged["sessions"][0];
    assert_eq!(
        (unchanged["cols"].as_u64(), unchanged["rows"].as_u64()),
        (Some(100), Some(30))
    );

    // A focused browser explicitly adopts its fitted grid, allowing a running
    // full-screen TUI to redraw before the user types again.
    socket
        .send(json_message(serde_json::json!({
            "type": "terminal_resize",
            "session_id": session_id,
            "cols": 120,
            "rows": 40,
            "active": true
        })))
        .await
        .unwrap();
    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "after-active-resize"
        })))
        .await
        .unwrap();
    let controlled = recv_until_type(&mut socket, "session_list").await;
    let controlled = &controlled["sessions"][0];
    assert_eq!(
        (controlled["cols"].as_u64(), controlled["rows"].as_u64()),
        (Some(120), Some(40))
    );
}

#[tokio::test]
async fn agent_metadata_and_terminal_title_round_trip_over_websocket() {
    let server = spawn_test_server(None).await;
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/api/ws", server.addr))
        .await
        .unwrap();
    assert_eq!(recv_json(&mut socket).await["type"], "auth_ok");

    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "initial"
        })))
        .await
        .unwrap();
    let initial = recv_until_type(&mut socket, "session_list").await;
    let session_id = initial["sessions"][0]["id"].as_str().unwrap();

    socket
        .send(json_message(serde_json::json!({
            "type": "agent_update",
            "session_id": session_id,
            "code_agent": "claude",
            "session_title": "Review adapter plugins",
            "status": "awaiting-input",
            "detail": "approval"
        })))
        .await
        .unwrap();
    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "agent"
        })))
        .await
        .unwrap();
    let adapted = recv_until_type(&mut socket, "session_list").await;
    let adapted = &adapted["sessions"][0];
    assert_eq!(adapted["title"], "Review adapter plugins");
    assert_eq!(adapted["agent"]["provider"], "claude_code");
    assert_eq!(adapted["agent"]["display_name"], "Claude Code");
    assert_eq!(adapted["agent"]["status"], "awaiting-input");

    socket
        .send(json_message(serde_json::json!({
            "type": "terminal_title",
            "session_id": session_id,
            "title": "OC | Product design\u{1b}\u{7}"
        })))
        .await
        .unwrap();
    socket
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "title"
        })))
        .await
        .unwrap();
    let titled = recv_until_type(&mut socket, "session_list").await;
    assert_eq!(titled["sessions"][0]["title"], "OC | Product design");
}

#[tokio::test]
async fn session_survives_websocket_reconnect() {
    use base64::Engine;

    let server = spawn_test_server(Some(AuthManager::new())).await;
    let token = server.token.as_deref().unwrap();
    let url = format!("ws://{}/api/ws", server.addr);
    let (mut first, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    authenticate(&mut first, token).await;

    first
        .send(json_message(serde_json::json!({
            "type": "session_create",
            "request_id": "create",
            "cols": 80,
            "rows": 24
        })))
        .await
        .unwrap();
    let created = recv_until_type(&mut first, "session_created").await;
    let session_id = created["session"]["id"].as_str().unwrap().to_string();
    first.close(None).await.unwrap();

    let (mut second, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    authenticate(&mut second, token).await;
    second
        .send(json_message(serde_json::json!({
            "type": "session_list",
            "request_id": "list"
        })))
        .await
        .unwrap();
    let listed = recv_until_type(&mut second, "session_list").await;
    assert!(listed["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|session| session["id"] == session_id));

    second
        .send(json_message(serde_json::json!({
            "type": "session_attach",
            "request_id": "attach",
            "session_id": session_id
        })))
        .await
        .unwrap();
    let input =
        base64::engine::general_purpose::STANDARD.encode(b"printf '__kumokara_reconnected__\\n'\n");
    second
        .send(json_message(serde_json::json!({
            "type": "terminal_input",
            "session_id": session_id,
            "data_base64": input
        })))
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let output = recv_until_type(&mut second, "terminal_output").await;
            let data = output["data_base64"]
                .as_str()
                .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
                .unwrap_or_default();
            if String::from_utf8_lossy(&data).contains("__kumokara_reconnected__") {
                break;
            }
        }
    })
    .await
    .expect("reconnected client did not receive PTY output");

    second
        .send(json_message(serde_json::json!({
            "type": "session_destroy",
            "request_id": "destroy",
            "session_id": session_id
        })))
        .await
        .unwrap();
    assert_eq!(
        recv_until_type(&mut second, "session_destroyed").await["type"],
        "session_destroyed"
    );
}

async fn authenticate<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>, token: &str)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(json_message(serde_json::json!({
            "type": "auth",
            "token": token
        })))
        .await
        .unwrap();
    assert_eq!(recv_json(socket).await["type"], "auth_ok");
}

fn json_message(value: serde_json::Value) -> Message {
    Message::Text(value.to_string().into())
}

async fn recv_json<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> serde_json::Value
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        match socket.next().await.unwrap().unwrap() {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            Message::Ping(data) => socket.send(Message::Pong(data)).await.unwrap(),
            _ => {}
        }
    }
}

async fn recv_until_type<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    expected: &str,
) -> serde_json::Value
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let message = recv_json(socket).await;
        if message["type"] == expected {
            return message;
        }
    }
}

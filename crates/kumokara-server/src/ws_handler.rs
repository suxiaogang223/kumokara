//! WebSocket handler — connection upgrades, auth first-message, and message dispatch.
//!
//! Following DESIGN.md §5.1:
//! - Authentication is via the first message after WebSocket connection (must be `auth { token }`)
//! - Control messages use text frames (JSON tagged enum)
//! - Terminal I/O uses binary frames (24-byte header + raw data)

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use kumokara_protocol::messages::{ClientMessage, ServerMessage};
use kumokara_protocol::workspace::CreateWorkspaceRequest;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::AppState;

/// Upgrade an HTTP connection to WebSocket.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Handle a WebSocket connection lifecycle.
async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();

    // Shared sender for use across async tasks
    let sender = Arc::new(Mutex::new(sender));
    let mut authenticated = false;

    tracing::info!("WebSocket connection established, waiting for auth");

    // Process messages
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                let text_str = text.to_string();

                // Parse the client message
                let client_msg: ClientMessage = match serde_json::from_str(&text_str) {
                    Ok(msg) => msg,
                    Err(e) => {
                        let _ = send_error(&sender, None, "INVALID_MESSAGE", &e.to_string()).await;
                        continue;
                    }
                };

                // Handle auth first — required before any other message
                if !authenticated {
                    if let ClientMessage::Auth { token } = &client_msg {
                        if state.auth_manager.validate_token(token) {
                            authenticated = true;
                            let response = ServerMessage::AuthOk {
                                server_version: state.version.clone(),
                            };
                            let _ = send_message(&sender, &response).await;
                            tracing::info!("WebSocket client authenticated");
                            continue;
                        } else {
                            let response = ServerMessage::AuthError {
                                code: "AUTH_INVALID".to_string(),
                                message: "Invalid authentication token".to_string(),
                            };
                            let _ = send_message(&sender, &response).await;
                            tracing::warn!("WebSocket auth failed — invalid token");
                            break; // Close connection on auth failure
                        }
                    } else {
                        let response = ServerMessage::AuthError {
                            code: "AUTH_INVALID".to_string(),
                            message: "Authentication required — send auth message first".to_string(),
                        };
                        let _ = send_message(&sender, &response).await;
                        break; // Close connection
                    }
                }

                // Dispatch authenticated messages
                if let Err(e) = dispatch_message(&state, &sender, client_msg).await {
                    tracing::error!("Error dispatching message: {}", e);
                }
            }

            Message::Binary(data) => {
                if !authenticated {
                    let response = ServerMessage::AuthError {
                        code: "AUTH_INVALID".to_string(),
                        message: "Authentication required".to_string(),
                    };
                    let _ = send_message(&sender, &response).await;
                    break;
                }

                // Binary frames: 24-byte header followed by terminal data
                // Header: 16 bytes session_id UUID + 8 bytes seq u64 (big-endian)
                if data.len() >= 24 {
                    let session_id_bytes = &data[..16];
                    let session_id = uuid::Uuid::from_slice(session_id_bytes)
                        .map(|u| u.to_string())
                        .unwrap_or_default();
                    let _seq = u64::from_be_bytes(data[16..24].try_into().unwrap_or([0; 8]));
                    let terminal_data = &data[24..];

                    // Forward terminal input to the PTY
                    // Phase 0: pass through (PTY forwarding will be connected in full server)
                    tracing::debug!(
                        "Binary terminal input: session={}, len={}",
                        session_id,
                        terminal_data.len()
                    );
                }
            }

            Message::Close(_) => {
                tracing::info!("WebSocket client disconnected");
                break;
            }

            _ => {}
        }
    }
}

/// Dispatch an authenticated client message to the appropriate handler.
async fn dispatch_message(
    state: &AppState,
    sender: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
    msg: ClientMessage,
) -> Result<(), anyhow::Error> {
    match msg {
        ClientMessage::CreateWorkspace { request_id, name, env, agent_config } => {
            let request = CreateWorkspaceRequest {
                name,
                env,
                agent_config,
            };
            match state.workspace_manager.create_workspace(request).await {
                Ok(workspace) => {
                    let response = ServerMessage::WorkspaceCreated {
                        request_id,
                        workspace,
                    };
                    let _ = send_message(sender, &response).await;
                }
                Err(e) => {
                    let _ = send_error(sender, Some(&request_id), "INTERNAL_ERROR", &e.to_string()).await;
                }
            }
        }

        ClientMessage::ListWorkspaces { request_id } => {
            let workspaces = state.workspace_manager.list_workspaces().await;
            let response = ServerMessage::WorkspaceList {
                request_id,
                workspaces,
            };
            let _ = send_message(sender, &response).await;
        }

        ClientMessage::GetWorkspace { request_id, workspace_id } => {
            match state.workspace_manager.get_workspace(&workspace_id).await {
                Some(_workspace) => {
                    // For now, return via list; in Phase 1, add dedicated response
                    let workspaces = state.workspace_manager.list_workspaces().await;
                    let response = ServerMessage::WorkspaceList {
                        request_id,
                        workspaces,
                    };
                    let _ = send_message(sender, &response).await;
                }
                None => {
                    let _ = send_error(sender, Some(&request_id), "WORKSPACE_NOT_FOUND", "Workspace not found").await;
                }
            }
        }

        ClientMessage::DestroyWorkspace { request_id, workspace_id } => {
            match state.workspace_manager.destroy_workspace(&workspace_id).await {
                Ok(true) => {
                    let response = ServerMessage::WorkspaceDestroyed { workspace_id };
                    let _ = send_message(sender, &response).await;
                }
                Ok(false) => {
                    let _ = send_error(sender, Some(&request_id), "WORKSPACE_NOT_FOUND", "Workspace not found").await;
                }
                Err(e) => {
                    let _ = send_error(sender, Some(&request_id), "INTERNAL_ERROR", &e.to_string()).await;
                }
            }
        }

        ClientMessage::UpdateWorkspace { request_id, workspace_id, name, env, agent_config } => {
            let request = kumokara_protocol::workspace::UpdateWorkspaceRequest {
                name,
                env,
                agent_config,
            };
            match state.workspace_manager.update_workspace(&workspace_id, request).await {
                Ok(Some(workspace)) => {
                    let response = ServerMessage::WorkspaceUpdated {
                        workspace_id,
                        workspace,
                    };
                    let _ = send_message(sender, &response).await;
                }
                Ok(None) => {
                    let _ = send_error(sender, Some(&request_id), "WORKSPACE_NOT_FOUND", "Workspace not found").await;
                }
                Err(e) => {
                    let _ = send_error(sender, Some(&request_id), "INTERNAL_ERROR", &e.to_string()).await;
                }
            }
        }

        // --- Session management ---
        ClientMessage::SessionCreate { request_id, workspace_id, session_type, cols, rows } => {
            let session_id = uuid::Uuid::new_v4().to_string();
            let title = match session_type.as_str() {
                "agent" => "agent",
                _ => "shell",
            };
            let session_info = kumokara_protocol::workspace::SessionInfo {
                id: session_id.clone(),
                workspace_id: workspace_id.clone(),
                session_type: session_type.clone(),
                agent: None,
                title: title.to_string(),
                state: "active".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                last_active_at: chrono::Utc::now().to_rfc3339(),
                cols,
                rows,
            };

            // Spawn a real PTY for this session
            let spawn_result = state.session_registry.create_shell_session(
                &session_id,
                cols,
                rows,
                sender.clone(),
            ).await;

            match spawn_result {
                Ok(()) => {
                    let response = ServerMessage::SessionCreated {
                        request_id,
                        workspace_id,
                        session: session_info,
                    };
                    let _ = send_message(sender, &response).await;
                }
                Err(e) => {
                    let _ = send_error(sender, Some(&request_id), "INTERNAL_ERROR", &e.to_string()).await;
                }
            }
        }

        ClientMessage::SessionList { request_id, workspace_id: _ } => {
            // Phase 0: return empty list (sessions tracked in registry, not workspace manager)
            let response = ServerMessage::SessionList {
                request_id,
                sessions: vec![],
            };
            let _ = send_message(sender, &response).await;
        }

        ClientMessage::TerminalInput { session_id, data } => {
            // Decode base64 input and forward to PTY
            use base64::Engine;
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&data) {
                let _ = state.session_registry.write_input(&session_id, &bytes).await;
            }
        }

        ClientMessage::TerminalResize { session_id, cols, rows } => {
            let _ = state.session_registry.resize(&session_id, cols, rows).await;
        }

        ClientMessage::SessionDestroy { request_id, session_id } => {
            state.session_registry.remove(&session_id).await;
            let response = ServerMessage::SessionDestroyed { session_id };
            let _ = send_message(sender, &response).await;
            // Echo request_id back via the type system isn't needed since SessionDestroyed doesn't have request_id
            // The client matches on the session_id
            drop(request_id);
        }

        _ => {
            // Unhandled message types in Phase 0
            let _ = send_error(
                sender,
                None,
                "INTERNAL_ERROR",
                "Message type not yet implemented in Phase 0",
            )
            .await;
        }
    }

    Ok(())
}

/// Send a JSON message to the client.
async fn send_message(
    sender: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
    msg: &ServerMessage,
) -> Result<(), anyhow::Error> {
    let json = serde_json::to_string(msg)?;
    sender.lock().await.send(Message::Text(json.into())).await?;
    Ok(())
}

/// Send an error message to the client.
async fn send_error(
    sender: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
    request_id: Option<&str>,
    code: &str,
    message: &str,
) -> Result<(), anyhow::Error> {
    let msg = ServerMessage::Error {
        request_id: request_id.map(|s| s.to_string()),
        code: code.to_string(),
        message: message.to_string(),
    };
    let json = serde_json::to_string(&msg)?;
    sender.lock().await.send(Message::Text(json.into())).await?;
    Ok(())
}

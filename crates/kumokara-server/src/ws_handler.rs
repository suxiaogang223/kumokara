//! Authenticated WebSocket transport for session control and terminal I/O.

use crate::directory_browser;
use crate::session_registry::{AgentUpdate, TerminalChunk};
use crate::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{stream::SplitSink, SinkExt, StreamExt};
use kumokara_protocol::messages::{ClientMessage, ServerMessage};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

type WsSender = Arc<Mutex<SplitSink<WebSocket, Message>>>;
type AttachmentTasks = Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>;

pub async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

async fn handle_connection(socket: WebSocket, state: AppState) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    let attachments = Arc::new(Mutex::new(HashMap::new()));
    let mut authenticated = !state.auth_required();

    if authenticated && send_auth_ok(&state, &sender).await.is_err() {
        return;
    }

    while let Some(Ok(frame)) = receiver.next().await {
        match frame {
            Message::Text(text) => {
                let message = match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(message) => message,
                    Err(error) => {
                        let _ =
                            send_error(&sender, None, "INVALID_MESSAGE", &error.to_string()).await;
                        continue;
                    }
                };

                if !authenticated {
                    match authenticate(&state, &sender, &message).await {
                        Ok(()) => {
                            authenticated = true;
                            continue;
                        }
                        Err(()) => break,
                    }
                }

                if let Err(error) = dispatch(&state, &sender, &attachments, message).await {
                    tracing::warn!(%error, "failed to handle WebSocket message");
                }
            }
            Message::Binary(data) if authenticated => {
                handle_binary_input(&state, &sender, &data).await;
            }
            Message::Binary(_) => {
                let _ = send_auth_error(&sender, "Authentication required").await;
                break;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    for (_, task) in attachments.lock().await.drain() {
        task.abort();
    }
}

async fn authenticate(
    state: &AppState,
    sender: &WsSender,
    message: &ClientMessage,
) -> Result<(), ()> {
    let ClientMessage::Auth { token } = message else {
        let _ = send_auth_error(sender, "Send an auth message first").await;
        return Err(());
    };
    let Some(auth_manager) = &state.auth_manager else {
        return send_auth_ok(state, sender).await.map_err(|_| ());
    };
    if !auth_manager.validate_token(token) {
        let _ = send_auth_error(sender, "Invalid authentication token").await;
        return Err(());
    }

    send_auth_ok(state, sender).await.map_err(|_| ())
}

async fn send_auth_ok(state: &AppState, sender: &WsSender) -> anyhow::Result<()> {
    send_message(
        sender,
        &ServerMessage::AuthOk {
            server_version: state.version.clone(),
        },
    )
    .await
}

async fn handle_binary_input(state: &AppState, sender: &WsSender, data: &[u8]) {
    if data.len() < 24 {
        let _ = send_error(
            sender,
            None,
            "INVALID_MESSAGE",
            "Binary frame is shorter than 24 bytes",
        )
        .await;
        return;
    }
    let Ok(session_id) = uuid::Uuid::from_slice(&data[..16]) else {
        let _ = send_error(sender, None, "INVALID_MESSAGE", "Invalid session UUID").await;
        return;
    };
    if let Err(error) = state
        .session_registry
        .write_input(&session_id.to_string(), &data[24..])
        .await
    {
        let _ = send_error(sender, None, "SESSION_NOT_FOUND", &error.to_string()).await;
    }
}

async fn dispatch(
    state: &AppState,
    sender: &WsSender,
    attachments: &AttachmentTasks,
    message: ClientMessage,
) -> anyhow::Result<()> {
    match message {
        ClientMessage::Auth { .. } => {
            if state.auth_required() {
                send_error(
                    sender,
                    None,
                    "INVALID_MESSAGE",
                    "Connection is already authenticated",
                )
                .await?;
            } else {
                // Keep auth disabled mode tolerant of clients reconnecting with
                // a token retained from a previously protected server.
                send_auth_ok(state, sender).await?;
            }
        }
        ClientMessage::SessionCreate {
            request_id,
            cwd,
            cols,
            rows,
        } => {
            let cwd = match cwd {
                Some(cwd) => PathBuf::from(cwd),
                None => directory_browser::home()?,
            };
            match state
                .session_registry
                .create_shell_session(cwd, cols, rows)
                .await
            {
                Ok(session) => {
                    send_message(
                        sender,
                        &ServerMessage::SessionCreated {
                            request_id,
                            session: Box::new(session),
                        },
                    )
                    .await?;
                }
                Err(error) => {
                    send_error(
                        sender,
                        Some(&request_id),
                        "SESSION_CREATE_FAILED",
                        &error.to_string(),
                    )
                    .await?;
                }
            }
        }
        ClientMessage::SessionList { request_id } => {
            send_message(
                sender,
                &ServerMessage::SessionList {
                    request_id,
                    sessions: state.session_registry.list().await,
                },
            )
            .await?;
        }
        ClientMessage::DirectoryList {
            request_id,
            path,
            show_hidden,
        } => match directory_browser::list(path, show_hidden).await {
            Ok(listing) => {
                send_message(
                    sender,
                    &ServerMessage::DirectoryListing {
                        request_id,
                        home: listing.home,
                        path: listing.path,
                        parent: listing.parent,
                        entries: listing.entries,
                    },
                )
                .await?;
            }
            Err(error) => {
                send_error(
                    sender,
                    Some(&request_id),
                    "DIRECTORY_LIST_FAILED",
                    &error.to_string(),
                )
                .await?;
            }
        },
        ClientMessage::DirectoryCreate {
            request_id,
            parent,
            name,
        } => match directory_browser::create(parent, name).await {
            Ok(path) => {
                send_message(
                    sender,
                    &ServerMessage::DirectoryCreated { request_id, path },
                )
                .await?;
            }
            Err(error) => {
                send_error(
                    sender,
                    Some(&request_id),
                    "DIRECTORY_CREATE_FAILED",
                    &error.to_string(),
                )
                .await?;
            }
        },
        ClientMessage::SessionAttach {
            request_id,
            session_id,
            last_seq,
        } => {
            stop_attachment(attachments, &session_id).await;
            match attach_output(state, sender, &session_id, last_seq).await {
                Ok(task) => {
                    attachments.lock().await.insert(session_id, task);
                }
                Err(error) => {
                    send_error(
                        sender,
                        Some(&request_id),
                        "SESSION_NOT_FOUND",
                        &error.to_string(),
                    )
                    .await?;
                }
            }
        }
        ClientMessage::SessionDetach { session_id } => {
            stop_attachment(attachments, &session_id).await;
        }
        ClientMessage::SessionDestroy {
            request_id,
            session_id,
        } => {
            stop_attachment(attachments, &session_id).await;
            if state.session_registry.remove(&session_id).await {
                send_message(sender, &ServerMessage::SessionDestroyed { session_id }).await?;
            } else {
                send_error(
                    sender,
                    Some(&request_id),
                    "SESSION_NOT_FOUND",
                    "Session not found",
                )
                .await?;
            }
        }
        ClientMessage::TerminalInput {
            session_id,
            data_base64,
            cols,
            rows,
        } => {
            use base64::Engine;
            match base64::engine::general_purpose::STANDARD.decode(data_base64) {
                Ok(data) => {
                    let size = cols.zip(rows);
                    if let Err(error) = state
                        .session_registry
                        .write_input_at_size(&session_id, &data, size)
                        .await
                    {
                        send_error(sender, None, "SESSION_NOT_FOUND", &error.to_string()).await?;
                    }
                }
                Err(error) => {
                    send_error(sender, None, "INVALID_MESSAGE", &error.to_string()).await?;
                }
            }
        }
        ClientMessage::TerminalResize {
            session_id,
            cols,
            rows,
            active,
        } => {
            // Old clients omitted `active` and remain passive. The focused
            // browser explicitly claims responsibility for the PTY grid so a
            // background page cannot resize a shared full-screen TUI.
            if active {
                if let Err(error) = state.session_registry.resize(&session_id, cols, rows).await {
                    send_error(sender, None, "SESSION_RESIZE_FAILED", &error.to_string()).await?;
                }
            }
        }
        ClientMessage::TerminalTitle { session_id, title } => {
            if let Err(error) = state
                .session_registry
                .set_terminal_title(&session_id, &title)
                .await
            {
                send_error(sender, None, "SESSION_TITLE_FAILED", &error.to_string()).await?;
            }
        }
        ClientMessage::AgentUpdate {
            session_id,
            code_agent,
            session_title,
            status,
            detail,
            mode,
            task_progress,
        } => {
            if let Err(error) = state
                .session_registry
                .apply_agent_update(
                    &session_id,
                    AgentUpdate {
                        code_agent,
                        session_title,
                        status,
                        detail,
                        mode,
                        task_progress,
                    },
                )
                .await
            {
                send_error(sender, None, "AGENT_UPDATE_FAILED", &error.to_string()).await?;
            }
        }
    }
    Ok(())
}

async fn stop_attachment(attachments: &AttachmentTasks, session_id: &str) {
    if let Some(task) = attachments.lock().await.remove(session_id) {
        task.abort();
    }
}

async fn attach_output(
    state: &AppState,
    sender: &WsSender,
    session_id: &str,
    last_seq: Option<u64>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    let mut attachment = state.session_registry.attach(session_id, last_seq).await?;

    if attachment.gap_detected {
        send_message(
            sender,
            &ServerMessage::ServerNotification {
                message: format!("Output history for session {session_id} is incomplete"),
            },
        )
        .await?;
    }
    for chunk in attachment.replay {
        send_terminal_output(sender, chunk).await?;
    }

    let sender = sender.clone();
    Ok(tokio::spawn(async move {
        loop {
            match attachment.live.recv().await {
                Ok(chunk) if chunk.seq >= attachment.live_from_seq => {
                    if send_terminal_output(&sender, chunk).await.is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let _ = send_message(
                        &sender,
                        &ServerMessage::ServerNotification {
                            message: "Terminal output lagged; reattach to recover history"
                                .to_string(),
                        },
                    )
                    .await;
                    break;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }))
}

async fn send_terminal_output(sender: &WsSender, chunk: TerminalChunk) -> anyhow::Result<()> {
    use base64::Engine;
    send_message(
        sender,
        &ServerMessage::TerminalOutput {
            session_id: chunk.session_id,
            seq: chunk.seq,
            data_base64: base64::engine::general_purpose::STANDARD.encode(chunk.data),
        },
    )
    .await
}

async fn send_message(sender: &WsSender, message: &ServerMessage) -> anyhow::Result<()> {
    let json = serde_json::to_string(message)?;
    sender.lock().await.send(Message::Text(json.into())).await?;
    Ok(())
}

async fn send_auth_error(sender: &WsSender, message: &str) -> anyhow::Result<()> {
    send_message(
        sender,
        &ServerMessage::AuthError {
            code: "AUTH_INVALID".to_string(),
            message: message.to_string(),
        },
    )
    .await
}

async fn send_error(
    sender: &WsSender,
    request_id: Option<&str>,
    code: &str,
    message: &str,
) -> anyhow::Result<()> {
    send_message(
        sender,
        &ServerMessage::Error {
            request_id: request_id.map(str::to_string),
            code: code.to_string(),
            message: message.to_string(),
        },
    )
    .await
}

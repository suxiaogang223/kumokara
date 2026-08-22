use kumokara_auth::AuthManager;
use kumokara_server::{serve_listener, AppState};
use serde::Serialize;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use tauri::State;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopConfig {
    local_server_url: String,
    local_server_token: String,
    app_version: String,
}

struct PreparedServer {
    listener: TcpListener,
    auth: AuthManager,
    config: DesktopConfig,
}

fn prepare_local_server() -> std::io::Result<PreparedServer> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let auth = AuthManager::new();
    let config = DesktopConfig {
        local_server_url: format!("http://{address}"),
        local_server_token: auth.server_token().to_owned(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    Ok(PreparedServer {
        listener,
        auth,
        config,
    })
}

#[tauri::command]
fn desktop_config(config: State<'_, DesktopConfig>) -> DesktopConfig {
    config.inner().clone()
}

pub fn run() {
    let prepared = prepare_local_server().expect("failed to reserve a local Kumokara port");
    let config = prepared.config.clone();

    tauri::Builder::default()
        .manage(config)
        .invoke_handler(tauri::generate_handler![desktop_config])
        .setup(move |_app| {
            let listener = prepared.listener;
            let state = AppState::new(Some(prepared.auth));
            tauri::async_runtime::spawn(async move {
                let listener = match tokio::net::TcpListener::from_std(listener) {
                    Ok(listener) => listener,
                    Err(error) => {
                        eprintln!("Kumokara desktop server failed to start: {error}");
                        return;
                    }
                };
                if let Err(error) = serve_listener(listener, state).await {
                    eprintln!("Kumokara desktop server stopped: {error}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Kumokara desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_server_is_private_and_uses_an_ephemeral_loopback_port() {
        let prepared = prepare_local_server().unwrap();
        let address = prepared.listener.local_addr().unwrap();

        assert!(address.ip().is_loopback());
        assert_ne!(address.port(), 0);
        assert_eq!(prepared.config.local_server_token.len(), 64);
        assert!(prepared
            .config
            .local_server_url
            .starts_with("http://127.0.0.1:"));
    }
}

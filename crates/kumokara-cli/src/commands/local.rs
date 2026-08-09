//! Local mode — start server and open browser.
//!
//! Implements the startup experience from DESIGN.md §2:
//! 1. Validate the required tmux runtime
//! 2. Configure optional token authentication
//! 3. Start the server on localhost
//! 4. Open the browser

use anyhow::Result;
use kumokara_server::{serve, AppState};
use std::net::SocketAddr;

/// Banner text printed at startup.
const BANNER: &str = r"
  _  __                     __
 | |/ /_  _  _ __ ___   ___| | ____ _ _ __ __ _
 | ' /| || || | '  \ _ \ / _ \ |/ / _` | '__/ _` |
 | . \ \_,_||_|_|_|_\___/\___/_/\_\__,_|_|  \__,_|
 |_|\_\

 Kumokara（雲殻）— Agents never sleep in Kumokara.
";

/// Run Kumokara in Local mode.
///
/// Starts the server bound to 127.0.0.1:9876 and opens the browser.
pub async fn run_local(require_token: bool) -> Result<()> {
    // Print ASCII banner
    println!("{BANNER}");

    let state = AppState::new(super::configure_auth(require_token))?;
    println!(
        "✓ {} — persistent session runtime ready (screen reconstruction is best-effort)",
        state.tmux_version
    );

    let addr: SocketAddr = "127.0.0.1:9876".parse()?;
    println!("→ Server listening on http://{addr}");
    println!("→ Opening browser...\n");

    // Try to open browser
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(format!("http://{addr}"))
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(format!("http://{}", addr))
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", &format!("http://{}", addr)])
            .spawn();
    }

    serve(addr, state).await?;

    Ok(())
}

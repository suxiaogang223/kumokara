//! portable-pty based PTY session (fallback when tmux is unavailable).
//!
//! Uses the `portable-pty` crate (v0.9+) for cross-platform PTY management.
//! In v0.9, MasterPty no longer has direct `read`/`write` methods — instead,
//! we use `try_clone_reader()` and `take_writer()` to get `std::io` handles.

use anyhow::Result;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use super::PtySession;

/// Spawn a new shell session using portable-pty.
pub(crate) async fn spawn(
    cwd: PathBuf,
    cols: u16,
    rows: u16,
    command: Option<Vec<String>>,
    env: HashMap<String, String>,
) -> Result<PtySession> {
    let pty_system = NativePtySystem::default();

    // Build the command
    let mut cmd = match command {
        Some(args) if !args.is_empty() => {
            let mut command = CommandBuilder::new(&args[0]);
            for arg in &args[1..] {
                command.arg(arg);
            }
            command
        }
        _ => CommandBuilder::new_default_prog(),
    };
    cmd.cwd(cwd.clone());

    // Set environment
    cmd.env("TERM", "xterm-256color");
    for (key, value) in env {
        cmd.env(key, value);
    }

    // Spawn the PTY
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| anyhow::anyhow!("Failed to open PTY: {e}"))?;

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| anyhow::anyhow!("Failed to spawn command in PTY: {e}"))?;
    let process_id = child.process_id();

    // Get reader and writer handles (portable-pty 0.9 API)
    let mut reader: Box<dyn Read + Send> = pair
        .master
        .try_clone_reader()
        .map_err(|e| anyhow::anyhow!("Failed to clone PTY reader: {e}"))?;

    let mut writer: Box<dyn Write + Send> = pair
        .master
        .take_writer()
        .map_err(|e| anyhow::anyhow!("Failed to take PTY writer: {e}"))?;

    // We keep the master around for resize operations
    let master = Arc::new(Mutex::new(pair.master));

    // Set up I/O channels
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, mut resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();

    let master_for_resize = master.clone();

    // Spawn output reading task
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break, // EOF or error
                Ok(n) => {
                    if output_tx.send(buf[..n].to_vec()).is_err() {
                        break; // Receiver dropped
                    }
                }
            }
        }
    });

    // Spawn input forwarding task
    tokio::task::spawn_blocking(move || {
        while let Some(data) = input_rx.blocking_recv() {
            if writer.write_all(&data).is_err() {
                break; // Write error
            }
            let _ = writer.flush();
        }
    });

    // Spawn resize task
    tokio::task::spawn_blocking(move || {
        while let Some((new_cols, new_rows)) = resize_rx.blocking_recv() {
            if let Ok(guard) = master_for_resize.lock() {
                let _ = guard.resize(PtySize {
                    rows: new_rows,
                    cols: new_cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
        }
    });

    // Store the child process for cleanup
    let child = Arc::new(Mutex::new(Some(child)));
    let child_for_cleanup = child.clone();

    let session = PtySession {
        process_id,
        output_rx: Some(output_rx),
        input_tx,
        resize_tx,
        cleanup: Some(Box::new(move || {
            if let Ok(mut guard) = child_for_cleanup.lock() {
                if let Some(mut child) = guard.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        })),
    };

    Ok(session)
}

//! kumokara-engine — PTY session management.
//!
//! Wraps portable-pty for cross-platform terminal sessions, with optional
//! tmux control mode integration for crash recovery (when tmux is available).

pub mod portable;
pub mod tmux;

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// The two backends for PTY sessions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PtyBackend {
    /// tmux control mode — full crash recovery
    Tmux,
    /// portable-pty — no recovery but works everywhere
    Portable,
}

impl std::fmt::Display for PtyBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyBackend::Tmux => write!(f, "tmux"),
            PtyBackend::Portable => write!(f, "portable-pty"),
        }
    }
}

/// A PTY session handle — abstracts over tmux and portable-pty.
pub struct PtySession {
    /// Unique session identifier
    pub id: String,
    /// Which backend is in use
    pub backend: PtyBackend,
    /// Path to the working directory
    pub cwd: PathBuf,
    /// Terminal dimensions
    pub cols: u16,
    pub rows: u16,

    /// Channel for receiving output from the PTY
    output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Channel for sending input to the PTY
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    /// Channel for sending resize commands
    resize_tx: Option<mpsc::UnboundedSender<(u16, u16)>>,
    /// For cleanup on drop
    _cleanup: Option<Box<dyn FnOnce() + Send>>,
}

impl PtySession {
    /// Spawn a new PTY session. Automatically selects the best available backend.
    pub async fn spawn(
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        command: Option<Vec<String>>,
    ) -> Result<Self> {
        let has_tmux = tmux::has_tmux();

        if has_tmux {
            tracing::info!("tmux detected, using tmux backend for session recovery");
            tracing::warn!("tmux control mode not yet implemented in Phase 0, using portable-pty");
        }

        portable::spawn_portable(cwd, cols, rows, command).await
    }

    /// Get the backend used for this session.
    pub fn backend(&self) -> PtyBackend {
        self.backend
    }

    /// Send input (keystrokes) to the PTY.
    pub fn write_input(&self, data: &[u8]) -> Result<()> {
        if let Some(ref tx) = self.input_tx {
            tx.send(data.to_vec())
                .map_err(|_| anyhow::anyhow!("PTY input channel closed"))?;
        }
        Ok(())
    }

    /// Resize the terminal.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        if let Some(ref tx) = self.resize_tx {
            tx.send((cols, rows))
                .map_err(|_| anyhow::anyhow!("PTY resize channel closed"))?;
        }
        Ok(())
    }

    /// Get a mutable reference to the output receiver.
    pub fn output_rx(&mut self) -> &mut mpsc::UnboundedReceiver<Vec<u8>> {
        &mut self.output_rx
    }

    /// Take ownership of the output receiver (for moving to a background task).
    pub fn take_output_rx(&mut self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
        Some(std::mem::replace(
            &mut self.output_rx,
            mpsc::unbounded_channel().1,
        ))
    }

    /// Clone the input sender (for storing in the registry).
    pub fn input_sender(&self) -> Option<mpsc::UnboundedSender<Vec<u8>>> {
        self.input_tx.clone()
    }

    /// Clone the resize sender (for storing in the registry).
    pub fn resize_sender(&self) -> Option<mpsc::UnboundedSender<(u16, u16)>> {
        self.resize_tx.clone()
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Some(cleanup) = self._cleanup.take() {
            cleanup();
        }
    }
}

/// Check if tmux is available on the system and get its version.
pub fn detect_tmux() -> Option<String> {
    tmux::detect_tmux()
}

/// Check if tmux is available (boolean).
pub fn has_tmux() -> bool {
    tmux::has_tmux()
}

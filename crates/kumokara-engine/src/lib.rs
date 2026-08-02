//! PTY ownership and tmux capability detection.

mod portable;
mod tmux;

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// A running portable PTY. Dropping it terminates and waits for its child.
pub struct PtySession {
    process_id: Option<u32>,
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    resize_tx: mpsc::UnboundedSender<(u16, u16)>,
    cleanup: Option<Box<dyn FnOnce() + Send>>,
}

impl PtySession {
    pub async fn spawn(
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        command: Option<Vec<String>>,
        env: HashMap<String, String>,
    ) -> Result<Self> {
        portable::spawn(cwd, cols, rows, command, env).await
    }

    pub fn process_id(&self) -> Option<u32> {
        self.process_id
    }

    pub fn write_input(&self, data: &[u8]) -> Result<()> {
        self.input_tx
            .send(data.to_vec())
            .map_err(|_| anyhow::anyhow!("PTY input channel closed"))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.resize_tx
            .send((cols, rows))
            .map_err(|_| anyhow::anyhow!("PTY resize channel closed"))
    }

    pub fn take_output_rx(&mut self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
        self.output_rx.take()
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

pub fn detect_tmux() -> Option<String> {
    tmux::detect_tmux()
}

pub fn has_tmux() -> bool {
    tmux::has_tmux()
}

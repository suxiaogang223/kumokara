//! Process-owned PTY session runtime.

use anyhow::{anyhow, Result};
use portable_pty::{Child, CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use tokio::sync::mpsc;

enum PtyCommand {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

/// A PTY owned by the Kumokara server process.
///
/// Browser attachments may come and go without affecting the child. Dropping
/// the server-side handle terminates and waits for the child process.
pub struct PtySession {
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    command_tx: mpsc::UnboundedSender<PtyCommand>,
    child: Option<Box<dyn Child + Send + Sync>>,
}

impl PtySession {
    pub fn spawn(
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        command: Option<Vec<String>>,
        env: HashMap<String, String>,
    ) -> Result<Self> {
        let pty_system = NativePtySystem::default();
        let mut command = build_command(command);
        command.cwd(cwd);
        for (key, value) in env {
            command.env(key, value);
        }

        // Service runners may expose TERM=dumb or NO_COLOR. The browser is
        // backed by xterm.js, so describe the PTY created here instead of the
        // environment that happened to launch Kumokara.
        command.env_remove("NO_COLOR");
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("CLICOLOR", "1");

        let pair = pty_system
            .openpty(pty_size(cols, rows))
            .map_err(|error| anyhow!("failed to open PTY: {error}"))?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| anyhow!("failed to spawn command in PTY: {error}"))?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| anyhow!("failed to clone PTY reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| anyhow!("failed to take PTY writer: {error}"))?;
        let master = pair.master;

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        spawn_output_reader(reader, output_tx);
        spawn_command_writer(writer, master, command_rx);

        Ok(Self {
            output_rx: Some(output_rx),
            command_tx,
            child: Some(child),
        })
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.as_ref().and_then(|child| child.process_id())
    }

    pub fn write_input(&self, data: &[u8]) -> Result<()> {
        self.command_tx
            .send(PtyCommand::Input(data.to_vec()))
            .map_err(|_| anyhow!("PTY input channel closed"))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.command_tx
            .send(PtyCommand::Resize { cols, rows })
            .map_err(|_| anyhow!("PTY resize channel closed"))
    }

    pub fn take_output_rx(&mut self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
        self.output_rx.take()
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn build_command(args: Option<Vec<String>>) -> CommandBuilder {
    let Some(mut args) = args.filter(|args| !args.is_empty()) else {
        return CommandBuilder::new_default_prog();
    };

    let mut command = CommandBuilder::new(args.remove(0));
    command.args(args);
    command
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn spawn_output_reader(
    mut reader: Box<dyn Read + Send>,
    output_tx: mpsc::UnboundedSender<Vec<u8>>,
) {
    tokio::task::spawn_blocking(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) if output_tx.send(buffer[..read].to_vec()).is_err() => break,
                Ok(_) => {}
            }
        }
    });
}

fn spawn_command_writer(
    mut writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    mut command_rx: mpsc::UnboundedReceiver<PtyCommand>,
) {
    tokio::task::spawn_blocking(move || {
        while let Some(command) = command_rx.blocking_recv() {
            match command {
                PtyCommand::Input(data) => {
                    if writer.write_all(&data).is_err() || writer.flush().is_err() {
                        break;
                    }
                }
                PtyCommand::Resize { cols, rows } => {
                    if let Err(error) = master.resize(pty_size(cols, rows)) {
                        tracing::warn!(%error, "failed to resize PTY");
                    }
                }
            }
        }
    });
}

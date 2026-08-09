//! tmux control mode integration for persistent terminal sessions.
//!
//! Kumokara creates every terminal session inside its dedicated tmux server.
//! tmux owns the shell process, so Kumokara server restarts no longer terminate
//! running agents.
//!
//! # Architecture
//!
//! ```text
//! Kumokara Server
//!     │
//!     ├── tmux new-session -d -s kumokara_<id>   (create detached session)
//!     ├── tmux set-option @kumokara_*              (metadata as user options)
//!     └── tmux -C attach -t kumokara_<id>         (control mode I/O)
//!           │
//!           ├── stdout: parse %output notifications  → output channel
//!           └── stdin:  send-keys / refresh-client   ← input / resize channels
//! ```
//!
//! # Recovery
//!
//! On server restart, tagged sessions are re-attached through control mode.
//! Kumokara reconstructs the visible pane and explicitly reports an output
//! history gap; it does not claim to preserve the previous process's sequence
//! numbers or a complete VT state snapshot.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

const META_PREFIX: &str = "@kumokara";
const DEFAULT_SOCKET_NAME: &str = "kumokara";
const MINIMUM_TMUX_VERSION: (u16, u16) = (3, 2);

/// Kumokara's tmux runtime.
///
/// Production uses a Kumokara-owned tmux server. Tests provide a unique socket
/// name so neither environment inspects or mutates the user's tmux sessions.
#[derive(Clone, Debug)]
pub struct Tmux {
    socket_name: String,
}

impl Default for Tmux {
    fn default() -> Self {
        Self::new(DEFAULT_SOCKET_NAME)
    }
}

pub struct PaneSnapshot {
    pub bytes: Vec<u8>,
    pub content_present: bool,
}

/// A control connection to one tmux-owned shell session.
///
/// Dropping this handle detaches Kumokara. The shell continues running in tmux
/// until `Tmux::kill_session` is called explicitly.
pub struct TmuxSession {
    process_id: Option<u32>,
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    resize_tx: mpsc::UnboundedSender<(u16, u16)>,
    name: String,
    _control: TmuxControl,
}

impl TmuxSession {
    pub fn create(
        tmux: &Tmux,
        name: String,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        env: HashMap<String, String>,
    ) -> Result<Self> {
        tmux.create_session(&name, &cwd, cols, rows, &env)?;
        match Self::connect(tmux, name.clone(), cols, rows) {
            Ok(session) => Ok(session),
            Err(error) => {
                let _ = tmux.kill_session(&name);
                Err(error)
            }
        }
    }

    pub fn attach(tmux: &Tmux, name: String, cols: u16, rows: u16) -> Result<Self> {
        Self::connect(tmux, name, cols, rows)
    }

    fn connect(tmux: &Tmux, name: String, cols: u16, rows: u16) -> Result<Self> {
        let process_id = tmux.get_pane_pid(&name).ok();
        let pane_id = tmux.get_pane_id(&name)?;
        let mut control = tmux.attach_control(&name)?;
        let stdout = control
            .take_stdout()
            .ok_or_else(|| anyhow::anyhow!("tmux control mode stdout missing"))?;
        let stdin = control
            .take_stdin()
            .ok_or_else(|| anyhow::anyhow!("tmux control mode stdin missing"))?;
        let stdin = Arc::new(Mutex::new(BufWriter::new(stdin)));

        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (resize_tx, mut resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);

        tokio::task::spawn_blocking(move || {
            let mut parser = TmuxOutputParser::new(stdout);
            loop {
                match parser.read_notification() {
                    Ok(Some(ControlNotification::Output(data))) => {
                        if output_tx.send(data).is_err() {
                            break;
                        }
                    }
                    Ok(Some(ControlNotification::SessionChanged)) => {
                        let _ = ready_tx.try_send(());
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!(%error, "tmux control mode read error");
                        break;
                    }
                }
            }
        });

        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| anyhow::anyhow!("tmux control mode did not become ready: {error}"))?;

        let stdin_for_input = stdin.clone();
        tokio::task::spawn_blocking(move || {
            while let Some(data) = input_rx.blocking_recv() {
                let mut writer = match stdin_for_input.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if write_send_keys_hex(&mut *writer, &pane_id, &data)
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        });

        tokio::task::spawn_blocking(move || {
            while let Some((new_cols, new_rows)) = resize_rx.blocking_recv() {
                let mut writer = match stdin.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                if writeln!(writer, "refresh-client -C {new_cols}x{new_rows}").is_err()
                    || writer.flush().is_err()
                {
                    break;
                }
            }
        });

        let session = Self {
            process_id,
            output_rx: Some(output_rx),
            input_tx,
            resize_tx,
            name,
            _control: control,
        };
        session.resize(cols, rows)?;
        Ok(session)
    }

    pub fn process_id(&self) -> Option<u32> {
        self.process_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn write_input(&self, data: &[u8]) -> Result<()> {
        self.input_tx
            .send(data.to_vec())
            .map_err(|_| anyhow::anyhow!("tmux input channel closed"))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.resize_tx
            .send((cols, rows))
            .map_err(|_| anyhow::anyhow!("tmux resize channel closed"))
    }

    pub fn take_output_rx(&mut self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
        self.output_rx.take()
    }
}

impl Tmux {
    pub fn new(socket_name: impl Into<String>) -> Self {
        Self {
            socket_name: socket_name.into(),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("tmux");
        command.args(["-L", &self.socket_name]);
        command
    }

    pub fn require_version(&self) -> Result<String> {
        let output = self
            .command()
            .arg("-V")
            .output()
            .with_context(|| "tmux is required but was not found on PATH")?;
        if !output.status.success() {
            bail!("tmux -V exited with {}", output.status);
        }

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let parsed = parse_tmux_version(&version)
            .with_context(|| format!("unable to parse required tmux version from '{version}'"))?;
        if parsed < MINIMUM_TMUX_VERSION {
            bail!(
                "tmux {}.{} or newer is required; found {version}",
                MINIMUM_TMUX_VERSION.0,
                MINIMUM_TMUX_VERSION.1
            );
        }
        Ok(version)
    }

    fn create_session(
        &self,
        name: &str,
        cwd: &Path,
        cols: u16,
        rows: u16,
        env: &HashMap<String, String>,
    ) -> Result<()> {
        let mut command = self.command();
        command.args([
            "new-session",
            "-d",
            "-s",
            name,
            "-x",
            &cols.to_string(),
            "-y",
            &rows.to_string(),
        ]);
        for (key, value) in env {
            command.args(["-e", &format!("{key}={value}")]);
        }
        let status = command
            .arg("-c")
            .arg(cwd)
            .status()
            .with_context(|| "failed to spawn tmux new-session")?;

        if !status.success() {
            bail!("tmux new-session exited with {status}");
        }
        Ok(())
    }

    pub fn kill_session(&self, name: &str) -> Result<()> {
        let status = self
            .command()
            .args(["kill-session", "-t", name])
            .status()
            .with_context(|| format!("failed to kill tmux session '{name}'"))?;

        if !status.success() {
            bail!("tmux kill-session exited with {status}");
        }
        Ok(())
    }

    pub fn kill_server(&self) -> Result<()> {
        let status = self
            .command()
            .arg("kill-server")
            .stderr(Stdio::null())
            .status()
            .with_context(|| "failed to kill tmux server")?;
        if !status.success() {
            bail!("tmux kill-server exited with {status}");
        }
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let output = self
            .command()
            .args(["list-sessions", "-F", "#{session_name}"])
            .stderr(Stdio::null())
            .output()
            .with_context(|| "failed to list tmux sessions")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect())
    }

    pub fn session_exists(&self, name: &str) -> bool {
        self.command()
            .args(["has-session", "-t", name])
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub fn set_session_metadata(&self, name: &str, key: &str, value: &str) -> Result<()> {
        let option = format!("{META_PREFIX}_{key}");
        let status = self
            .command()
            .args(["set-option", "-t", name, &option, value])
            .status()
            .with_context(|| format!("failed to set metadata '{key}' on session '{name}'"))?;

        if !status.success() {
            bail!("tmux set-option exited with {status}");
        }
        Ok(())
    }

    pub fn get_session_metadata(&self, name: &str, key: &str) -> Result<Option<String>> {
        let option = format!("{META_PREFIX}_{key}");
        let output = self
            .command()
            .args(["show-options", "-t", name, "-v", &option])
            .output()
            .with_context(|| format!("failed to get metadata '{key}' from session '{name}'"))?;

        if !output.status.success() {
            return Ok(None);
        }

        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok((!value.is_empty()).then_some(value))
    }

    fn get_pane_pid(&self, name: &str) -> Result<u32> {
        let output = self
            .command()
            .args(["display-message", "-t", name, "-p", "#{pane_pid}"])
            .output()
            .with_context(|| format!("failed to get pane PID for session '{name}'"))?;

        if !output.status.success() {
            bail!("tmux display-message exited with {}", output.status);
        }

        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .with_context(|| "failed to parse pane PID")
    }

    fn get_pane_id(&self, name: &str) -> Result<String> {
        let output = self
            .command()
            .args(["display-message", "-t", name, "-p", "#{pane_id}"])
            .output()
            .with_context(|| format!("failed to get pane ID for session '{name}'"))?;

        if !output.status.success() {
            bail!("tmux display-message exited with {}", output.status);
        }

        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if pane_id.starts_with('%') {
            Ok(pane_id)
        } else {
            bail!("invalid tmux pane ID: {pane_id}")
        }
    }

    /// Build a best-effort ANSI reconstruction of the visible pane.
    ///
    /// This intentionally captures only the visible screen, not scrollback.
    /// The leading RIS makes it a standalone replacement screen rather than
    /// pretending that captured text is an incremental PTY byte stream.
    pub fn capture_visible_snapshot(&self, name: &str) -> Result<PaneSnapshot> {
        let state = self
            .command()
            .args([
                "display-message",
                "-t",
                name,
                "-p",
                "#{alternate_on}:#{cursor_x}:#{cursor_y}:#{cursor_flag}",
            ])
            .output()
            .with_context(|| format!("failed to inspect pane state for session '{name}'"))?;
        if !state.status.success() {
            bail!("tmux display-message exited with {}", state.status);
        }

        let output = self
            .command()
            .args(["capture-pane", "-t", name, "-p", "-e", "-N"])
            .output()
            .with_context(|| format!("failed to capture visible pane for session '{name}'"))?;
        if !output.status.success() {
            bail!("tmux capture-pane exited with {}", output.status);
        }

        let state = String::from_utf8_lossy(&state.stdout);
        let mut fields = state.trim().split(':');
        let alternate_on = fields.next() == Some("1");
        let cursor_x = fields
            .next()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        let cursor_y = fields
            .next()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        let cursor_visible = fields.next() != Some("0");

        let content_present = output.stdout.iter().any(|byte| !byte.is_ascii_whitespace());
        let mut bytes = Vec::with_capacity(output.stdout.len() + 48);
        bytes.extend_from_slice(b"\x1bc");
        if alternate_on {
            bytes.extend_from_slice(b"\x1b[?1049h");
        }
        bytes.extend_from_slice(b"\x1b[H");
        bytes.extend_from_slice(&output.stdout);
        write!(bytes, "\x1b[{};{}H", cursor_y + 1, cursor_x + 1)?;
        bytes.extend_from_slice(if cursor_visible {
            b"\x1b[?25h"
        } else {
            b"\x1b[?25l"
        });
        Ok(PaneSnapshot {
            bytes,
            content_present,
        })
    }

    fn attach_control(&self, session_name: &str) -> Result<TmuxControl> {
        if !self.session_exists(session_name) {
            bail!("tmux session does not exist: {session_name}");
        }
        let child = self
            .command()
            .args(["-C", "attach", "-t", session_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| {
                format!("failed to attach control mode to session '{session_name}'")
            })?;
        Ok(TmuxControl { child })
    }
}

fn parse_tmux_version(output: &str) -> Option<(u16, u16)> {
    let token = output.split_whitespace().nth(1)?;
    let start = token.find(|character: char| character.is_ascii_digit())?;
    let numeric = &token[start..];
    let (major, minor) = numeric.split_once('.')?;
    let minor = minor
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    Some((major.parse().ok()?, minor.parse().ok()?))
}

// ---------------------------------------------------------------------------
// Control mode I/O
// ---------------------------------------------------------------------------

/// A connected tmux control-mode client.
///
/// Created by running `tmux -C attach -t <session>`. Stdout carries
/// `%output` notifications; stdin accepts `send-keys` and `refresh-client`
/// commands.
struct TmuxControl {
    child: Child,
}

impl TmuxControl {
    /// Take the stdout handle for reading `%output` notifications.
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.child.stdout.take().map(|stdout| {
            let reader: Box<dyn Read + Send> = Box::new(stdout);
            reader
        })
    }

    /// Take the stdin handle for writing `send-keys` and other commands.
    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>> {
        self.child.stdin.take().map(|stdin| {
            let writer: Box<dyn Write + Send> = Box::new(stdin);
            writer
        })
    }
}

impl Drop for TmuxControl {
    fn drop(&mut self) {
        // Best-effort: kill the control mode child so we don't leave
        // a zombie. The tmux session itself is unaffected.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// %output notification parser
// ---------------------------------------------------------------------------

/// A parsed control-mode notification.
#[derive(Debug, PartialEq)]
enum ControlNotification {
    /// `%output <pane-id> <value>` — terminal output from a pane.
    Output(Vec<u8>),
    /// The initial `%session-changed` event marks the control client ready.
    SessionChanged,
    /// `%begin <timestamp>` or `%end <timestamp>` — notification block markers.
    BlockMarker,
    /// Any other `%`-prefixed line (session-changed, window-add, etc.).
    Other,
}

/// Decode a `%output` value from tmux control mode.
///
/// tmux escapes non-printable bytes as octal (`\012` = LF, `\015` = CR,
/// `\134` = backslash). Printable bytes (including multi-byte UTF-8)
/// pass through unchanged. Because newlines are escaped, `read_line`
/// is a safe framing primitive for control mode stdout.
///
/// Returns `None` if the line does not start with `%output`.
pub fn decode_output(line: &str) -> Option<Vec<u8>> {
    let rest = line.strip_prefix("%output ")?;
    // Skip the pane-id token (e.g. "%0", "%1")
    let value = rest.split_once(' ').map(|(_, v)| v).unwrap_or(rest);
    Some(octal_unescape(value))
}

/// Unescape tmux octal sequences: `\012` → `\n`, `\134` → `\\`, etc.
fn octal_unescape(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            // Try to parse exactly 3 octal digits
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 4]).unwrap_or(""), 8)
            {
                out.push(byte);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Stateful parser for tmux control mode output.
///
/// Wraps a `BufRead` and produces `ControlNotification` values one at a time.
/// Because tmux octal-escapes newlines in `%output`, this parser can safely
/// use `read_line` as its framing primitive.
struct TmuxOutputParser {
    reader: BufReader<Box<dyn Read + Send>>,
}

impl TmuxOutputParser {
    /// Wrap a reader (typically `TmuxControl::take_stdout()`).
    fn new(reader: Box<dyn Read + Send>) -> Self {
        Self {
            reader: BufReader::new(reader),
        }
    }

    /// Read the next complete notification.
    ///
    /// Returns `Ok(None)` on EOF.
    fn read_notification(&mut self) -> Result<Option<ControlNotification>> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .reader
                .read_line(&mut line)
                .with_context(|| "failed to read from tmux control mode stdout")?;
            if n == 0 {
                return Ok(None); // EOF
            }

            // Strip the protocol-level trailing \n. tmux octal-escapes
            // embedded newlines as \012, so any literal \n in the line
            // is the control mode message delimiter.
            let trimmed = line.strip_suffix('\n').unwrap_or(&line);

            if trimmed.starts_with("%output ") {
                // Octal-unescape the value (handles \012 = newline, etc.)
                let data = decode_output(trimmed).unwrap_or_default();
                return Ok(Some(ControlNotification::Output(data)));
            } else if trimmed.starts_with("%session-changed ") {
                return Ok(Some(ControlNotification::SessionChanged));
            } else if trimmed.starts_with("%begin") || trimmed.starts_with("%end") {
                return Ok(Some(ControlNotification::BlockMarker));
            } else if trimmed.starts_with('%') {
                return Ok(Some(ControlNotification::Other));
            }
            // Ignore non-% lines (should not normally appear in control mode).
        }
    }
}

// ---------------------------------------------------------------------------
// Input helpers
// ---------------------------------------------------------------------------

/// Write terminal input as hexadecimal bytes understood by `send-keys -H`.
///
/// Unlike `send-keys -l`, this is independent of tmux command quoting and
/// preserves spaces, quotes, control bytes, and non-UTF-8 input exactly.
fn write_send_keys_hex<W: Write + ?Sized>(
    writer: &mut W,
    pane_id: &str,
    data: &[u8],
) -> io::Result<()> {
    const BYTES_PER_COMMAND: usize = 128;

    for chunk in data.chunks(BYTES_PER_COMMAND) {
        write!(writer, "send-keys -t {pane_id} -H")?;
        for byte in chunk {
            write!(writer, " {byte:02x}")?;
        }
        writer.write_all(b"\n")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tmux_uses_kumokara_owned_socket() {
        assert_eq!(Tmux::default().socket_name, DEFAULT_SOCKET_NAME);
    }

    #[test]
    fn parses_release_and_development_versions() {
        assert_eq!(parse_tmux_version("tmux 3.2"), Some((3, 2)));
        assert_eq!(parse_tmux_version("tmux 3.6a"), Some((3, 6)));
        assert_eq!(parse_tmux_version("tmux next-3.8"), Some((3, 8)));
        assert_eq!(parse_tmux_version("tmux master"), None);
    }

    // -----------------------------------------------------------------------
    // decode_output (octal unescaping)
    // -----------------------------------------------------------------------

    #[test]
    fn decode_plain_text() {
        assert_eq!(
            decode_output("%output %1 hello world"),
            Some(b"hello world".to_vec())
        );
    }

    #[test]
    fn decode_with_octal_newline() {
        // tmux escapes \n as \012
        assert_eq!(
            decode_output("%output %1 hello\\012world"),
            Some(b"hello\nworld".to_vec())
        );
    }

    #[test]
    fn decode_with_octal_cr() {
        assert_eq!(
            decode_output("%output %1 line\\015"),
            Some(b"line\r".to_vec())
        );
    }

    #[test]
    fn decode_with_octal_backslash() {
        // \\ is not an escape, \134 is backslash
        assert_eq!(
            decode_output("%output %1 path\\134bin"),
            Some(b"path\\bin".to_vec())
        );
    }

    #[test]
    fn decode_ansi_escape_sequence() {
        assert_eq!(
            decode_output("%output %1 \\033[31mRED\\033[0m"),
            Some(b"\x1b[31mRED\x1b[0m".to_vec())
        );
    }

    #[test]
    fn decode_utf8_passthrough() {
        // Multi-byte UTF-8 passes through unchanged
        let input = "%output %1 🎉 hello 世界";
        let result = decode_output(input).unwrap();
        let text = String::from_utf8(result).unwrap();
        assert!(text.contains("🎉"));
        assert!(text.contains("世界"));
    }

    #[test]
    fn decode_empty_value() {
        assert_eq!(decode_output("%output %1 "), Some(b"".to_vec()));
    }

    #[test]
    fn decode_not_an_output_line() {
        assert_eq!(decode_output("%begin 1234"), None);
        assert_eq!(decode_output("%end 1234"), None);
        assert_eq!(decode_output("plain text"), None);
    }

    // -----------------------------------------------------------------------
    // TmuxOutputParser (line-at-a-time)
    // -----------------------------------------------------------------------

    fn make_parser(data: &[u8]) -> TmuxOutputParser {
        TmuxOutputParser::new(Box::new(std::io::Cursor::new(data.to_vec())))
    }

    #[test]
    fn parse_simple_output() {
        let mut parser = make_parser(b"%output %1 hello world\n");
        let notif = parser.read_notification().unwrap().unwrap();
        assert_eq!(notif, ControlNotification::Output(b"hello world".to_vec()));
    }

    #[test]
    fn parse_output_with_octal_newline() {
        // Real tmux output: echo -e "hello\nworld" produces
        // %output %1 hello\012world\012
        let mut parser = make_parser(b"%output %1 hello\\012world\\012\n");
        let notif = parser.read_notification().unwrap().unwrap();
        assert_eq!(
            notif,
            ControlNotification::Output(b"hello\nworld\n".to_vec())
        );
    }

    #[test]
    fn parse_begin_and_end_blocks() {
        let mut parser = make_parser(b"%begin 1234\n%output %1 data\n%end 1234\n");

        assert_eq!(
            parser.read_notification().unwrap().unwrap(),
            ControlNotification::BlockMarker
        );
        assert_eq!(
            parser.read_notification().unwrap().unwrap(),
            ControlNotification::Output(b"data".to_vec())
        );
        assert_eq!(
            parser.read_notification().unwrap().unwrap(),
            ControlNotification::BlockMarker
        );
    }

    #[test]
    fn parse_control_client_ready_event() {
        let mut parser = make_parser(b"%session-changed $0 kumokara-test\n");
        assert_eq!(
            parser.read_notification().unwrap().unwrap(),
            ControlNotification::SessionChanged
        );
    }

    #[test]
    fn parse_sequential_outputs() {
        let mut parser = make_parser(b"%output %1 first\n%output %1 second\n");
        assert_eq!(
            parser.read_notification().unwrap().unwrap(),
            ControlNotification::Output(b"first".to_vec())
        );
        assert_eq!(
            parser.read_notification().unwrap().unwrap(),
            ControlNotification::Output(b"second".to_vec())
        );
    }

    #[test]
    fn parse_empty_output() {
        let mut parser = make_parser(b"%output %1 \n");
        let notif = parser.read_notification().unwrap().unwrap();
        assert_eq!(notif, ControlNotification::Output(b"".to_vec()));
    }

    #[test]
    fn parse_eof_after_output() {
        let mut parser = make_parser(b"%output %1 trailing");
        let notif = parser.read_notification().unwrap().unwrap();
        assert_eq!(notif, ControlNotification::Output(b"trailing".to_vec()));
        assert!(parser.read_notification().unwrap().is_none());
    }

    // -----------------------------------------------------------------------
    // write_send_keys_hex
    // -----------------------------------------------------------------------

    #[test]
    fn input_is_encoded_as_exact_hex_bytes() {
        let mut commands = Vec::new();
        write_send_keys_hex(&mut commands, "%42", b"echo 'hello world'\r").unwrap();
        assert_eq!(
            String::from_utf8(commands).unwrap(),
            "send-keys -t %42 -H 65 63 68 6f 20 27 68 65 6c 6c 6f 20 77 6f 72 6c 64 27 0d\n"
        );
    }

    #[test]
    fn input_encoding_preserves_all_byte_values() {
        let data = (0..=u8::MAX).collect::<Vec<_>>();
        let mut commands = Vec::new();
        write_send_keys_hex(&mut commands, "%42", &data).unwrap();

        let encoded = String::from_utf8(commands).unwrap();
        let decoded = encoded
            .lines()
            .flat_map(|line| {
                line.strip_prefix("send-keys -t %42 -H ")
                    .unwrap()
                    .split(' ')
            })
            .map(|value| u8::from_str_radix(value, 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(decoded, data);
    }

    #[test]
    fn empty_input_writes_no_command() {
        let mut commands = Vec::new();
        write_send_keys_hex(&mut commands, "%42", b"").unwrap();
        assert!(commands.is_empty());
    }
}

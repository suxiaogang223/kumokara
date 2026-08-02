//! OSC 133 / OSC 7 sequence parser.
//!
//! Parses the FTCS (Final Term Command Sequences) and OSC 7 working directory
//! sequences injected by the shell integration scripts. These form the data
//! foundation for badges, outlines, notifications, and audit.

use kumokara_protocol::event::Event;

/// Result of parsing a chunk of terminal output.
#[derive(Debug, Clone)]
pub enum ParseResult {
    /// No OSC sequence found — pass through as plain output.
    Passthrough(Vec<u8>),
    /// An OSC sequence was recognized and converted to an event.
    Event(Event),
    /// Both an event and remaining plain output.
    Mixed {
        event: Event,
        remaining: Vec<u8>,
    },
}

/// Parse a raw terminal output chunk for OSC 133/7 sequences.
///
/// Phase 0: basic pattern matching for the standard sequences.
/// Phase 1+: full state-machine parser with partial-sequence buffering.
pub fn parse_output_chunk(
    session_id: &str,
    workspace_id: &str,
    data: &[u8],
) -> Vec<ParseResult> {
    let mut results = Vec::new();
    let text = String::from_utf8_lossy(data);
    let text = text.as_ref();

    // Look for OSC sequences: ESC ] <code> ; <params> <ST|BEL>
    // ESC = \x1b, ST = ESC \ = \x1b\\, BEL = \x07

    let mut remaining = text;
    while let Some(osc_start) = remaining.find("\x1b]") {
        // Emit any text before the OSC sequence as passthrough
        if osc_start > 0 {
            results.push(ParseResult::Passthrough(
                remaining[..osc_start].as_bytes().to_vec(),
            ));
        }

        let after_osc = &remaining[osc_start + 2..]; // skip ESC ]
        let osc_end = after_osc.find('\x07')
            .or_else(|| {
                after_osc
                    .find("\x1b\\")
                    .map(|i| i) // ST terminator
            });

        if let Some(end) = osc_end {
            let params = &after_osc[..end];
            let rest_start = if after_osc.as_bytes().get(end) == Some(&0x07) {
                end + 1
            } else {
                end + 2 // ESC \ is 2 bytes
            };

            // Parse the OSC parameters
            if let Some(event) = parse_osc_sequence(session_id, workspace_id, params) {
                // Check if there's remaining data after this sequence
                let rest = &after_osc[rest_start..];
                if !rest.is_empty() {
                    results.push(ParseResult::Mixed {
                        event,
                        remaining: rest.as_bytes().to_vec(),
                    });
                } else {
                    results.push(ParseResult::Event(event));
                }
            }

            remaining = &after_osc[rest_start..];
        } else {
            // Incomplete sequence — buffer it (Phase 1 feature)
            // For now, emit as passthrough
            results.push(ParseResult::Passthrough(
                remaining.as_bytes().to_vec(),
            ));
            break;
        }
    }

    // Remaining plain text after last OSC
    if !remaining.is_empty() && results.is_empty() {
        results.push(ParseResult::Passthrough(remaining.as_bytes().to_vec()));
    }

    results
}

/// Parse a single OSC parameter string into an Event, if recognized.
fn parse_osc_sequence(
    session_id: &str,
    _workspace_id: &str,
    params: &str,
) -> Option<Event> {
    // Split into code and args: "133;C" or "7;file://..."
    let semicolon_pos = params.find(';')?;
    let code = &params[..semicolon_pos];
    let args = &params[semicolon_pos + 1..];

    match code {
        // OSC 133 ; C — command started (pre-exec)
        "133" if args.starts_with('C') => {
            let cmd = if args.len() > 1 { &args[1..].trim() } else { "" };
            Some(Event::CommandStarted {
                session_id: session_id.to_string(),
                command: cmd.to_string(),
                cwd: None,
            })
        }

        // OSC 133 ; D ; <exit_code> — command finished
        "133" if args.starts_with('D') => {
            let exit_part = if args.len() > 1 { &args[1..].trim() } else { "" };
            let exit_code = exit_part
                .strip_prefix(';')
                .unwrap_or(exit_part)
                .trim()
                .parse::<i32>()
                .unwrap_or(0);
            Some(Event::CommandFinished {
                session_id: session_id.to_string(),
                exit_code,
                duration_ms: 0,
            })
        }

        // OSC 7 ; file://<host><path> — cwd changed
        "7" => {
            let path = args
                .strip_prefix("file://")
                .and_then(|s| s.split_once('/'))
                .map(|(_, path)| format!("/{}", path))
                .unwrap_or_else(|| args.to_string());
            Some(Event::CwdChanged {
                session_id: session_id.to_string(),
                path,
            })
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_started() {
        let result = parse_output_chunk(
            "sess-1", "ws-1",
            b"\x1b]133;C\x07",
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            ParseResult::Event(Event::CommandStarted { session_id, command, .. }) => {
                assert_eq!(session_id, "sess-1");
                assert!(command.is_empty()); // no command text after C
            }
            _ => panic!("Expected CommandStarted event"),
        }
    }

    #[test]
    fn test_parse_command_finished() {
        let result = parse_output_chunk(
            "sess-1", "ws-1",
            b"\x1b]133;D;0\x07",
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            ParseResult::Event(Event::CommandFinished { session_id, exit_code, .. }) => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(*exit_code, 0);
            }
            _ => panic!("Expected CommandFinished event"),
        }
    }

    #[test]
    fn test_parse_cwd_changed() {
        let result = parse_output_chunk(
            "sess-1", "ws-1",
            b"\x1b]7;file://host/home/user\x07",
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            ParseResult::Event(Event::CwdChanged { session_id, path }) => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(path, "/home/user");
            }
            _ => panic!("Expected CwdChanged event"),
        }
    }

    #[test]
    fn test_parse_mixed_output() {
        let result = parse_output_chunk(
            "sess-1", "ws-1",
            b"hello\x1b]133;D;1\x07world",
        );
        // Should have passthrough for "hello" and Mixed for D;1 with remaining "world"
        assert!(result.len() >= 1);
    }

    #[test]
    fn test_parse_no_osc() {
        let result = parse_output_chunk(
            "sess-1", "ws-1",
            b"plain output no sequences",
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            ParseResult::Passthrough(data) => {
                assert_eq!(data, b"plain output no sequences");
            }
            _ => panic!("Expected Passthrough"),
        }
    }
}

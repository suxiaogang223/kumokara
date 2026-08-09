//! tmux-owned terminal session runtime.
//!
//! tmux is a required runtime dependency. It owns every shell process and PTY;
//! Kumokara connects through control mode so server restarts do not terminate
//! active sessions.

pub mod tmux;

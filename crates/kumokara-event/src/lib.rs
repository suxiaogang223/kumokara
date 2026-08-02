//! kumokara-event — Event bus, persistence, and output buffering.
//!
//! Implements the two-layer model from DESIGN.md §3.5:
//! 1. Structured events → SQLite (long-term, queryable)
//! 2. Raw terminal output → ring buffer (ephemeral, per-session)

pub mod buffer;
pub mod bus;
pub mod log;

pub use buffer::OutputBuffer;
pub use bus::EventBus;
pub use log::EventLog;

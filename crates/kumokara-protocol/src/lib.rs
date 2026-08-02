//! Kumokara Protocol — shared types for WebSocket messages, workspace info, and events.
//!
//! This crate defines the wire format and shared data structures used between
//! the Kumokara server and all clients (web, CLI, future Tauri app).

pub mod event;
pub mod messages;
pub mod workspace;

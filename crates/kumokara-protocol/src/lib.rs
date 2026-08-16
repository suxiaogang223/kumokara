//! Wire types shared by the Kumokara server and its clients.
//!
//! The protocol exposes authentication, terminal sessions and I/O, plus a
//! small server-side directory browser used to choose session working
//! directories. Workspaces remain a client navigation concept rather than a
//! separate server lifecycle object.

pub mod messages;
pub mod session;

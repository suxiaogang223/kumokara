//! Wire types shared by the Kumokara server and its clients.
//!
//! The protocol intentionally exposes only authentication, terminal sessions,
//! and terminal I/O. Working directories and agents are properties discovered
//! from sessions rather than separate lifecycle objects.

pub mod messages;
pub mod session;

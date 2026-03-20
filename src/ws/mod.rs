//! NomadTerm WebSocket server — connects Flutter mobile clients to PTY sessions.
//!
//! Module layout:
//!   protocol  — JSON message types (ServerMessage / ClientMessage)
//!   session   — SessionPool (spawn, inject, broadcast PTY output)
//!   handler   — per-connection WebSocket handler
//!   server    — axum HTTP server, token auth, QR code

pub mod handler;
pub mod protocol;
pub mod server;
pub mod session;

pub use server::{WsConfig, run};

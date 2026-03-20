//! WebSocket message protocol (Phase 2).
//!
//! Two frame types:
//!   - Text frames: JSON-encoded control messages (spawn, approve, list, etc.)
//!   - Binary frames: raw PTY bytes forwarded directly to xterm on the client

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Messages sent from the Rust daemon → Flutter client (JSON text frames).
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// AI CLI tool-call approval request — mobile shows approve dialog.
    Approve {
        id: String,
        command: String,
        risk: String,
    },
    /// PTY output chunk (base64-encoded) — alternative to raw binary frames.
    /// Binary frames are preferred; this is used when the client requests text mode.
    Output {
        session_id: String,
        /// base64-encoded PTY bytes
        data: String,
    },
    /// Current session list (sent on connect and on change).
    /// `workspace` is the absolute path of the server's working directory.
    SessionList { sessions: Vec<SessionInfo>, workspace: String },
    /// Error response.
    Error { message: String },
    /// Session started successfully.
    SessionStarted { session_id: String, cli: String },
    /// Session ended.
    SessionEnded { session_id: String },
    /// Periodic usage snapshot — AI tokens/cost + hardware power.
    UsageUpdate {
        /// Unix timestamp in milliseconds.
        timestamp: u64,
        /// Per-CLI usage stats; key is CLI name ("claude", "codex", etc.).
        ai_usage: HashMap<String, AiUsage>,
        /// Hardware power readings (None if unavailable on this platform).
        hardware: Option<HardwarePower>,
    },
}

/// Per-AI-CLI token and cost statistics.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AiUsage {
    pub session_id: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: f64,
    /// Cumulative cost aggregated across all sessions today.
    pub cumulative_day_usd: f64,
}

/// System hardware power readings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HardwarePower {
    pub cpu_watts: f64,
    pub gpu_watts: Option<f64>,
    pub total_watts: f64,
    /// Rolling average since daemon startup.
    pub average_since_session: f64,
}

/// Messages sent from Flutter client → Rust daemon (JSON text frames).
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Spawn a new AI CLI session.
    Spawn {
        /// "claude" | "codex" | "copilot" | "gemini"
        cli: String,
    },
    /// Approve or deny a tool-call request.
    Approve { id: String, decision: bool },
    /// Send text input to a PTY session (JSON text mode).
    Input { session_id: String, data: String },
    /// Resize a PTY session terminal.
    Resize {
        session_id: String,
        cols: u16,
        rows: u16,
    },
    /// Kill a PTY session.
    Kill { session_id: String },
    /// Request current session list.
    ListSessions,
}

/// Metadata about a single PTY session.
#[derive(Serialize, Clone, Debug)]
pub struct SessionInfo {
    pub id: String,
    pub cli: String,
    pub status: String,
    pub inject_port: u16,
}

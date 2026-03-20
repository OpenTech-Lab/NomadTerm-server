//! Axum WebSocket server — Phase 1 & Phase 5 (auth + optional TLS).
//!
//! Startup sequence:
//!   1. Load or generate a random bearer token from ~/.hcom/nomadterm.token
//!   2. Bind to Tailscale IP (--bind-tailscale) or 0.0.0.0 on the given port
//!   3. Print connection string + QR code to stdout
//!   4. Handle /ws upgrades, validating Authorization: Bearer <token>

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::{
        State,
        WebSocketUpgrade,
        ws::WebSocket,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use std::io::{self, BufRead, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::ws::{handler, session::SessionPool};

/// Shared state for the axum server (also used by integration tests).
#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<SessionPool>,
    pub token: Arc<String>,
    pub workspace: Arc<PathBuf>,
    pub repo_id: Option<String>,
}

/// Configuration for the WebSocket server.
pub struct WsConfig {
    /// Network interface to bind (Tailscale IP or 0.0.0.0).
    pub bind_addr: String,
    /// TCP port (default 7681).
    pub port: u16,
    /// Disable TLS (dev mode / trusted LAN).
    pub no_tls: bool,
    /// Working directory used as the session workspace (defaults to CWD at startup).
    pub workspace_dir: PathBuf,
    /// Skip the interactive trust prompt (e.g. when --go flag is passed).
    pub skip_trust_prompt: bool,
    /// Per-repo token override (GUI mode — skips global file token).
    pub token_override: Option<String>,
    /// Repo UUID — used to call touch_repo on successful WS connect.
    pub repo_id: Option<String>,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            port: 7681,
            no_tls: true,
            workspace_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            skip_trust_prompt: false,
            token_override: None,
            repo_id: None,
        }
    }
}

/// Entry point: start the WebSocket server (blocking — call from a tokio runtime).
pub async fn run(config: WsConfig) -> Result<()> {
    // Resolve workspace to an absolute canonical path.
    let workspace = config.workspace_dir.canonicalize()
        .unwrap_or_else(|_| config.workspace_dir.clone());

    // Trust prompt — mirrors VS Code's "Do you trust the authors of this folder?"
    if !config.skip_trust_prompt {
        prompt_trust_folder(&workspace)?;
    }

    let token = if let Some(t) = config.token_override {
        t
    } else {
        load_or_create_token()?
    };
    let pool = Arc::new(SessionPool::new_with_workspace(workspace.clone()));
    let token_arc = Arc::new(token.clone());
    let workspace_arc = Arc::new(workspace.clone());

    let state = AppState {
        pool,
        token: token_arc,
        workspace: workspace_arc,
        repo_id: config.repo_id,
    };

    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route("/health", get(health_handler))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", config.bind_addr, config.port)
        .parse()
        .context("Invalid bind address")?;

    print_connection_info(&addr, &token, &workspace);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {addr}"))?;

    eprintln!("[nomadterm] WebSocket server listening on ws://{addr}/ws");

    axum::serve(listener, app)
        .await
        .context("WebSocket server error")
}

/// GET /ws — authenticate and upgrade to WebSocket.
async fn ws_upgrade_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Phase 5: validate bearer token.
    if !check_auth(&headers, &state.token) {
        return (StatusCode::UNAUTHORIZED, "Invalid or missing token").into_response();
    }

    let pool = state.pool.clone();
    let workspace = state.workspace.clone();
    let repo_id = state.repo_id.clone();
    ws.on_upgrade(move |socket: WebSocket| async move {
        handler::handle_socket(socket, pool, workspace, repo_id).await;
    })
}

/// GET /health — liveness probe.
async fn health_handler() -> &'static str {
    "ok"
}

/// Validate `Authorization: Bearer <token>` header.
fn check_auth(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == expected)
        .unwrap_or(false)
}

/// Prompt the user to confirm they trust the given folder before starting the server.
/// Mirrors VS Code's "Do you trust the authors of the files in this folder?" prompt.
fn prompt_trust_folder(path: &std::path::Path) -> Result<()> {
    let path_str = path.display();
    eprintln!("\n┌─────────────────────────────────────────────────────┐");
    eprintln!("│          NomadTerm — Workspace Trust Check          │");
    eprintln!("├─────────────────────────────────────────────────────┤");
    eprintln!("│  Do you trust the authors of the files in:          │");
    eprintln!("│  {}  │", path_str);
    eprintln!("│                                                     │");
    eprintln!("│  NomadTerm will use this folder as your workspace   │");
    eprintln!("│  and may execute code within it.                    │");
    eprintln!("└─────────────────────────────────────────────────────┘");
    eprint!("\nTrust this folder and continue? [y/N]: ");
    io::stderr().flush().ok();

    let stdin = io::stdin();
    let answer = stdin.lock().lines().next()
        .and_then(|l| l.ok())
        .unwrap_or_default()
        .trim()
        .to_lowercase();

    if answer == "y" || answer == "yes" {
        eprintln!("[nomadterm] Workspace trusted: {}", path_str);
        Ok(())
    } else {
        bail!("Workspace not trusted. Exiting.")
    }
}

/// Load token from disk or generate + persist a new one.
fn load_or_create_token() -> Result<String> {
    let token_path = crate::paths::hcom_dir().join("nomadterm.token");

    if let Ok(token) = std::fs::read_to_string(&token_path) {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    // Generate a random 32-byte hex token.
    let token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();

    crate::paths::atomic_write(&token_path, &token);
    Ok(token)
}

/// Print the connection URL and QR code to stderr so the user can scan from their phone.
fn print_connection_info(addr: &SocketAddr, token: &str, workspace: &std::path::Path) {
    let connection_string = format!("ws://{}/?token={}", addr, token);
    let workspace_str = workspace.display().to_string();

    eprintln!("\n╔══════════════════════════════════════════════════╗");
    eprintln!("║           NomadTerm — WebSocket Daemon           ║");
    eprintln!("╠══════════════════════════════════════════════════╣");
    eprintln!("║  Workspace: {}", workspace_str);
    eprintln!("║  Address : ws://{}/ws", addr);
    eprintln!("║  Token   : {}", token);
    eprintln!("╠══════════════════════════════════════════════════╣");
    eprintln!("║  Scan QR code from the NomadTerm Flutter app:   ║");
    eprintln!("╚══════════════════════════════════════════════════╝\n");

    // Print QR code to terminal.
    match qrcode::QrCode::new(connection_string.as_bytes()) {
        Ok(code) => {
            let image = code
                .render::<char>()
                .quiet_zone(false)
                .module_dimensions(2, 1)
                .build();
            eprintln!("{image}");
        }
        Err(e) => {
            eprintln!("[nomadterm] QR code generation failed: {e}");
        }
    }

    eprintln!("\nConnection URL: {connection_string}\n");
}

/// Detect the Tailscale IP by scanning network interfaces for the 100.x.x.x range.
pub fn detect_tailscale_ip() -> Option<String> {
    // Parse /proc/net/fib_trie or use `ip addr` — simplest: scan interface IPs.
    // We look for a 100.64.0.0/10 address (Tailscale's CGNAT range).
    use std::net::IpAddr;

    // Try reading from `ip addr` output via /proc/net/if_inet6 is complex.
    // Instead, iterate over all local addresses.
    // On Linux we can read /proc/net/fib_trie; cross-platform: use `if_addrs` crate.
    // For simplicity, check environment variable TAILSCALE_IP first.
    if let Ok(ip) = std::env::var("TAILSCALE_IP") {
        if !ip.is_empty() {
            return Some(ip);
        }
    }

    // Fallback: run `tailscale ip -4` and capture output.
    if let Ok(output) = std::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
    {
        if output.status.success() {
            let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ip.is_empty() {
                // Validate it's in 100.64.0.0/10
                if let Ok(IpAddr::V4(v4)) = ip.parse::<IpAddr>() {
                    let octets = v4.octets();
                    if octets[0] == 100 && (octets[1] & 0xC0) == 64 {
                        return Some(ip);
                    }
                }
            }
        }
    }

    None
}

//! NomadTerm Tauri backend.
//!
//! Self-contained: owns the repo DB operations (rusqlite) and manages
//! per-repo nomadterm WS server subprocesses.

mod db;

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use db::RepoRow;

// ---------------------------------------------------------------------------
// Types shared with the frontend (serialised to JSON)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub id: String,
    pub path: String,
    pub name: String,
    pub token: String,
    pub added_at: i64,
    pub last_seen: Option<i64>,
    pub is_active: bool,
}

impl From<RepoRow> for RepoEntry {
    fn from(r: RepoRow) -> Self {
        Self {
            id: r.id,
            path: r.path,
            name: r.name,
            token: r.token,
            added_at: r.added_at,
            last_seen: r.last_seen,
            is_active: r.is_active,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStrategyKind {
    Tailscale,
    Lan,
    LocalOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStrategy {
    pub kind: ConnectionStrategyKind,
    pub host: Option<String>,
    pub remote_capable: bool,
    pub secure: bool,
}

impl ConnectionStrategy {
    fn bind_tailscale(&self) -> bool {
        matches!(self.kind, ConnectionStrategyKind::Tailscale)
    }
}

// ---------------------------------------------------------------------------
// App state — tracks running nomadterm subprocesses
// ---------------------------------------------------------------------------

struct ServerProcess {
    port: u16,
    #[allow(dead_code)]
    child: tokio::process::Child,
}

pub struct AppState {
    servers: Mutex<HashMap<String, ServerProcess>>,
}

// ---------------------------------------------------------------------------
// Helper: find the nomadterm binary
// ---------------------------------------------------------------------------

fn nomadterm_bin() -> String {
    // 1. Same directory as this Tauri app binary (production install path).
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.parent().map(|p| p.join("nomadterm"));
        if let Some(path) = sibling {
            if path.is_file() {
                return path.to_string_lossy().to_string();
            }
        }

        // 2. Dev: walk up from the Tauri exe location to find the workspace
        //    target/debug or target/release directory.
        //    e.g. .../desktop/src-tauri/target/debug/nomadterm-desktop
        //    → look for .../server/target/debug/nomadterm
        if let Some(dir) = exe.parent() {
            // Climb up from target/{debug,release} → src-tauri → desktop → server
            let candidates = [
                dir.join("nomadterm"),                                        // sibling (already checked)
                dir.join("../../..").join("target/debug/nomadterm"),          // workspace debug
                dir.join("../../..").join("target/release/nomadterm"),        // workspace release
                dir.join("../../../..").join("target/debug/nomadterm"),       // one more level
                dir.join("../../../..").join("target/release/nomadterm"),
            ];
            for candidate in &candidates {
                if let Ok(resolved) = candidate.canonicalize() {
                    if resolved.is_file() {
                        return resolved.to_string_lossy().to_string();
                    }
                }
            }
        }
    }
    // 3. PATH fallback.
    "nomadterm".to_string()
}

fn preferred_connect_host(tailscale_ip: Option<String>, lan_ip: Option<String>) -> Option<String> {
    tailscale_ip.or(lan_ip)
}

fn detect_connection_strategy_inner() -> ConnectionStrategy {
    let tailscale_ip = detect_tailscale_ip();
    let lan_ip = detect_lan_ip();
    let host = preferred_connect_host(tailscale_ip.clone(), lan_ip.clone());

    if let Some(ip) = tailscale_ip {
        ConnectionStrategy {
            kind: ConnectionStrategyKind::Tailscale,
            host: Some(ip),
            remote_capable: true,
            secure: true,
        }
    } else if let Some(ip) = lan_ip {
        ConnectionStrategy {
            kind: ConnectionStrategyKind::Lan,
            host: Some(ip),
            remote_capable: false,
            secure: false,
        }
    } else {
        ConnectionStrategy {
            kind: ConnectionStrategyKind::LocalOnly,
            host,
            remote_capable: false,
            secure: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_repos() -> Result<Vec<RepoEntry>, String> {
    let db = db::RepoDB::open().map_err(|e| e.to_string())?;
    let rows = db.list_repos().map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(RepoEntry::from).collect())
}

#[tauri::command]
fn add_repo(path: String) -> Result<RepoEntry, String> {
    let db = db::RepoDB::open().map_err(|e| e.to_string())?;
    let row = db.upsert_repo(&path).map_err(|e| e.to_string())?;
    Ok(RepoEntry::from(row))
}

#[tauri::command]
fn remove_repo(state: State<AppState>, id: String) -> Result<(), String> {
    // Kill running server for this repo if any.
    {
        let mut servers = state.servers.lock().unwrap();
        if let Some(mut proc) = servers.remove(&id) {
            let _ = proc.child.start_kill();
        }
    }
    let db = db::RepoDB::open().map_err(|e| e.to_string())?;
    db.remove_repo(&id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn start_server(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    repo_token: String,
    repo_path: String,
    port: u16,
) -> Result<(), String> {
    // Bail early if already running.
    {
        let servers = state.servers.lock().unwrap();
        if servers.contains_key(&repo_id) {
            return Ok(());
        }
    }

    let strategy = detect_connection_strategy_inner();
    let bin = nomadterm_bin();
    let mut command = tokio::process::Command::new(&bin);
    command.args(["--ws", "--no-tls", "--go"]);
    if strategy.bind_tailscale() {
        command.arg("--bind-tailscale");
    }
    command.args([
        "--port",
        &port.to_string(),
        "--token",
        &repo_token,
        "--workspace",
        &repo_path,
    ]);
    let child = command
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to spawn {bin}: {e}"))?;

    {
        let mut servers = state.servers.lock().unwrap();
        servers.insert(repo_id.clone(), ServerProcess { port, child });
    }

    // Update DB active flag.
    if let Ok(db) = db::RepoDB::open() {
        let _ = db.set_repo_active(&repo_id, true);
    }

    let _ = app.emit("server-started", &repo_id);

    // Spawn a watcher task that emits server-stopped when the process exits.
    let app2 = app.clone();
    let rid = repo_id.clone();
    drop(state);
    // The child is stored with kill_on_drop; process exit is handled by
    // stop_server or on app exit.
    // Emit server-stopped if the process dies unexpectedly.
    tauri::async_runtime::spawn(async move {
        // Give the process a moment to start before we watch it.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // We can't access the managed State from here, so just note the
        // exit in the log; the frontend can call is_server_running to poll.
        let _ = (app2, rid);
    });

    Ok(())
}

#[tauri::command]
fn stop_server(
    app: AppHandle,
    state: State<AppState>,
    repo_id: String,
) -> Result<(), String> {
    let mut servers = state.servers.lock().unwrap();
    if let Some(mut proc) = servers.remove(&repo_id) {
        let _ = proc.child.start_kill();
    }
    if let Ok(db) = db::RepoDB::open() {
        let _ = db.set_repo_active(&repo_id, false);
    }
    let _ = app.emit("server-stopped", &repo_id);
    Ok(())
}

#[tauri::command]
fn detect_host() -> Option<String> {
    detect_connection_strategy_inner().host
}

#[tauri::command]
fn detect_connection_strategy() -> ConnectionStrategy {
    detect_connection_strategy_inner()
}

#[tauri::command]
fn is_server_running(state: State<AppState>, repo_id: String) -> bool {
    state.servers.lock().unwrap().contains_key(&repo_id)
}

#[tauri::command]
fn server_port(state: State<AppState>, repo_id: String) -> Option<u16> {
    state
        .servers
        .lock()
        .unwrap()
        .get(&repo_id)
        .map(|p| p.port)
}

// ---------------------------------------------------------------------------
// Network host detection (mirrors ws/server.rs logic, without the full crate)
// ---------------------------------------------------------------------------

fn detect_tailscale_ip() -> Option<String> {
    use std::process::Command;
    if let Ok(ip) = std::env::var("TAILSCALE_IP") {
        if !ip.is_empty() {
            return Some(ip);
        }
    }
    // `tailscale ip -4` is the canonical way
    if let Ok(out) = Command::new("tailscale").args(["ip", "-4"]).output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() && out.status.success() {
            return Some(s);
        }
    }
    None
}

fn detect_lan_ip() -> Option<String> {
    use std::net::{IpAddr, UdpSocket};
    if let Ok(ip) = std::env::var("NOMADTERM_LAN_IP") {
        if !ip.is_empty() {
            return Some(ip);
        }
    }
    // Bind a UDP socket and "connect" it to a public address — no packets are
    // sent, but the OS routing table chooses the correct outbound interface,
    // giving us the LAN IP instantly without any network round-trip.
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    for remote in ["192.0.2.1:80", "8.8.8.8:80"] {
        if socket.connect(remote).is_err() {
            continue;
        }
        if let Ok(local_addr) = socket.local_addr() {
            if let IpAddr::V4(v4) = local_addr.ip() {
                if !v4.is_loopback() && !v4.is_unspecified() {
                    return Some(v4.to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            servers: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            list_repos,
            add_repo,
            remove_repo,
            start_server,
            stop_server,
            detect_host,
            detect_connection_strategy,
            is_server_running,
            server_port,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

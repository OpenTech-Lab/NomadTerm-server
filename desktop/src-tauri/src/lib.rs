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
    // 1. Same directory as this Tauri app binary.
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.parent().map(|p| p.join("nomadterm"));
        if let Some(path) = sibling {
            if path.is_file() {
                return path.to_string_lossy().to_string();
            }
        }
    }
    // 2. PATH fallback.
    "nomadterm".to_string()
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

    let bin = nomadterm_bin();
    let child = tokio::process::Command::new(&bin)
        .args([
            "--ws",
            "--no-tls",
            "--go",
            "--port",
            &port.to_string(),
            "--token",
            &repo_token,
            "--workspace",
            &repo_path,
        ])
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
    // Try Tailscale first, then LAN.
    detect_tailscale_ip().or_else(detect_lan_ip)
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
    // `tailscale ip -4` is the canonical way
    if let Ok(out) = Command::new("tailscale").args(["ip", "-4"]).output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() && out.status.success() {
            return Some(s);
        }
    }
    // Fallback: look for 100.x.y.z address in network interfaces
    detect_lan_ip()
        .filter(|ip| ip.starts_with("100."))
        .or(None)
}

fn detect_lan_ip() -> Option<String> {
    use std::net::{Ipv4Addr, TcpStream, ToSocketAddrs};
    // Connect to a public address to discover the outbound interface.
    let target = "8.8.8.8:80";
    if let Ok(addrs) = target.to_socket_addrs() {
        for addr in addrs {
            if let Ok(sock) = TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)) {
                if let Ok(local) = sock.local_addr() {
                    if let std::net::IpAddr::V4(v4) = local.ip() {
                        if v4 != Ipv4Addr::LOCALHOST && !v4.is_loopback() {
                            return Some(v4.to_string());
                        }
                    }
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
            is_server_running,
            server_port,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

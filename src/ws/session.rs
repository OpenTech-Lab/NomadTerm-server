//! PTY session pool — manages active AI CLI sessions for the WebSocket daemon.
//!
//! Each session wraps a running `nomadterm pty <cli>` subprocess whose stdout
//! is broadcast to subscribed WebSocket clients via a tokio broadcast channel.
//! Input is injected via the PTY's existing TCP inject port (registered in SQLite).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ws::protocol::SessionInfo;

/// Capacity of the PTY output broadcast channel per session (bytes buffered).
const BROADCAST_CAPACITY: usize = 256;
/// Timeout for PTY inject port to appear in DB.
const INJECT_PORT_TIMEOUT_MS: u64 = 5000;
/// Poll interval while waiting for inject port.
const INJECT_PORT_POLL_MS: u64 = 50;

/// Status of a PTY session.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Starting,
    Running,
    Exited,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Exited => "exited",
        }
    }
}

/// A single active PTY session.
pub struct Session {
    pub id: String,
    pub cli: String,
    pub status: SessionStatus,
    /// TCP port of the PTY inject server (discovered from SQLite after startup).
    pub inject_port: u16,
    /// Broadcast sender — PTY stdout is sent here; WS handlers subscribe.
    pub tx: broadcast::Sender<Vec<u8>>,
    /// Child process handle (kept alive to prevent zombie).
    _child: std::process::Child,
}

impl Session {
    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            cli: self.cli.clone(),
            status: self.status.as_str().to_string(),
            inject_port: self.inject_port,
        }
    }

    /// Inject text into the PTY via its TCP inject port.
    pub fn inject(&self, text: &str) -> Result<()> {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", self.inject_port))
            .context("Failed to connect to inject port")?;
        stream
            .write_all(text.as_bytes())
            .context("Failed to write to inject port")?;
        Ok(())
    }

    /// Subscribe to PTY output.
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.tx.subscribe()
    }
}

/// Thread-safe pool of active PTY sessions.
#[derive(Clone)]
pub struct SessionPool {
    inner: Arc<Mutex<HashMap<String, Session>>>,
    workspace_dir: Arc<PathBuf>,
}

impl SessionPool {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            workspace_dir: Arc::new(
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            ),
        }
    }

    pub fn new_with_workspace(workspace_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            workspace_dir: Arc::new(workspace_dir),
        }
    }

    /// Spawn a new PTY session for the given AI CLI tool.
    ///
    /// Launches `nomadterm pty <cli>` with `HCOM_INSTANCE_NAME=<uuid>`, then polls
    /// the SQLite DB until the PTY registers its inject port.
    pub fn spawn(&self, cli: &str) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let cli_name = cli.to_string();

        let binary = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("nomadterm"));

        let (tx, _) = broadcast::channel::<Vec<u8>>(BROADCAST_CAPACITY);
        let tx_clone = tx.clone();

        let mut child = std::process::Command::new(&binary)
            .args(["pty", cli])
            .env("HCOM_INSTANCE_NAME", &session_id)
            .current_dir(self.workspace_dir.as_ref())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn PTY for cli={cli}"))?;

        // Wait for the PTY to register its inject port in SQLite.
        let inject_port = Self::wait_for_inject_port(&session_id, INJECT_PORT_TIMEOUT_MS)?;

        // Pipe PTY stdout → broadcast channel in a background thread.
        let stdout = child.stdout.take().context("No stdout from PTY child")?;
        std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = std::io::BufReader::new(stdout);
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let _ = tx_clone.send(buf[..n].to_vec());
                    }
                }
            }
        });

        let session = Session {
            id: session_id.clone(),
            cli: cli_name,
            status: SessionStatus::Running,
            inject_port,
            tx,
            _child: child,
        };

        self.inner.lock().unwrap().insert(session_id.clone(), session);

        Ok(session_id)
    }

    /// Poll SQLite until the PTY registers its inject port or timeout expires.
    fn wait_for_inject_port(instance_name: &str, timeout_ms: u64) -> Result<u16> {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(timeout_ms);

        let db = crate::db::HcomDb::open().context("Failed to open DB for inject port discovery")?;

        while std::time::Instant::now() < deadline {
            if let Ok(Some(port)) = db.get_inject_port(instance_name) {
                return Ok(port);
            }
            std::thread::sleep(std::time::Duration::from_millis(INJECT_PORT_POLL_MS));
        }

        anyhow::bail!(
            "Timed out waiting for inject port for instance={instance_name} ({timeout_ms}ms)"
        )
    }

    /// List metadata for all non-exited sessions.
    pub fn list(&self) -> Vec<SessionInfo> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.status != SessionStatus::Exited)
            .map(|s| s.info())
            .collect()
    }

    /// Subscribe to PTY broadcast output for a session.
    pub fn subscribe(&self, session_id: &str) -> Option<broadcast::Receiver<Vec<u8>>> {
        self.inner
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.subscribe())
    }

    /// Inject raw text into a PTY session.
    pub fn inject(&self, session_id: &str, text: &str) -> Result<()> {
        let pool = self.inner.lock().unwrap();
        let session = pool
            .get(session_id)
            .with_context(|| format!("Session not found: {session_id}"))?;
        session.inject(text)
    }

    /// Inject approval response ("y\n" or "n\n") for a tool-call.
    pub fn inject_approve(&self, session_id: &str, approved: bool) -> Result<()> {
        let response = if approved { "y\n" } else { "n\n" };
        self.inject(session_id, response)
    }

    /// Kill and remove a session.
    pub fn kill(&self, session_id: &str) {
        self.inner.lock().unwrap().remove(session_id);
    }
}

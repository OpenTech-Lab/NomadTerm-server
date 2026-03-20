//! Per-repo WS server tasks.

use std::path::PathBuf;
use tokio::sync::oneshot;

use crate::ws::WsConfig;

/// Base port for GUI-managed servers.
pub const BASE_PORT: u16 = 7682;

/// A handle to a running per-repo WS server task.
pub struct ServerHandle {
    pub repo_id: String,
    pub port: u16,
    shutdown_tx: oneshot::Sender<()>,
}

impl ServerHandle {
    /// Spawn a WS server for the given repo and return a handle.
    pub fn spawn(
        runtime: &tokio::runtime::Handle,
        repo_id: String,
        repo_token: String,
        repo_path: PathBuf,
        port: u16,
        event_tx: tokio::sync::mpsc::UnboundedSender<super::state::ServerEvent>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let rid = repo_id.clone();
        let rid2 = repo_id.clone();

        runtime.spawn(async move {
            let config = WsConfig {
                bind_addr: "0.0.0.0".to_string(),
                port,
                no_tls: true,
                workspace_dir: repo_path,
                skip_trust_prompt: true,
                token_override: Some(repo_token),
                repo_id: Some(rid.clone()),
            };

            let _ = event_tx.send(super::state::ServerEvent::Started { repo_id: rid });

            tokio::select! {
                result = crate::ws::run(config) => {
                    if let Err(e) = result {
                        eprintln!("[gui] WS server error for repo {}: {e}", rid2);
                    }
                }
                _ = shutdown_rx => {
                    eprintln!("[gui] WS server for repo {} shut down", rid2);
                }
            }
        });

        ServerHandle { repo_id, port, shutdown_tx }
    }

    /// Send the shutdown signal to the running task.
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
    }
}

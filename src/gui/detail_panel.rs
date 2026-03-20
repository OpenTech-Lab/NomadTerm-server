//! Right panel: QR image, Start/Stop, status, copy URL.

use egui::{TextureHandle, Ui};

use super::server_task::ServerHandle;
use super::state::GuiState;

pub fn show(
    ui: &mut Ui,
    state: &mut GuiState,
    handles: &mut Vec<Option<ServerHandle>>,
    rt: &tokio::runtime::Handle,
    qr_cache: &mut Option<TextureHandle>,
    session_counts: &std::collections::HashMap<String, usize>,
) {
    let idx = match state.selected_idx {
        Some(i) if i < state.repos.len() => i,
        _ => {
            ui.label("Select a repo from the left panel.");
            return;
        }
    };

    // Clone repo fields needed across mutable borrow boundaries.
    let repo_id = state.repos[idx].id.clone();
    let repo_name = state.repos[idx].name.clone();
    let repo_path = state.repos[idx].path.clone();
    let repo_token = state.repos[idx].token.clone();
    let port = super::server_task::BASE_PORT + idx as u16;

    ui.vertical(|ui| {
        ui.heading(&repo_name);
        ui.label(&repo_path);
        ui.add_space(8.0);

        let is_running = handles.get(idx).and_then(|h| h.as_ref()).is_some();
        let btn_label = if is_running { "■ Stop" } else { "▶ Start" };
        let count = session_counts.get(&repo_id).copied().unwrap_or(0);

        let status_text = if is_running {
            format!("● Running   Sessions: {}", count)
        } else {
            "○ Stopped".to_string()
        };
        ui.label(&status_text);
        ui.label(format!("Port: {}", port));

        if ui.button(btn_label).clicked() {
            if is_running {
                // Stop
                if let Some(entry) = handles.get_mut(idx) {
                    if let Some(handle) = entry.take() {
                        handle.stop();
                    }
                }
                state.repos[idx].is_active = false;
                if let Ok(db) = crate::db::HcomDb::open() {
                    let _ = db.set_repo_active(&repo_id, false);
                }
                *qr_cache = None;
            } else {
                // Start — ensure handles vec is large enough
                while handles.len() <= idx {
                    handles.push(None);
                }
                let h = ServerHandle::spawn(
                    rt,
                    repo_id.clone(),
                    repo_token.clone(),
                    std::path::PathBuf::from(&repo_path),
                    port,
                    state.event_tx.clone(),
                );
                handles[idx] = Some(h);
                state.repos[idx].is_active = true;
                if let Ok(db) = crate::db::HcomDb::open() {
                    let _ = db.set_repo_active(&repo_id, true);
                }
                *qr_cache = None;
            }
        }

        ui.add_space(12.0);

        if is_running {
            let detected_host = crate::ws::server::detect_connect_host();
            let host = detected_host
                .clone()
                .unwrap_or_else(|| "127.0.0.1".to_string());
            let repo_name_enc = urlencoding_simple(&repo_name);
            let uri = format!(
                "nomadterm://{}:{}?token={}&repo_id={}&repo_name={}&tls=0",
                host, port, repo_token, repo_id, repo_name_enc
            );

            // QR code
            if qr_cache.is_none() {
                *qr_cache = super::qr_texture::generate_qr_texture(ui.ctx(), &uri);
            }

            if let Some(tex) = qr_cache {
                let size = egui::vec2(260.0, 260.0);
                ui.image((tex.id(), size));
            }

            ui.add_space(8.0);
            ui.label(&uri);
            if detected_host.is_none() {
                ui.label("No reachable LAN or Tailscale IP detected. The phone must be on the same reachable network as this machine.");
            }

            if ui.button("Copy URL").clicked() {
                ui.ctx().copy_text(uri.clone());
            }
        }
    });
}

/// Minimal percent-encoding for path components (spaces → %20, etc.).
fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/' {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect::<Vec<_>>()
            }
        })
        .collect()
}

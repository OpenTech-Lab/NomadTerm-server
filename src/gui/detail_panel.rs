//! Right panel: QR image, Start/Stop, status, copy URL.

use egui::{TextureHandle, Ui};

use super::server_task::ServerHandle;
use super::state::{GuiState, ServerEvent};

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

    let repo = &state.repos[idx];
    let port = super::server_task::BASE_PORT + idx as u16;

    ui.vertical(|ui| {
        ui.heading(&repo.name);
        ui.label(&repo.path);
        ui.add_space(8.0);

        let is_running = handles.get(idx).and_then(|h| h.as_ref()).is_some();
        let btn_label = if is_running { "■ Stop" } else { "▶ Start" };
        let count = session_counts.get(&repo.id).copied().unwrap_or(0);

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
                    let _ = db.set_repo_active(&repo.id, false);
                }
                *qr_cache = None;
            } else {
                // Start — ensure handles vec is large enough
                while handles.len() <= idx {
                    handles.push(None);
                }
                let h = ServerHandle::spawn(
                    rt,
                    repo.id.clone(),
                    repo.token.clone(),
                    std::path::PathBuf::from(&repo.path),
                    port,
                    state.event_tx.clone(),
                );
                handles[idx] = Some(h);
                state.repos[idx].is_active = true;
                if let Ok(db) = crate::db::HcomDb::open() {
                    let _ = db.set_repo_active(&repo.id, true);
                }
                *qr_cache = None;
            }
        }

        ui.add_space(12.0);

        if is_running {
            let host = crate::ws::server::detect_tailscale_ip()
                .unwrap_or_else(|| "0.0.0.0".to_string());
            let expires_at = chrono::Utc::now()
                + chrono::Duration::days(30);
            let expires_str = expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let repo_path_enc = urlencoding_simple(&repo.path);
            let repo_name_enc = urlencoding_simple(&repo.name);
            let uri = format!(
                "nomadterm://{}:{}?token={}&repo_id={}&repo_path={}&repo_name={}&expires_at={}&tls=0",
                host, port, repo.token, repo.id, repo_path_enc, repo_name_enc, expires_str
            );

            // QR code
            if qr_cache.is_none() {
                *qr_cache = super::qr_texture::generate_qr_texture(ui.ctx(), &uri);
            }

            if let Some(tex) = qr_cache {
                let size = egui::vec2(200.0, 200.0);
                ui.image((tex.id(), size));
            }

            ui.add_space(8.0);
            ui.label(&uri);

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

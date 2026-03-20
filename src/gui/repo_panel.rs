//! Left sidebar: repo list + Add/Remove controls.

use egui::Ui;

use super::state::{GuiState, RepoEntry};

pub fn show(ui: &mut Ui, state: &mut GuiState) {
    ui.vertical(|ui| {
        ui.heading("REPOS");
        ui.separator();

        if ui.button("+ Add Repo").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                let path_str = path.to_string_lossy().to_string();
                // Upsert in DB
                if let Ok(mut db) = crate::db::HcomDb::open() {
                    if db.ensure_schema().is_ok() {
                        if let Ok(row) = db.upsert_repo(&path_str) {
                            let entry = RepoEntry {
                                id: row.id,
                                path: row.path,
                                name: row.name,
                                token: row.token,
                                added_at: row.added_at,
                                last_seen: row.last_seen,
                                is_active: row.is_active,
                            };
                            state.repos.push(entry);
                            state.selected_idx = Some(state.repos.len() - 1);
                        }
                    }
                }
            }
        }

        ui.add_space(8.0);

        let mut new_selection = state.selected_idx;
        for (i, repo) in state.repos.iter().enumerate() {
            let label = if repo.is_active {
                format!("● {}", repo.name)
            } else {
                format!("  {}", repo.name)
            };
            let selected = state.selected_idx == Some(i);
            if ui.selectable_label(selected, &label).clicked() {
                new_selection = Some(i);
            }
        }
        state.selected_idx = new_selection;

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            if ui.button("Remove").clicked() {
                if let Some(idx) = state.selected_idx {
                    if idx < state.repos.len() {
                        let repo_id = state.repos[idx].id.clone();
                        if let Ok(db) = crate::db::HcomDb::open() {
                            let _ = db.remove_repo(&repo_id);
                        }
                        state.repos.remove(idx);
                        state.selected_idx = if state.repos.is_empty() {
                            None
                        } else {
                            Some(idx.saturating_sub(1))
                        };
                    }
                }
            }
        });
    });
}

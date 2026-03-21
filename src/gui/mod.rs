//! Desktop GUI entry point.

mod app;
mod detail_panel;
mod qr_texture;
mod repo_panel;
mod server_task;
pub mod state;

use anyhow::Result;

/// Decode the embedded `assets/logo.png` into an [`egui::IconData`] at startup.
fn app_icon() -> egui::IconData {
    // Path is relative to this source file: server/src/gui/ → up 3 → repo root.
    let bytes = include_bytes!("../../contrib/logo.png");
    let img = image::load_from_memory(bytes)
        .expect("embedded logo.png must be a valid PNG")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}

/// Launch the eframe desktop window (blocks until window is closed).
pub fn run() -> Result<()> {
    // Create tokio runtime for WS server tasks.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build tokio runtime: {e}"))?;

    let rt_handle = rt.handle().clone();

    // Load existing repos from DB.
    let mut gui_state = state::GuiState::new();
    if let Ok(mut db) = crate::db::NomadtermDb::open() {
        if db.ensure_schema().is_ok() {
            if let Ok(rows) = db.list_repos() {
                gui_state.repos = rows
                    .into_iter()
                    .map(|r| state::RepoEntry {
                        id: r.id,
                        path: r.path,
                        name: r.name,
                        token: r.token,
                        added_at: r.added_at,
                        last_seen: r.last_seen,
                        is_active: false, // servers don't survive restart
                    })
                    .collect();
            }
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NomadTerm")
            .with_inner_size([800.0, 560.0])
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "NomadTerm",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::GuiApp::new(cc, gui_state, rt_handle)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))
}

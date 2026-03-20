//! GuiApp — the eframe application.

use std::collections::HashMap;

use egui::{CentralPanel, SidePanel, TextureHandle};

use super::detail_panel;
use super::repo_panel;
use super::server_task::ServerHandle;
use super::state::{GuiState, ServerEvent};

pub struct GuiApp {
    state: GuiState,
    rt: tokio::runtime::Handle,
    handles: Vec<Option<ServerHandle>>,
    qr_cache: Option<TextureHandle>,
    session_counts: HashMap<String, usize>,
}

impl GuiApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        state: GuiState,
        rt: tokio::runtime::Handle,
    ) -> Self {
        Self {
            state,
            rt,
            handles: Vec::new(),
            qr_cache: None,
            session_counts: HashMap::new(),
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain server events from background tasks.
        while let Ok(event) = self.state.event_rx.try_recv() {
            match event {
                ServerEvent::SessionCount { repo_id, count } => {
                    self.session_counts.insert(repo_id, count);
                }
                ServerEvent::Started { .. } | ServerEvent::Stopped { .. } => {}
            }
        }

        SidePanel::left("repo_panel")
            .min_width(180.0)
            .show(ctx, |ui| {
                repo_panel::show(ui, &mut self.state);
            });

        CentralPanel::default().show(ctx, |ui| {
            detail_panel::show(
                ui,
                &mut self.state,
                &mut self.handles,
                &self.rt,
                &mut self.qr_cache,
                &self.session_counts,
            );
        });
    }
}

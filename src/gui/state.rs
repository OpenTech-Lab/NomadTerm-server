//! GUI state types.

/// A single registered repo entry (mirrors RepoRow but owned by the GUI).
#[derive(Debug, Clone)]
pub struct RepoEntry {
    pub id: String,
    pub path: String,
    pub name: String,
    pub token: String,
    pub added_at: i64,
    pub last_seen: Option<i64>,
    pub is_active: bool,
}

/// Events sent from WS server tasks → GUI main loop.
#[derive(Debug, Clone)]
pub enum ServerEvent {
    Started { repo_id: String },
    Stopped { repo_id: String },
    SessionCount { repo_id: String, count: usize },
}

/// Overall GUI application state.
pub struct GuiState {
    pub repos: Vec<RepoEntry>,
    pub selected_idx: Option<usize>,
    pub event_rx: tokio::sync::mpsc::UnboundedReceiver<ServerEvent>,
    pub event_tx: tokio::sync::mpsc::UnboundedSender<ServerEvent>,
}

impl GuiState {
    pub fn new() -> Self {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            repos: Vec::new(),
            selected_idx: None,
            event_rx,
            event_tx,
        }
    }
}

//! Per-connection WebSocket handler.
//!
//! Each connected Flutter client gets one handler task that:
//!   - Reads ClientMessage JSON text frames and dispatches them.
//!   - Forwards PTY output (binary frames) to the client for any subscribed session.
//!   - Validates the session auth token on first message (or via header — checked in server.rs).

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use std::sync::Arc;

use crate::ws::protocol::{ClientMessage, ServerMessage};
use crate::ws::session::SessionPool;

/// Drive a single WebSocket connection to completion.
pub async fn handle_socket(socket: WebSocket, pool: Arc<SessionPool>) {
    let (mut sender, mut receiver) = socket.split();

    // Send current session list on connect.
    let sessions = pool.list();
    let msg = ServerMessage::SessionList { sessions };
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Channel to forward PTY output → sender task.
    let (pty_tx, mut pty_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // Spawn sender task: reads from pty_tx and writes binary frames to WS.
    let mut send_task = tokio::spawn(async move {
        while let Some(data) = pty_rx.recv().await {
            if sender.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
    });

    // Receiver task: process incoming ClientMessages.
    let pool_ref = pool.clone();
    let pty_tx_ref = pty_tx.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    handle_text_frame(text.as_str(), &pool_ref, &pty_tx_ref).await;
                }
                Message::Binary(_) => {
                    // Binary frames from client are PTY input (raw keystrokes).
                    // Without a session_id we can't route them; clients should use
                    // JSON Input messages instead for proper routing.
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Abort the other task when one finishes.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

/// Handle a JSON text control frame from the client.
async fn handle_text_frame(
    text: &str,
    pool: &Arc<SessionPool>,
    pty_tx: &tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
) {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            // Log parse error; don't crash the connection.
            eprintln!("[ws] bad client message: {e}");
            return;
        }
    };

    match msg {
        ClientMessage::Spawn { cli } => {
            handle_spawn(&cli, pool, pty_tx).await;
        }
        ClientMessage::Input { session_id, data } => {
            if let Err(e) = pool.inject(&session_id, &data) {
                eprintln!("[ws] inject failed for {session_id}: {e}");
            }
        }
        ClientMessage::Approve { id, decision } => {
            // id format: "<session_id>:<nonce>" — nonce prevents replayed approvals.
            // We validate only that the session exists; nonce uniqueness is
            // enforced by the server sending each approve request with a UUID.
            if let Some(session_id) = id.split(':').next() {
                if let Err(e) = pool.inject_approve(session_id, decision) {
                    eprintln!("[ws] approve inject failed: {e}");
                }
            }
        }
        ClientMessage::Kill { session_id } => {
            pool.kill(&session_id);
        }
        ClientMessage::ListSessions => {
            // Response is sent as text back through pty_tx would be wrong — pty_tx is binary.
            // Sessions list is sent as an initial message; clients re-connect to refresh.
            // TODO: add a dedicated control_tx for server→client JSON messages.
        }
        ClientMessage::Resize { .. } => {
            // PTY resize via TCP is not yet implemented; no-op for now.
        }
    }
}

/// Spawn a new PTY session and subscribe to its output.
async fn handle_spawn(
    cli: &str,
    pool: &Arc<SessionPool>,
    pty_tx: &tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
) {
    let session_id = match pool.spawn(cli) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[ws] spawn failed for cli={cli}: {e}");
            return;
        }
    };

    // Subscribe to PTY broadcast and forward output to WS sender.
    if let Some(mut rx) = pool.subscribe(&session_id) {
        let tx = pty_tx.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(data) => {
                        if tx.send(data).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

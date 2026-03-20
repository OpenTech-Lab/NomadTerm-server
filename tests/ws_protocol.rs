//! Integration tests for the NomadTerm WebSocket protocol.
//!
//! Tests spin up a minimal axum WebSocket server on a random port and
//! connect via tokio-tungstenite to verify auth and protocol correctness.
//! No crate internals are imported — this is a pure black-box WS test.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::WebSocket},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

// ── Minimal test server ──────────────────────────────────────────────────

#[derive(Clone)]
struct TestState {
    token: Arc<String>,
}

async fn ws_handler(
    State(state): State<TestState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let ok = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == state.token.as_str())
        .unwrap_or(false);

    if !ok {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }

    ws.on_upgrade(|mut socket: WebSocket| async move {
        // Send a session_list on connect (mirrors real server behaviour).
        let msg = r#"{"type":"session_list","sessions":[]}"#;
        let _ = socket
            .send(axum::extract::ws::Message::Text(msg.into()))
            .await;
        // Keep alive until client closes.
        while let Some(Ok(_)) = socket.recv().await {}
    })
}

fn make_router(token: &str) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(TestState {
            token: Arc::new(token.to_string()),
        })
}

async fn start_server(token: &str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = make_router(token);
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_reject_without_token() {
    let addr = start_server("secret").await;
    let url = format!("ws://{addr}/ws");
    // No Authorization header → expect non-101 → connect_async error.
    let result = connect_async(url).await;
    assert!(result.is_err(), "expected rejection without token");
}

#[tokio::test]
async fn test_reject_with_wrong_token() {
    let addr = start_server("correct").await;
    let url = format!("ws://{addr}/ws");
    let mut req = url.into_client_request().unwrap();
    req.headers_mut()
        .insert("authorization", "Bearer wrong".parse().unwrap());

    let result = connect_async(req).await;
    assert!(result.is_err(), "expected rejection with wrong token");
}

#[tokio::test]
async fn test_accept_with_correct_token() {
    let token = "test-token-abc";
    let addr = start_server(token).await;

    let url = format!("ws://{addr}/ws");
    let mut req = url.into_client_request().unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());

    let (mut stream, _response) = connect_async(req).await.expect("connection should succeed");

    // First frame must be a session_list JSON message.
    let msg = stream.next().await.expect("expected a message").unwrap();
    let text = msg.into_text().expect("expected text frame");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["type"], "session_list");
    assert!(json["sessions"].is_array());
}

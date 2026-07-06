//! WebSocket endpoint for pushing `ServerEvent`s to connected clients.
//!
//! ponytail: stub for Task 7 — accepts the upgrade and immediately closes. Task 9 wires
//! this to `AppState::events` (broadcast receiver -> client sends).

use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use crate::state::AppState;

pub async fn handle(ws: WebSocketUpgrade, State(_st): State<AppState>) -> Response {
    ws.on_upgrade(|socket| async move {
        let _ = socket.close().await;
    })
}

//! WebSocket endpoint: streams chat completions and pushes `ServerEvent` notifications.
//!
//! Wire protocol:
//! - Client→server: `{"type":"chat","id":"<client-id>","cmd":"send_chat"|"quick_chat_send",
//!   "payload":{...}}`. `send_chat` payload is `{"conversationId": <i64>}`;
//!   `quick_chat_send` is `{"message": <str>, "appName": <str>}`.
//! - Server→client (per chat token): `{"type":"chat_event","id":"<same id>","event": <ChatEvent>}`.
//! - Server→client (broadcast): `{"type":"notification","title":...,"body":...}`.

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use donna_core::ops::{self, ChatEvent};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

#[derive(Deserialize)]
struct ChatFrame {
    id: String,
    cmd: String,
    payload: serde_json::Value,
}

#[derive(Deserialize)]
struct SendChatPayload {
    #[serde(rename = "conversationId")]
    conversation_id: i64,
}

#[derive(Deserialize)]
struct QuickChatPayload {
    message: String,
    #[serde(rename = "appName")]
    app_name: String,
}

pub async fn handle(ws: WebSocketUpgrade, State(st): State<AppState>) -> Response {
    ws.on_upgrade(|socket| async move { run(socket, st).await })
}

async fn run(socket: WebSocket, st: AppState) {
    let (mut sink, mut stream) = socket.split();
    // Single outbound channel: notification-forwarder and chat callbacks both feed it,
    // one task owns the sink.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    // Forward broadcast notifications ({"type":"notification",...}) to this client.
    let mut events = st.events.subscribe();
    let notif_tx = out_tx.clone();
    let forwarder = tokio::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match events.recv().await {
                Ok(ev) => {
                    let Ok(json) = serde_json::to_string(&ev) else { continue };
                    if notif_tx.send(json).is_err() {
                        break; // client gone
                    }
                }
                Err(RecvError::Lagged(_)) => continue, // dropped some; keep serving
                Err(RecvError::Closed) => break,
            }
        }
    });

    // Sink drain loop: everything sent to the client goes through here.
    let sender = tokio::spawn(async move {
        while let Some(json) = out_rx.recv().await {
            if sink.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Incoming frames: dispatch each chat request onto its own task.
    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let frame: ChatFrame = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(_) => continue, // ignore malformed frames
        };
        let db = st.db.clone();
        let tx = out_tx.clone();
        tokio::spawn(async move { dispatch(db, frame, tx).await });
    }

    // Client hung up. Dropping out_tx alone isn't enough: `forwarder` still holds
    // notif_tx and is parked on events.recv().await, so out_rx would never see
    // None and `sender` would hang forever waiting for a close that never comes.
    // ponytail: abort forwarder + sender directly instead of select!'ing the read
    // loop against them — same cancellation, smaller diff against the existing
    // spawned-task structure.
    drop(out_tx);
    forwarder.abort();
    sender.abort();
}

/// Run one chat command, forwarding each `ChatEvent` as a `chat_event` frame.
/// Any ops error becomes a final Error `chat_event` — the socket is never dropped for it.
async fn dispatch(db: std::sync::Arc<donna_core::db::Db>, frame: ChatFrame, tx: mpsc::UnboundedSender<String>) {
    let id = frame.id;
    // Bridge the sync `on_event` callback to the async sink via `tx`.
    let emit = {
        let tx = tx.clone();
        let id = id.clone();
        move |ev: ChatEvent| {
            if let Ok(json) = serde_json::to_string(&chat_event_frame(&id, &ev)) {
                let _ = tx.send(json);
            }
        }
    };

    // Unify every failure mode into an error string; ChatEvent::Error is emitted below.
    let err: Option<String> = match frame.cmd.as_str() {
        "send_chat" => match serde_json::from_value::<SendChatPayload>(frame.payload) {
            Ok(p) => ops::send_chat(&db, p.conversation_id, &emit).await.err().map(|e| e.to_string()),
            Err(e) => Some(format!("bad send_chat payload: {e}")),
        },
        "quick_chat_send" => match serde_json::from_value::<QuickChatPayload>(frame.payload) {
            Ok(p) => ops::quick_chat_send(&db, p.message, p.app_name, &emit).await.err().map(|e| e.to_string()),
            Err(e) => Some(format!("bad quick_chat_send payload: {e}")),
        },
        other => Some(format!("unknown cmd: {other}")),
    };

    if let Some(message) = err {
        emit(ChatEvent::Error { message });
    }
}

/// `{"type":"chat_event","id":<id>,"event":<ChatEvent>}` — ChatEvent nests cleanly since
/// its own `"type"` tag lives inside the `event` object.
fn chat_event_frame(id: &str, ev: &ChatEvent) -> serde_json::Value {
    serde_json::json!({ "type": "chat_event", "id": id, "event": ev })
}

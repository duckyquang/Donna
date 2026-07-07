//! WebSocket plumbing: bad-token rejection and end-to-end chat_event streaming.
use futures_util::{SinkExt, StreamExt};

async fn spawn_server() -> (String, donna_server::AppState) {
    let state = donna_server::test_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = donna_server::build_app(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("127.0.0.1:{}", addr.port()), state)
}

#[tokio::test]
async fn ws_rejects_bad_token() {
    let (addr, _st) = spawn_server().await;
    let err = tokio_tungstenite::connect_async(format!("ws://{addr}/ws?token=wrong")).await;
    assert!(err.is_err()); // 401 on upgrade
}

#[tokio::test]
async fn ws_chat_yields_events() {
    let (addr, _st) = spawn_server().await;
    let (mut ws, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws?token=test-token"))
            .await
            .unwrap();
    // No provider key configured: the stream must still answer with an Error
    // chat_event carrying our id — that proves the full WS plumbing.
    ws.send(tokio_tungstenite::tungstenite::Message::text(
        r#"{"type":"chat","id":"t1","cmd":"send_chat","payload":{"conversationId":123456789}}"#,
    ))
    .await
    .unwrap();
    let msg = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next())
        .await
        .expect("no frame within 10s")
        .unwrap()
        .unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["type"], "chat_event");
    assert_eq!(frame["id"], "t1");
    // The event carries a ChatEvent; with no model configured it must be an error.
    assert_eq!(frame["event"]["type"], "error");
}

// Regression test for the per-connection task leak (notification-forwarder parked on
// `events.recv().await` after disconnect, so the sink task's mpsc channel never saw
// `None` and `run()`'s final await hung forever). A clean leak assertion isn't possible
// from outside the process (no task-count introspection), so this is the pragmatic
// version from the task brief: connect, chat, disconnect abruptly (no close handshake,
// to hit the exact leaking path), then prove the server still works end-to-end —
// both a fresh connection completing a chat round trip, and a broadcast notification
// (the same `state.events` the forwarder subscribes to) still reaching a live client.
#[tokio::test]
async fn ws_reconnect_and_broadcast_after_disconnect_still_works() {
    let (addr, st) = spawn_server().await;

    // First connection: send a chat, get the error event, then hang up abruptly.
    let (mut ws1, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws?token=test-token"))
            .await
            .unwrap();
    ws1.send(tokio_tungstenite::tungstenite::Message::text(
        r#"{"type":"chat","id":"a","cmd":"send_chat","payload":{"conversationId":1}}"#,
    ))
    .await
    .unwrap();
    let msg = tokio::time::timeout(std::time::Duration::from_secs(10), ws1.next())
        .await
        .expect("no frame within 10s")
        .unwrap()
        .unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["event"]["type"], "error");
    drop(ws1); // abrupt disconnect, no close frame — the path that used to leak

    // A fresh second connection must still complete a full chat round trip.
    let (mut ws2, _) =
        tokio_tungstenite::connect_async(format!("ws://{addr}/ws?token=test-token"))
            .await
            .unwrap();
    ws2.send(tokio_tungstenite::tungstenite::Message::text(
        r#"{"type":"chat","id":"b","cmd":"send_chat","payload":{"conversationId":2}}"#,
    ))
    .await
    .unwrap();
    let msg = tokio::time::timeout(std::time::Duration::from_secs(10), ws2.next())
        .await
        .expect("no frame within 10s")
        .unwrap()
        .unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["id"], "b");
    assert_eq!(frame["event"]["type"], "error");

    // And a broadcast notification still reaches it — proves the shared `events`
    // sender (and this connection's own forwarder) is healthy, not wedged by whatever
    // connection #1 left behind.
    let _ = st.events.send(donna_server::state::ServerEvent::Notification {
        title: "hi".into(),
        body: "there".into(),
    });
    let msg = tokio::time::timeout(std::time::Duration::from_secs(10), ws2.next())
        .await
        .expect("no notification within 10s")
        .unwrap()
        .unwrap();
    let frame: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(frame["type"], "notification");
}

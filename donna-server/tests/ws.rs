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

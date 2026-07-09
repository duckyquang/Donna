//! WhatsApp webhook integration tests. NO bearer header on any request — that alone
//! proves the routes live outside the auth layer. Signatures are real HMAC-SHA256 over
//! the raw body with the test secret. Inbound processing is spawned, so DB side effects
//! are asserted by polling (the reply/echo sends fail without creds — that's swallowed).

use axum::{body::Body, http::{Request, StatusCode}};
use donna_server::{build_app, test_state};
use donna_core::db::Db;
use hmac::{Hmac, Mac};
use std::sync::Arc;
use tower::util::ServiceExt;

/// `sha256=<hex>` header value for `body` signed with the test secret.
fn sign(body: &str) -> String {
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"test-secret").unwrap();
    mac.update(body.as_bytes());
    let bytes = mac.finalize().into_bytes();
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("sha256={hex}")
}

/// Poll `f` up to ~2s for a DB side effect produced by spawned async work.
async fn poll<T>(db: &Arc<Db>, mut f: impl FnMut(&Db) -> Option<T>) -> Option<T> {
    for _ in 0..40 {
        if let Some(v) = f(db) {
            return Some(v);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

fn user_messages(db: &Db, title: &str) -> Vec<String> {
    let Some(conv) = db.list_conversations().unwrap().into_iter().find(|c| c.title == title) else {
        return vec![];
    };
    db.get_messages(conv.id)
        .unwrap()
        .into_iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content)
        .collect()
}

fn post(body: &str, sig: Option<&str>) -> Request<Body> {
    let mut req = Request::post("/webhook/whatsapp").header("content-type", "application/json");
    if let Some(s) = sig {
        req = req.header("x-hub-signature-256", s);
    }
    req.body(Body::from(body.to_string())).unwrap()
}

const TEXT_PAYLOAD: &str = r#"{"entry":[{"changes":[{"value":{"messages":[{"from":"15550100","id":"wamid.T1","type":"text","text":{"body":"hello"}}]}}]}]}"#;

#[tokio::test]
async fn verify_handshake_accepts_correct_token_and_rejects_wrong() {
    let app = build_app(test_state());
    let res = app
        .clone()
        .oneshot(Request::get("/webhook/whatsapp?hub.mode=subscribe&hub.verify_token=test-verify&hub.challenge=CH123").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    assert_eq!(&body[..], b"CH123");

    let res = app
        .oneshot(Request::get("/webhook/whatsapp?hub.mode=subscribe&hub.verify_token=wrong&hub.challenge=CH123").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn text_message_is_appended_to_whatsapp_conversation() {
    let state = test_state();
    let db = state.db.clone();
    db.set_setting("whatsapp_my_number", "+1 555 0100").unwrap();
    let app = build_app(state);

    let res = app.oneshot(post(TEXT_PAYLOAD, Some(&sign(TEXT_PAYLOAD)))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let msgs = poll(&db, |db| {
        let m = user_messages(db, "WhatsApp");
        (!m.is_empty()).then_some(m)
    })
    .await
    .expect("expected a WhatsApp user message");
    assert_eq!(msgs, vec!["hello".to_string()]);
}

#[tokio::test]
async fn duplicate_delivery_is_deduped() {
    let state = test_state();
    let db = state.db.clone();
    db.set_setting("whatsapp_my_number", "+1 555 0100").unwrap();
    let app = build_app(state);

    let res = app.clone().oneshot(post(TEXT_PAYLOAD, Some(&sign(TEXT_PAYLOAD)))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // Wait for the first to land.
    poll(&db, |db| (!user_messages(db, "WhatsApp").is_empty()).then_some(())).await.unwrap();

    let res = app.oneshot(post(TEXT_PAYLOAD, Some(&sign(TEXT_PAYLOAD)))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // Give any (wrongly) spawned work a chance to run, then confirm still exactly one.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(user_messages(&db, "WhatsApp").len(), 1);
}

#[tokio::test]
async fn bad_signature_is_rejected_and_nothing_stored() {
    let state = test_state();
    let db = state.db.clone();
    db.set_setting("whatsapp_my_number", "+1 555 0100").unwrap();
    let app = build_app(state);

    let res = app.oneshot(post(TEXT_PAYLOAD, Some("sha256=deadbeef"))).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(user_messages(&db, "WhatsApp").is_empty());
}

#[tokio::test]
async fn non_allowlisted_sender_is_ignored() {
    let state = test_state();
    let db = state.db.clone();
    db.set_setting("whatsapp_my_number", "+1 555 0100").unwrap();
    let app = build_app(state);

    let payload = r#"{"entry":[{"changes":[{"value":{"messages":[{"from":"19998887777","id":"wamid.X1","type":"text","text":{"body":"hi"}}]}}]}]}"#;
    let res = app.oneshot(post(payload, Some(&sign(payload)))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(user_messages(&db, "WhatsApp").is_empty());
}

#[tokio::test]
async fn button_reply_resolves_seeded_approval() {
    let state = test_state();
    let db = state.db.clone();
    db.set_setting("whatsapp_my_number", "+1 555 0100").unwrap();
    let conv = db.create_conversation("WhatsApp").unwrap();
    let approval_id = db
        .insert_approval(conv, "slack_send_message", r##"{"channel":"#g","text":"x"}"##, "send x")
        .unwrap();
    let app = build_app(state);

    let payload = format!(
        r#"{{"entry":[{{"changes":[{{"value":{{"messages":[{{"from":"15550100","id":"wamid.B1","type":"interactive","interactive":{{"type":"button_reply","button_reply":{{"id":"reject:{approval_id}","title":"Reject"}}}}}}]}}}}]}}]}}"#
    );
    let res = app.oneshot(post(&payload, Some(&sign(&payload)))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let final_status = poll(&db, |db| {
        let s = db.get_approval(approval_id).unwrap().unwrap().status;
        (s == "rejected").then_some(s)
    })
    .await;
    assert_eq!(final_status.as_deref(), Some("rejected"));
}

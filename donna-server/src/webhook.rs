//! Meta (WhatsApp Cloud API) webhook endpoints. Both live OUTSIDE the bearer-auth
//! layer — Meta authenticates the GET handshake with a shared verify token and every
//! POST with an HMAC-SHA256 signature over the raw body, not with our bearer.
//!
//! Security notes:
//! - The signature is compared in constant time via `Mac::verify_slice`, never a
//!   string `==` on the hex (which would leak length/prefix timing).
//! - With no app secret configured, inbound POSTs are ignored (200, no processing) —
//!   we never process an unverifiable body. Secure by default.

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;

use crate::state::AppState;
use donna_core::ops;

type HmacSha256 = Hmac<Sha256>;

/// GET handshake: echo `hub.challenge` iff `hub.verify_token` matches ours. Query keys
/// contain dots (`hub.verify_token`), so a `HashMap` is the simplest correct extractor.
pub async fn verify(State(st): State<AppState>, Query(q): Query<HashMap<String, String>>) -> Response {
    let ok = matches!((&st.wa_verify_token, q.get("hub.verify_token")), (Some(a), Some(b)) if a == b);
    if ok {
        q.get("hub.challenge").cloned().unwrap_or_default().into_response()
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

/// POST inbound events. Verifies the signature over the RAW bytes, then dispatches each
/// allowlisted message. Dedupe + allowlist run before any spawn; a 200 returns as soon
/// as the payload is walked (handler work continues in the background).
pub async fn receive(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(secret) = st.wa_app_secret.as_deref() else {
        eprintln!("whatsapp webhook: DONNA_WA_APP_SECRET unset — ignoring inbound event");
        return StatusCode::OK.into_response();
    };

    if !signature_ok(secret, &headers, &body) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // Lenient parse: a malformed/foreign body is not our problem to 500 over.
    let Ok(payload) = serde_json::from_slice::<Webhook>(&body) else {
        return Json(serde_json::json!({"status": "ok"})).into_response();
    };

    let allow = st.db.get_setting("whatsapp_my_number").ok().flatten();
    for entry in payload.entry {
        for change in entry.changes {
            for msg in change.value.messages {
                dispatch(&st, allow.as_deref(), msg);
            }
        }
    }
    Json(serde_json::json!({"status": "ok"})).into_response()
}

/// Constant-time verify of `X-Hub-Signature-256: sha256=<hex>` against HMAC-SHA256 of
/// the raw body. Missing header, wrong scheme, malformed hex, or wrong digest → false.
fn signature_ok(secret: &str, headers: &HeaderMap, body: &[u8]) -> bool {
    let Some(sig) = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("sha256="))
    else {
        return false;
    };
    let Some(sig_bytes) = hex_decode(sig) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key length");
    mac.update(body);
    mac.verify_slice(&sig_bytes).is_ok()
}

/// One inbound message → dedupe, allowlist, then spawn the matching handler. Every
/// handler is `spawn`ed with a cloned `Arc<Db>` so the 200 is never blocked on it.
fn dispatch(st: &AppState, allow: Option<&str>, msg: Message) {
    if msg.id.is_empty() {
        return; // No id to dedupe on — don't poison the empty-string claim key.
    }
    if !st.db.try_claim_webhook_event(&msg.id).unwrap_or(false) {
        return; // Meta at-least-once retry — already handled.
    }
    if !allow.is_some_and(|a| digits(a) == digits(&msg.from)) {
        return; // Not the owner's number (or allowlist unset) — ignore silently.
    }

    let db = st.db.clone();
    match msg.r#type.as_str() {
        "text" => {
            let text = msg.text.and_then(|t| t.body).unwrap_or_default();
            tokio::spawn(async move { let _ = ops::whatsapp_handle_text(&db, &text).await; });
        }
        "interactive" => {
            if let Some(id) = msg.interactive.and_then(|i| i.button_reply).map(|b| b.id) {
                tokio::spawn(async move { let _ = ops::whatsapp_handle_button(&db, &id).await; });
            } else {
                polite_reply(db);
            }
        }
        _ => polite_reply(db),
    }
}

/// Best-effort "I only read text" reply for unsupported message types. The sender is
/// already allowlisted; the reply target is the owner's own number. Send failures
/// (e.g. no WhatsApp creds) are swallowed — a webhook must never surface an error.
fn polite_reply(db: std::sync::Arc<donna_core::db::Db>) {
    tokio::spawn(async move {
        if let Ok(Some(number)) = db.get_setting("whatsapp_my_number") {
            let _ = donna_core::integrations::whatsapp::send_message(&number, "I can only read text messages right now.").await;
        }
    });
}

/// Digits only, for tolerant phone-number comparison ("+1 555 0100" == "15550100").
fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// Decode an even-length lowercase/uppercase hex string to bytes. `None` on any
/// non-hex char or odd length. Local so we don't pull in a `hex` crate for ~10 lines.
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

// --- Payload subset. All fields Option/default: Meta's payloads are deep and evolving,
// and status callbacks omit `messages` entirely. Unknown fields are ignored. -------

#[derive(Deserialize, Default)]
struct Webhook {
    #[serde(default)]
    entry: Vec<Entry>,
}

#[derive(Deserialize, Default)]
struct Entry {
    #[serde(default)]
    changes: Vec<Change>,
}

#[derive(Deserialize, Default)]
struct Change {
    #[serde(default)]
    value: Value,
}

#[derive(Deserialize, Default)]
struct Value {
    #[serde(default)]
    messages: Vec<Message>,
}

#[derive(Deserialize, Default)]
struct Message {
    #[serde(default)]
    from: String,
    #[serde(default)]
    id: String,
    #[serde(default, rename = "type")]
    r#type: String,
    text: Option<Text>,
    interactive: Option<Interactive>,
}

#[derive(Deserialize)]
struct Text {
    body: Option<String>,
}

#[derive(Deserialize)]
struct Interactive {
    button_reply: Option<ButtonReply>,
}

#[derive(Deserialize)]
struct ButtonReply {
    #[serde(default)]
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decode_roundtrip_and_rejects_garbage() {
        assert_eq!(hex_decode("00ff10"), Some(vec![0x00, 0xff, 0x10]));
        assert_eq!(hex_decode("abc"), None); // odd length
        assert_eq!(hex_decode("zz"), None); // non-hex
    }

    #[test]
    fn digits_strips_formatting() {
        assert_eq!(digits("+1 555 0100"), "15550100");
        assert_eq!(digits("15550100"), "15550100");
    }
}

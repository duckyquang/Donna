//! WhatsApp Business Cloud API connector.
//!
//! Requires a Meta Business app with WhatsApp Cloud API enabled. Donna stores the
//! permanent access token and phone number id in the keychain.

use reqwest::multipart::{Form, Part};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::secrets;

const TOKEN_KEY: &str = "token:whatsapp";
const PHONE_ID_KEY: &str = "phone_id:whatsapp";

pub fn set_credentials(access_token: &str, phone_number_id: &str) -> Result<()> {
    secrets::set_secret(TOKEN_KEY, access_token)?;
    secrets::set_secret(PHONE_ID_KEY, phone_number_id)
}

pub fn is_connected() -> Result<bool> {
    Ok(secrets::has_secret(TOKEN_KEY)? && secrets::has_secret(PHONE_ID_KEY)?)
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(TOKEN_KEY)?;
    secrets::delete_secret(PHONE_ID_KEY)
}

fn access_token() -> Result<String> {
    secrets::get_secret(TOKEN_KEY)?
        .ok_or_else(|| Error::Provider("WhatsApp is not connected".into()))
}

fn phone_number_id() -> Result<String> {
    secrets::get_secret(PHONE_ID_KEY)?
        .ok_or_else(|| Error::Provider("WhatsApp phone number id is missing".into()))
}

/// Send a plain-text WhatsApp message to a recipient E.164 number (e.g. +14155551234).
pub async fn send_message(to: &str, text: &str) -> Result<()> {
    let phone_id = phone_number_id()?;
    let url = format!("https://graph.facebook.com/v19.0/{phone_id}/messages");
    let resp = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token()?)
        .json(&serde_json::json!({
            "messaging_product": "whatsapp",
            "to": to.trim_start_matches('+'),
            "type": "text",
            "text": { "body": text }
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("WhatsApp API error: {detail}")));
    }
    Ok(())
}

/// Truncate `s` to at most `max_bytes` bytes, backing off to the nearest char
/// boundary so the result is always valid UTF-8.
fn truncate_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Build the interactive button-message body for an approval request. Pure so it's
/// unit-testable without a network call. `summary` is truncated to keep the body
/// (including the "Approval needed:\n" prefix) within WhatsApp's 1024-BYTE limit.
fn approval_buttons_body(to_digits: &str, approval_id: i64, summary: &str) -> serde_json::Value {
    const PREFIX: &str = "Approval needed:\n";
    let max_summary_bytes = (1024 - PREFIX.len()).min(900);
    let truncated = truncate_bytes(summary, max_summary_bytes);
    serde_json::json!({
        "messaging_product": "whatsapp",
        "to": to_digits,
        "type": "interactive",
        "interactive": {
            "type": "button",
            "body": { "text": format!("{PREFIX}{truncated}") },
            "action": { "buttons": [
                { "type": "reply", "reply": { "id": format!("approve:{approval_id}"), "title": "Approve" } },
                { "type": "reply", "reply": { "id": format!("reject:{approval_id}"), "title": "Reject" } }
            ]}
        }
    })
}

/// Send an interactive approve/reject button message for a pending approval.
pub async fn send_approval_buttons(to: &str, approval_id: i64, summary: &str) -> Result<()> {
    let phone_id = phone_number_id()?;
    let url = format!("https://graph.facebook.com/v19.0/{phone_id}/messages");
    let body = approval_buttons_body(to.trim_start_matches('+'), approval_id, summary);
    let resp = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token()?)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("WhatsApp API error: {detail}")));
    }
    Ok(())
}

/// Download inbound media by id: resolve the CDN url via the Graph API, then fetch
/// the bytes from that url. Both requests need the bearer token — Meta's media CDN
/// checks it too, not just the Graph lookup.
pub async fn download_media(media_id: &str) -> Result<Vec<u8>> {
    let url = format!("https://graph.facebook.com/v19.0/{media_id}");
    let resp = reqwest::Client::new().get(url).bearer_auth(access_token()?).send().await?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("WhatsApp API error: {detail}")));
    }
    let meta: serde_json::Value = resp.json().await?;
    let media_url = meta["url"]
        .as_str()
        .ok_or_else(|| Error::Provider("WhatsApp media lookup missing url".into()))?;

    let resp = reqwest::Client::new().get(media_url).bearer_auth(access_token()?).send().await?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("WhatsApp API error: {detail}")));
    }
    Ok(resp.bytes().await?.to_vec())
}

/// Upload media bytes to WhatsApp, returning the resulting media id.
pub async fn upload_media(bytes: Vec<u8>, mime: &str) -> Result<String> {
    let phone_id = phone_number_id()?;
    let url = format!("https://graph.facebook.com/v19.0/{phone_id}/media");
    let form = Form::new()
        .text("messaging_product", "whatsapp")
        .text("type", mime.to_string())
        .part("file", Part::bytes(bytes).file_name("donna.ogg").mime_str(mime)?);
    let resp = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token()?)
        .multipart(form)
        .send()
        .await?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("WhatsApp API error: {detail}")));
    }
    let body: serde_json::Value = resp.json().await?;
    body["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Provider("WhatsApp media upload missing id".into()))
}

/// Build the outbound voice-note message body. Pure so it's unit-testable without a
/// network call.
fn voice_note_body(to_digits: &str, media_id: &str) -> serde_json::Value {
    serde_json::json!({
        "messaging_product": "whatsapp",
        "to": to_digits,
        "type": "audio",
        "audio": { "id": media_id }
    })
}

/// Upload `audio` as a voice note and send it to `to`.
pub async fn send_voice_note(to: &str, audio: Vec<u8>) -> Result<()> {
    let media_id = upload_media(audio, "audio/ogg").await?;
    let phone_id = phone_number_id()?;
    let url = format!("https://graph.facebook.com/v19.0/{phone_id}/messages");
    let body = voice_note_body(to.trim_start_matches('+'), &media_id);
    let resp = reqwest::Client::new().post(url).bearer_auth(access_token()?).json(&body).send().await?;
    if !resp.status().is_success() {
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("WhatsApp API error: {detail}")));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct WhatsAppStatus {
    pub connected: bool,
}

pub fn status() -> Result<WhatsAppStatus> {
    Ok(WhatsAppStatus {
        connected: is_connected()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_buttons_shape_and_truncation() {
        let v = approval_buttons_body("15550100", 42, &"x".repeat(2000));
        assert_eq!(v["interactive"]["action"]["buttons"][0]["reply"]["id"], "approve:42");
        assert_eq!(v["interactive"]["action"]["buttons"][1]["reply"]["id"], "reject:42");
        assert!(v["interactive"]["body"]["text"].as_str().unwrap().len() <= 1024);
        assert!(v["interactive"]["action"]["buttons"][0]["reply"]["title"].as_str().unwrap().len() <= 20);
    }

    #[test]
    fn approval_buttons_truncate_multibyte_by_bytes() {
        // Meta's 1024 limit is bytes, not chars. A multibyte summary must still
        // produce a body <= 1024 bytes; slicing on a non-char-boundary would
        // panic, so this test passing also proves boundary safety.
        let v = approval_buttons_body("15550100", 42, &"🎉".repeat(2000));
        let body_text = v["interactive"]["body"]["text"].as_str().unwrap();
        assert!(body_text.len() <= 1024, "body must be <= 1024 bytes, was {}", body_text.len());
    }

    #[test]
    fn voice_note_message_body() {
        let b = voice_note_body("15550100", "MEDIA123");
        assert_eq!(b["type"], "audio");
        assert_eq!(b["audio"]["id"], "MEDIA123");
        assert_eq!(b["to"], "15550100");
    }
}

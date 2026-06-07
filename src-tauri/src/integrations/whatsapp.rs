//! WhatsApp Business Cloud API connector.
//!
//! Requires a Meta Business app with WhatsApp Cloud API enabled. Donna stores the
//! permanent access token and phone number id in the keychain.

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

#[derive(Debug, Serialize)]
pub struct WhatsAppStatus {
    pub connected: bool,
}

pub fn status() -> Result<WhatsAppStatus> {
    Ok(WhatsAppStatus {
        connected: is_connected()?,
    })
}

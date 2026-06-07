//! Telegram connector (bot token + chat id).

use serde::Serialize;

use crate::error::{Error, Result};
use crate::secrets;

const TOKEN_KEY: &str = "token:telegram";
const CHAT_KEY: &str = "chat_id:telegram";

pub fn set_credentials(bot_token: &str, chat_id: &str) -> Result<()> {
    secrets::set_secret(TOKEN_KEY, bot_token)?;
    secrets::set_secret(CHAT_KEY, chat_id)
}

pub fn is_connected() -> Result<bool> {
    Ok(secrets::has_secret(TOKEN_KEY)? && secrets::has_secret(CHAT_KEY)?)
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(TOKEN_KEY)?;
    secrets::delete_secret(CHAT_KEY)
}

fn bot_token() -> Result<String> {
    secrets::get_secret(TOKEN_KEY)?
        .ok_or_else(|| Error::Provider("Telegram is not connected".into()))
}

fn chat_id() -> Result<String> {
    secrets::get_secret(CHAT_KEY)?
        .ok_or_else(|| Error::Provider("Telegram chat id is missing".into()))
}

pub async fn send_message(text: &str) -> Result<()> {
    let token = bot_token()?;
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let resp = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({
            "chat_id": chat_id()?,
            "text": text,
        }))
        .send()
        .await?;
    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        let desc = body
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("unknown");
        return Err(Error::Provider(format!("Telegram error: {desc}")));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct TelegramStatus {
    pub connected: bool,
}

pub fn status() -> Result<TelegramStatus> {
    Ok(TelegramStatus {
        connected: is_connected()?,
    })
}

//! Slack connector (token-based).
//!
//! The user provides a Slack bot token (xoxb-…), stored in the keychain. Donna can list
//! channels and post messages. Full OAuth app install can come later.

use serde::Serialize;

use crate::error::{Error, Result};
use crate::secrets;

const TOKEN_KEY: &str = "token:slack";

#[derive(Debug, Serialize)]
pub struct SlackChannel {
    pub id: String,
    pub name: String,
}

pub fn set_token(token: &str) -> Result<()> {
    secrets::set_secret(TOKEN_KEY, token)
}

pub fn is_connected() -> Result<bool> {
    secrets::has_secret(TOKEN_KEY)
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(TOKEN_KEY)
}

fn token() -> Result<String> {
    secrets::get_secret(TOKEN_KEY)?.ok_or_else(|| Error::Provider("Slack is not connected".into()))
}

pub async fn list_channels() -> Result<Vec<SlackChannel>> {
    let resp = reqwest::Client::new()
        .get("https://slack.com/api/conversations.list")
        .bearer_auth(token()?)
        .query(&[("types", "public_channel,private_channel"), ("limit", "200")])
        .send()
        .await?;
    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        let err = body.get("error").and_then(|e| e.as_str()).unwrap_or("unknown");
        return Err(Error::Provider(format!("Slack error: {err}")));
    }
    let channels = body
        .get("channels")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(channels
        .iter()
        .filter_map(|c| {
            Some(SlackChannel {
                id: c.get("id")?.as_str()?.to_string(),
                name: c.get("name")?.as_str()?.to_string(),
            })
        })
        .collect())
}

pub async fn send_message(channel: &str, text: &str) -> Result<()> {
    let resp = reqwest::Client::new()
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token()?)
        .json(&serde_json::json!({ "channel": channel, "text": text }))
        .send()
        .await?;
    let body: serde_json::Value = resp.json().await?;
    if body.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        let err = body.get("error").and_then(|e| e.as_str()).unwrap_or("unknown");
        return Err(Error::Provider(format!("Slack error: {err}")));
    }
    Ok(())
}

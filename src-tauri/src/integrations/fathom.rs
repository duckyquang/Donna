//! Fathom connector (API key based).
//!
//! Stores the user's Fathom API key in the keychain so Donna can pull meeting recaps
//! and transcripts.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::secrets;

const KEY: &str = "api_key:fathom";
const API_BASE: &str = "https://api.fathom.ai/external/v1";

pub fn set_key(key: &str) -> Result<()> {
    secrets::set_secret(KEY, key)
}

pub fn is_connected() -> Result<bool> {
    secrets::has_secret(KEY)
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(KEY)
}

fn api_key() -> Result<String> {
    secrets::get_secret(KEY)?
        .ok_or_else(|| Error::Provider("Fathom is not connected".into()))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FathomMeeting {
    pub id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub created_at: Option<String>,
    pub url: Option<String>,
}

fn parse_meeting(v: &serde_json::Value) -> Option<FathomMeeting> {
    let id = v
        .get("recording_id")
        .or_else(|| v.get("id"))
        .and_then(|x| x.as_str())
        .map(String::from)?;
    Some(FathomMeeting {
        id,
        title: v
            .get("title")
            .or_else(|| v.get("meeting_title"))
            .and_then(|x| x.as_str())
            .map(String::from),
        summary: v
            .get("default_summary")
            .or_else(|| v.get("summary"))
            .and_then(|x| x.as_str())
            .map(String::from),
        created_at: v
            .get("created_at")
            .or_else(|| v.get("recorded_at"))
            .and_then(|x| x.as_str())
            .map(String::from),
        url: v
            .get("share_url")
            .or_else(|| v.get("url"))
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

/// List recent Fathom meetings (most recent first).
pub async fn list_recent_meetings(limit: u32) -> Result<Vec<FathomMeeting>> {
    let key = api_key()?;
    let resp = reqwest::Client::new()
        .get(format!("{API_BASE}/meetings"))
        .header("X-Api-Key", key)
        .query(&[("limit", limit.to_string())])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Fathom API error ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    let items = body
        .get("items")
        .or_else(|| body.get("meetings"))
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items.iter().filter_map(parse_meeting).collect())
}

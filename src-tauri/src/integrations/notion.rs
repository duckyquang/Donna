//! Notion connector (internal integration token).

use serde::Serialize;

use crate::error::{Error, Result};
use crate::secrets;

const TOKEN_KEY: &str = "token:notion";
const NOTION_VERSION: &str = "2022-06-28";

#[derive(Debug, Serialize)]
pub struct NotionPage {
    pub id: String,
    pub title: String,
    pub url: String,
    pub last_edited: Option<String>,
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
    secrets::get_secret(TOKEN_KEY)?
        .ok_or_else(|| Error::Provider("Notion is not connected".into()))
}

fn page_title(props: &serde_json::Value) -> String {
    props
        .as_object()
        .and_then(|obj| {
            obj.values().find_map(|v| {
                v.get("type")
                    .and_then(|t| t.as_str())
                    .filter(|t| *t == "title")
                    .and_then(|_| v.get("title"))
                    .and_then(|arr| arr.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|t| t.get("plain_text"))
                    .and_then(|s| s.as_str())
            })
        })
        .unwrap_or("(Untitled)")
        .to_string()
}

pub async fn search_pages(limit: u32) -> Result<Vec<NotionPage>> {
    let resp = reqwest::Client::new()
        .post("https://api.notion.com/v1/search")
        .header("Authorization", format!("Bearer {}", token()?))
        .header("Notion-Version", NOTION_VERSION)
        .json(&serde_json::json!({
            "page_size": limit,
            "filter": { "value": "page", "property": "object" }
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Notion API error ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    let results = body
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(results
        .iter()
        .filter_map(|p| {
            Some(NotionPage {
                id: p.get("id")?.as_str()?.to_string(),
                title: page_title(p.get("properties")?),
                url: p.get("url")?.as_str()?.to_string(),
                last_edited: p
                    .get("last_edited_time")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
        })
        .collect())
}

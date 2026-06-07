//! Linear connector (API key).

use serde::Serialize;

use crate::error::{Error, Result};
use crate::secrets;

const KEY: &str = "api_key:linear";

#[derive(Debug, Serialize)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub state: String,
    pub url: String,
}

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
        .ok_or_else(|| Error::Provider("Linear is not connected".into()))
}

pub async fn list_issues(limit: u32) -> Result<Vec<LinearIssue>> {
    let query = format!(
        "query {{ issues(first: {limit}, filter: {{ state: {{ type: {{ nin: [\"completed\", \"canceled\"] }} }} }}) {{ nodes {{ id identifier title url state {{ name }} }} }} }}"
    );
    let resp = reqwest::Client::new()
        .post("https://api.linear.app/graphql")
        .header("Authorization", api_key()?)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Linear API error ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    if let Some(errors) = body.get("errors") {
        return Err(Error::Provider(format!("Linear error: {errors}")));
    }
    let nodes = body
        .pointer("/data/issues/nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(nodes
        .iter()
        .filter_map(|n| {
            Some(LinearIssue {
                id: n.get("id")?.as_str()?.to_string(),
                identifier: n.get("identifier")?.as_str()?.to_string(),
                title: n.get("title")?.as_str()?.to_string(),
                state: n
                    .get("state")
                    .and_then(|s| s.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                url: n.get("url")?.as_str()?.to_string(),
            })
        })
        .collect())
}

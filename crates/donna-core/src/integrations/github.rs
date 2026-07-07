//! GitHub connector (personal access token).

use serde::Serialize;

use crate::error::{Error, Result};
use crate::secrets;

const TOKEN_KEY: &str = "token:github";

#[derive(Debug, Serialize)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
}

#[derive(Debug, Serialize)]
pub struct GitHubIssue {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub html_url: String,
    pub repo: String,
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
        .ok_or_else(|| Error::Provider("GitHub is not connected".into()))
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("Donna-App")
        .build()
        .map_err(|e| Error::Provider(e.to_string()))?)
}

pub async fn list_repos(limit: u32) -> Result<Vec<GitHubRepo>> {
    let resp = client()?
        .get("https://api.github.com/user/repos")
        .bearer_auth(token()?)
        .query(&[
            ("per_page", limit.to_string()),
            ("sort", "updated".into()),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "GitHub API error ({})",
            resp.status()
        )));
    }
    let body: Vec<serde_json::Value> = resp.json().await?;
    Ok(body
        .iter()
        .filter_map(|r| {
            Some(GitHubRepo {
                id: r.get("id")?.as_i64()?,
                name: r.get("name")?.as_str()?.to_string(),
                full_name: r.get("full_name")?.as_str()?.to_string(),
                private: r.get("private").and_then(|v| v.as_bool()).unwrap_or(false),
                html_url: r.get("html_url")?.as_str()?.to_string(),
            })
        })
        .collect())
}

pub async fn list_issues(limit: u32) -> Result<Vec<GitHubIssue>> {
    let resp = client()?
        .get("https://api.github.com/user/issues")
        .bearer_auth(token()?)
        .query(&[
            ("per_page", limit.to_string()),
            ("state", "open".into()),
            ("filter", "assigned".into()),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "GitHub API error ({})",
            resp.status()
        )));
    }
    let body: Vec<serde_json::Value> = resp.json().await?;
    Ok(body
        .iter()
        .filter_map(|i| {
            let repo = i.get("repository")?;
            Some(GitHubIssue {
                id: i.get("id")?.as_i64()?,
                number: i.get("number")?.as_i64()?,
                title: i.get("title")?.as_str()?.to_string(),
                state: i.get("state")?.as_str()?.to_string(),
                html_url: i.get("html_url")?.as_str()?.to_string(),
                repo: repo.get("full_name")?.as_str()?.to_string(),
            })
        })
        .collect())
}

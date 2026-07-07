//! News integration — Hacker News top stories for daily tech digest.

use serde::{Deserialize, Serialize};
use crate::error::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewsItem {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub score: u32,
    pub by: String,
}

/// Fetch top N Hacker News stories.
pub async fn top_stories(limit: usize) -> Result<Vec<NewsItem>> {
    let client = reqwest::Client::new();
    let ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await?
        .json()
        .await?;

    let mut items = Vec::new();
    for id in ids.iter().take(limit) {
        let url = format!("https://hacker-news.firebaseio.com/v0/item/{id}.json");
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(item) = resp.json::<serde_json::Value>().await {
                let title = item["title"].as_str().unwrap_or("(no title)").to_string();
                let link = item["url"].as_str().map(|s| s.to_string());
                let score = item["score"].as_u64().unwrap_or(0) as u32;
                let by = item["by"].as_str().unwrap_or("unknown").to_string();
                let hn_id = item["id"].as_u64().unwrap_or(*id);
                items.push(NewsItem { id: hn_id, title, url: link, score, by });
            }
        }
    }
    Ok(items)
}

/// Format top stories into a readable digest string.
pub fn format_digest(items: &[NewsItem]) -> String {
    let mut out = String::from("## 🗞️ Today's Top Tech Stories (Hacker News)\n\n");
    for (i, item) in items.iter().enumerate() {
        let fallback = format!("https://news.ycombinator.com/item?id={}", item.id);
        let link = item.url.as_deref().unwrap_or(&fallback);
        out.push_str(&format!("{}. **{}** (↑{} by {})\n   {}\n\n", i + 1, item.title, item.score, item.by, link));
    }
    out
}

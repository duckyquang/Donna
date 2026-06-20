use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HnItem {
    id: Option<u64>,
    title: Option<String>,
    url: Option<String>,
    score: Option<u32>,
    by: Option<String>,
}

pub struct NewsItem {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub score: u32,
    pub by: String,
}

pub async fn top_stories(limit: usize) -> Result<Vec<NewsItem>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let ids: Vec<u64> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send().await?.json().await?;

    let mut items = Vec::new();
    for id in ids.iter().take(limit) {
        let url = format!("https://hacker-news.firebaseio.com/v0/item/{id}.json");
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(item) = resp.json::<HnItem>().await {
                items.push(NewsItem {
                    id: item.id.unwrap_or(*id),
                    title: item.title.unwrap_or_else(|| "(no title)".into()),
                    url: item.url,
                    score: item.score.unwrap_or(0),
                    by: item.by.unwrap_or_else(|| "unknown".into()),
                });
            }
        }
    }
    Ok(items)
}

pub fn format_digest(items: &[NewsItem]) -> String {
    let mut out = String::from("🗞️ *Today's Top Tech Stories*\n\n");
    for (i, item) in items.iter().enumerate() {
        let fallback = format!("https://news.ycombinator.com/item?id={}", item.id);
        let link = item.url.as_deref().unwrap_or(&fallback);
        out.push_str(&format!("{}. *{}* (↑{} by @{})\n{}\n\n", i + 1, item.title, item.score, item.by, link));
    }
    out
}

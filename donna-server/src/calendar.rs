use serde::Deserialize;
use chrono::Local;
use crate::config::Config;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Clone)]
pub struct CalEvent {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub start_time: Option<String>,
    pub attendees: Vec<String>,
    pub minutes_until: i64,
}

async fn access_token(cfg: &Config) -> Result<String, Box<dyn std::error::Error>> {
    let params = [
        ("client_id", cfg.google_client_id.as_deref().unwrap_or("")),
        ("client_secret", cfg.google_client_secret.as_deref().unwrap_or("")),
        ("refresh_token", cfg.google_refresh_token.as_deref().unwrap_or("")),
        ("grant_type", "refresh_token"),
    ];
    let resp: TokenResponse = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send().await?.json().await?;
    Ok(resp.access_token)
}

pub async fn today_events(cfg: &Config) -> Result<Vec<CalEvent>, Box<dyn std::error::Error>> {
    let now = Local::now();
    let end = now + chrono::Duration::hours(24);
    fetch_events(cfg, &now.to_rfc3339(), &end.to_rfc3339()).await
}

pub async fn this_week_events(cfg: &Config) -> Result<Vec<CalEvent>, Box<dyn std::error::Error>> {
    let now = Local::now();
    let week_ago = now - chrono::Duration::days(7);
    fetch_events(cfg, &week_ago.to_rfc3339(), &now.to_rfc3339()).await
}

pub async fn upcoming_in_minutes(cfg: &Config, within: i64) -> Result<Vec<CalEvent>, Box<dyn std::error::Error>> {
    let now = Local::now();
    let end = now + chrono::Duration::minutes(within);
    fetch_events(cfg, &now.to_rfc3339(), &end.to_rfc3339()).await
}

async fn fetch_events(cfg: &Config, time_min: &str, time_max: &str) -> Result<Vec<CalEvent>, Box<dyn std::error::Error>> {
    if cfg.google_refresh_token.is_none() {
        return Ok(vec![]);
    }
    let token = access_token(cfg).await?;
    let url = format!(
        "https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={time_min}&timeMax={time_max}&singleEvents=true&orderBy=startTime"
    );
    let resp: serde_json::Value = reqwest::Client::new()
        .get(&url).bearer_auth(&token).send().await?.json().await?;

    let now = Local::now();
    let mut events = Vec::new();
    if let Some(items) = resp["items"].as_array() {
        for item in items {
            let summary = item["summary"].as_str().map(|s| s.to_string());
            let description = item["description"].as_str().map(|s| s.to_string());
            let start_raw = item["start"]["dateTime"].as_str()
                .or_else(|| item["start"]["date"].as_str())
                .unwrap_or("")
                .to_string();
            let attendees: Vec<String> = item["attendees"].as_array()
                .map(|arr| arr.iter().filter_map(|a| a["email"].as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let minutes_until = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&start_raw) {
                (dt.with_timezone(&Local) - now).num_minutes()
            } else { 0 };
            events.push(CalEvent { summary, description, start_time: Some(start_raw), attendees, minutes_until });
        }
    }
    Ok(events)
}

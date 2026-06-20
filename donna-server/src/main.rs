//! donna-server — always-on headless companion for Donna.
//!
//! Runs on any Linux machine (VPS, Raspberry Pi, Docker).
//! All config via environment variables. See .env.example.

mod calendar;
mod config;
mod messaging;
mod news;
mod weather;

use std::time::Duration;
use chrono::{Local, Timelike, Datelike};

#[tokio::main]
async fn main() {
    println!("🤖 donna-server starting…");
    let cfg = config::Config::from_env();
    println!("  Provider: {}", cfg.ai_provider);
    println!("  WhatsApp: {}", if cfg.whatsapp_token.is_some() { "connected" } else { "not set" });
    println!("  Telegram: {}", if cfg.telegram_token.is_some() { "connected" } else { "not set" });
    println!("  Calendar: {}", if cfg.google_refresh_token.is_some() { "connected" } else { "not set" });
    println!("  News hour: {}:00", cfg.news_hour);
    println!("  Briefing hour: {}:00", cfg.briefing_hour);
    println!("Scheduler loop started (60s tick).\n");

    let mut last_news_day: Option<u32> = None;
    let mut last_briefing_day: Option<u32> = None;
    let mut last_weekly_review_week: Option<u32> = None;

    loop {
        let now = Local::now();
        let hour = now.hour();
        let minute = now.minute();
        let day = now.ordinal();
        let weekday = now.weekday().num_days_from_monday(); // 0=Mon, 6=Sun

        // Morning briefing
        if hour == cfg.briefing_hour && minute < 2 && last_briefing_day != Some(day) {
            last_briefing_day = Some(day);
            println!("[{}] Running morning briefing…", now.format("%Y-%m-%d %H:%M"));
            if let Err(e) = run_morning_briefing(&cfg).await {
                eprintln!("Morning briefing error: {e}");
            }
        }

        // Daily tech news
        if hour == cfg.news_hour && minute < 2 && last_news_day != Some(day) {
            last_news_day = Some(day);
            println!("[{}] Fetching tech news…", now.format("%Y-%m-%d %H:%M"));
            if let Err(e) = run_tech_news(&cfg).await {
                eprintln!("Tech news error: {e}");
            }
        }

        // Weekly review — Sunday at 20:00
        if weekday == 6 && hour == 20 && minute < 2 && last_weekly_review_week != Some(now.iso_week().week()) {
            last_weekly_review_week = Some(now.iso_week().week());
            println!("[{}] Running weekly review…", now.format("%Y-%m-%d %H:%M"));
            if let Err(e) = run_weekly_review(&cfg).await {
                eprintln!("Weekly review error: {e}");
            }
        }

        // Pre-meeting briefings (check every minute)
        if let Err(e) = run_meeting_briefings(&cfg).await {
            eprintln!("Meeting briefing error: {e}");
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn run_morning_briefing(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = vec!["## ☀️ Good morning! Here's your briefing:\n".to_string()];

    // Weather
    if let (Some(lat), Some(lon)) = (cfg.lat, cfg.lon) {
        if let Ok(w) = weather::fetch(lat, lon).await {
            parts.push(format!("**🌤 Weather:** {}\n", weather::format_summary(&w)));
        }
    }

    // Calendar events today
    if cfg.google_refresh_token.is_some() {
        if let Ok(events) = calendar::today_events(cfg).await {
            if events.is_empty() {
                parts.push("**📅 Calendar:** No events today — clear day!\n".to_string());
            } else {
                let lines: Vec<String> = events.iter().map(|e| {
                    format!("- {} at {}", e.summary.as_deref().unwrap_or("Meeting"), e.start_time.as_deref().unwrap_or("TBD"))
                }).collect();
                parts.push(format!("**📅 Today's calendar:**\n{}\n", lines.join("\n")));
            }
        }
    }

    parts.push("\nHave a great day! 💪".to_string());
    let message = parts.join("\n");
    messaging::send(cfg, &message).await?;
    Ok(())
}

async fn run_tech_news(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let stories = news::top_stories(12).await?;
    let digest = news::format_digest(&stories);

    // AI summary if provider configured
    let message = if cfg.ai_provider != "none" {
        match ai_summarize(cfg, &digest).await {
            Ok(summary) => format!("{}\n\n---\n*Donna's take:* {}", digest, summary),
            Err(_) => digest,
        }
    } else {
        digest
    };

    messaging::send(cfg, &message).await?;
    Ok(())
}

async fn run_meeting_briefings(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.google_refresh_token.is_none() {
        return Ok(());
    }
    let events = calendar::upcoming_in_minutes(cfg, 32).await?;
    for event in events {
        // Only brief events starting in ~30 minutes
        let start_mins = event.minutes_until;
        if start_mins < 28 || start_mins > 35 {
            continue;
        }
        let title = event.summary.as_deref().unwrap_or("Meeting");
        let attendees = event.attendees.join(", ");
        let description = event.description.as_deref().unwrap_or("(no description)");

        let message = if cfg.ai_provider != "none" {
            let prompt = format!(
                "Brief me on this upcoming meeting in 3-5 bullet points. Meeting: {title}\nAttendees: {attendees}\nDescription: {description}\n\nBe concise and actionable — what should I know and prepare?"
            );
            ai_complete(cfg, &prompt).await.unwrap_or_else(|_| {
                format!("📅 **Meeting in 30 min: {title}**\nAttendees: {attendees}")
            })
        } else {
            format!("📅 **Meeting in 30 min: {title}**\nAttendees: {attendees}\n{description}")
        };

        messaging::send(cfg, &message).await?;
    }
    Ok(())
}

async fn run_weekly_review(cfg: &config::Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = vec!["## 📊 Weekly Review\n".to_string()];

    if cfg.google_refresh_token.is_some() {
        if let Ok(events) = calendar::this_week_events(cfg).await {
            if !events.is_empty() {
                let lines: Vec<String> = events.iter().map(|e| {
                    format!("- {} ({})", e.summary.as_deref().unwrap_or("Event"), e.start_time.as_deref().unwrap_or(""))
                }).collect();
                parts.push(format!("**This week's meetings:**\n{}\n", lines.join("\n")));
            }
        }
    }

    parts.push("**Reflection prompts:**\n- What did you accomplish this week?\n- What's carrying over to next week?\n- Who should you reconnect with?\n".to_string());

    let message = parts.join("\n");
    messaging::send(cfg, &message).await?;
    Ok(())
}

async fn ai_complete(cfg: &config::Config, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    match cfg.ai_provider.as_str() {
        "openai" => {
            let key = cfg.ai_key.as_deref().unwrap_or("");
            let body = serde_json::json!({
                "model": cfg.ai_model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": 500
            });
            let resp: serde_json::Value = reqwest::Client::new()
                .post("https://api.openai.com/v1/chat/completions")
                .bearer_auth(key)
                .json(&body)
                .send().await?.json().await?;
            Ok(resp["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
        }
        "anthropic" => {
            let key = cfg.ai_key.as_deref().unwrap_or("");
            let body = serde_json::json!({
                "model": cfg.ai_model,
                "max_tokens": 500,
                "messages": [{"role": "user", "content": prompt}]
            });
            let resp: serde_json::Value = reqwest::Client::new()
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send().await?.json().await?;
            Ok(resp["content"][0]["text"].as_str().unwrap_or("").to_string())
        }
        "ollama" => {
            let host = cfg.ollama_url.as_deref().unwrap_or("http://localhost:11434");
            let body = serde_json::json!({
                "model": cfg.ai_model,
                "prompt": prompt,
                "stream": false
            });
            let resp: serde_json::Value = reqwest::Client::new()
                .post(format!("{host}/api/generate"))
                .json(&body)
                .send().await?.json().await?;
            Ok(resp["response"].as_str().unwrap_or("").to_string())
        }
        _ => Err("No AI provider configured".into()),
    }
}

async fn ai_summarize(cfg: &config::Config, content: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = format!("You are Donna. Based on these tech stories, give a 2-3 sentence take on the most important themes and what the user should pay attention to:\n\n{content}");
    ai_complete(cfg, &prompt).await
}

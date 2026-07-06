//! Background scheduler — evaluates routines and emits proactive notifications.
//!
//! Portable: takes `Arc<Db>` and a `Notifier` instead of a Tauri `AppHandle`, so both
//! the desktop app and donna-server can drive it (desktop currently doesn't — see
//! `// ponytail: one brain, one scheduler` in src-tauri/src/lib.rs).

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Timelike, Utc};

use crate::db::{Db, Routine};
use crate::docs;
use crate::error::Result;
use crate::integrations::{fathom, google};
use crate::knowledge;
use crate::providers::{self, ChatTurn};
use crate::retrieval;
use crate::secrets;

/// Display a system/OS notification. Implemented by the desktop app (Tauri plugin)
/// and by donna-server (no-op or push, per Task 9).
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str);
}

/// Start the 60-second scheduler loop and seed built-in routines once.
pub fn run_loop(db: Arc<Db>, notifier: Arc<dyn Notifier>) {
    tokio::spawn(async move {
        let _ = db.seed_builtin_routines();

        loop {
            if let Err(e) = tick(&db, &notifier).await {
                eprintln!("scheduler tick error: {e}");
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

struct DueRoutine {
    routine: Routine,
    /// Extra context for one-off runs (e.g. a specific calendar event).
    context: Option<String>,
    dedupe_key: Option<String>,
}

/// Look up setting `timezone` and parse it as an IANA name (e.g. "America/New_York").
fn configured_tz(db: &Db) -> Option<chrono_tz::Tz> {
    db.get_setting("timezone")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<chrono_tz::Tz>().ok())
}

async fn tick(db: &Db, notifier: &Arc<dyn Notifier>) -> Result<()> {
    let routines = db.list_routines()?;
    // Prefer the user's configured timezone; fall back to the system-local one
    // when the setting is unset or fails to parse as a `chrono_tz::Tz`.
    let due = match configured_tz(db) {
        Some(tz) => collect_due_routines(db, &routines, Utc::now().with_timezone(&tz)).await?,
        None => collect_due_routines(db, &routines, Local::now()).await?,
    };

    for item in due {
        if let Err(e) = execute_routine(db, notifier, &item).await {
            eprintln!("routine {} failed: {e}", item.routine.name);
        }
    }
    Ok(())
}

async fn collect_due_routines<Tz: TimeZone>(
    db: &Db,
    routines: &[Routine],
    now: DateTime<Tz>,
) -> Result<Vec<DueRoutine>> {
    let mut due = Vec::new();

    for routine in routines {
        if !routine.enabled {
            continue;
        }
        match routine.schedule_type.as_str() {
            "daily" | "weekly" => {
                if is_due(routine, &now) {
                    due.push(DueRoutine {
                        routine: routine.clone(),
                        context: None,
                        dedupe_key: None,
                    });
                }
            }
            "before_meeting" => {
                let minutes = routine.minutes_before.unwrap_or(30);
                if let Ok(events) = upcoming_meetings(minutes + 5).await {
                    for ev in events {
                        let Some(event_id) = ev.id.clone() else {
                            continue;
                        };
                        if db.has_routine_dedupe(routine.id, &event_id)? {
                            continue;
                        }
                        let Some(start) = parse_event_start(&ev.start) else {
                            continue;
                        };
                        let mins_until = (start - Local::now()).num_minutes();
                        if mins_until >= minutes.saturating_sub(1) as i64
                            && mins_until <= minutes.saturating_add(1) as i64
                        {
                            let title = ev
                                .summary
                                .clone()
                                .unwrap_or_else(|| "Upcoming meeting".into());
                            let ctx = format!(
                                "Meeting: {title}\nStarts: {}\nDescription: {}",
                                ev.start,
                                ev.description.as_deref().unwrap_or("(none)")
                            );
                            due.push(DueRoutine {
                                routine: routine.clone(),
                                context: Some(ctx),
                                dedupe_key: Some(event_id),
                            });
                        }
                    }
                }
            }
            "after_meeting" => {
                let minutes = routine.minutes_before.unwrap_or(10);
                if let Ok(events) = recently_ended_meetings(minutes).await {
                    for ev in events {
                        let Some(event_id) = ev.id.clone() else { continue; };
                        if db.has_routine_dedupe(routine.id, &event_id)? { continue; }
                        let title = ev.summary.clone().unwrap_or_else(|| "Meeting".into());
                        let ctx = format!("Meeting just ended: {title}");
                        due.push(DueRoutine {
                            routine: routine.clone(),
                            context: Some(ctx),
                            dedupe_key: Some(event_id),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(due)
}

/// Pure schedule-matching: is `routine` due at `now`, given its schedule type,
/// hour/minute/day-of-week, and `last_run_at` (deduped per slot so a routine that
/// already fired for this exact minute doesn't fire again on a later tick).
fn is_due<Tz: TimeZone>(routine: &Routine, now: &DateTime<Tz>) -> bool {
    match routine.schedule_type.as_str() {
        "daily" => is_daily_due(routine, now) && !already_ran_slot(routine, now),
        "weekly" => is_weekly_due(routine, now) && !already_ran_slot(routine, now),
        _ => false,
    }
}

fn is_daily_due<Tz: TimeZone>(routine: &Routine, now: &DateTime<Tz>) -> bool {
    routine.hour == Some(now.hour() as i32) && routine.minute == Some(now.minute() as i32)
}

fn is_weekly_due<Tz: TimeZone>(routine: &Routine, now: &DateTime<Tz>) -> bool {
    let expected = routine.day_of_week.unwrap_or(0);
    let actual = now.weekday().num_days_from_monday() as i32;
    is_daily_due(routine, now) && expected == actual
}

fn already_ran_slot<Tz: TimeZone>(routine: &Routine, now: &DateTime<Tz>) -> bool {
    let Some(ref last) = routine.last_run_at else {
        return false;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(last) else {
        return false;
    };
    let local = parsed.with_timezone(&now.timezone());
    if local.date_naive() != now.date_naive() {
        return false;
    }
    if routine.schedule_type == "weekly" {
        let same_week = local.iso_week() == now.iso_week();
        return same_week && local.hour() == now.hour() && local.minute() == now.minute();
    }
    local.hour() == now.hour() && local.minute() == now.minute()
}

fn parse_event_start(raw: &str) -> Option<chrono::DateTime<Local>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Local));
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|ndt| ndt.and_local_timezone(Local).unwrap());
    }
    None
}

async fn upcoming_meetings(within_minutes: i32) -> Result<Vec<google::CalendarEvent>> {
    let now = Local::now();
    let end = now + chrono::Duration::minutes(within_minutes as i64);
    google::list_events(&now.to_rfc3339(), &end.to_rfc3339()).await
}

async fn recently_ended_meetings(within_minutes: i32) -> Result<Vec<google::CalendarEvent>> {
    let now = Local::now();
    let start = now - chrono::Duration::minutes(within_minutes as i64);
    google::list_events(&start.to_rfc3339(), &now.to_rfc3339()).await
}

async fn execute_routine(db: &Db, notifier: &Arc<dyn Notifier>, item: &DueRoutine) -> Result<()> {
    let provider = db
        .get_setting("provider")?
        .unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model")?.unwrap_or_default();
    if model.is_empty() {
        return Ok(());
    }
    let ollama_host = db
        .get_setting("ollama_host")?
        .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let embed_model = db
        .get_setting("embed_model")?
        .unwrap_or_else(|| crate::embeddings::DEFAULT_EMBED_MODEL.into());
    let api_key = secrets::get_api_key(&provider)?;

    let context = gather_context(
        db,
        &item.routine,
        item.context.as_deref(),
        &provider,
        &ollama_host,
        &embed_model,
    )
    .await?;
    let instruction = item
        .routine
        .prompt
        .clone()
        .unwrap_or_else(|| "Complete this routine for the user.".into());

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: "You are Donna, a proactive personal assistant. Write a helpful, \
                      concise document for the user based on the routine and context. \
                      Use Markdown headings and bullet points."
                .into(),
        },
        ChatTurn {
            role: "user".into(),
            content: format!(
                "## Routine\n{}\n\n## Instruction\n{instruction}\n\n## Context\n{context}",
                item.routine.name
            ),
        },
    ];

    let content = providers::complete(&provider, &model, api_key, &ollama_host, &turns).await?;
    if content.trim().is_empty() {
        return Ok(());
    }

    let doc_title = format!("{} — {}", item.routine.name, Local::now().format("%Y-%m-%d"));
    let source = item
        .routine
        .builtin_id
        .clone()
        .unwrap_or_else(|| "routine".into());
    let doc_id = docs::create(db, &doc_title, &source, &content)?;

    let notif_title = item.routine.name.clone();
    let preview: String = content.chars().take(160).collect();
    db.insert_notification(
        &notif_title,
        &preview,
        Some("open_doc"),
        Some(doc_id),
    )?;

    notifier.notify(&notif_title, &preview);

    // Deliver via WhatsApp or Telegram if connected
    let short_body: String = content.chars().take(1500).collect();
    if crate::integrations::whatsapp::is_connected().unwrap_or(false) {
        if let Ok(Some(my_number)) = db.get_setting("whatsapp_my_number") {
            if !my_number.is_empty() {
                let _ = crate::integrations::whatsapp::send_message(&my_number, &short_body).await;
            }
        }
    } else if crate::integrations::telegram::is_connected().unwrap_or(false) {
        let _ = crate::integrations::telegram::send_message(&short_body).await;
    }

    db.mark_routine_run(item.routine.id)?;
    if let Some(ref key) = item.dedupe_key {
        db.record_routine_dedupe(item.routine.id, key)?;
    }

    Ok(())
}

async fn gather_context(
    db: &Db,
    routine: &Routine,
    extra: Option<&str>,
    provider: &str,
    ollama_host: &str,
    embed_model: &str,
) -> Result<String> {
    let mut parts = Vec::new();

    if let Ok(summary) = knowledge::summary_for_prompt() {
        if !summary.trim().is_empty() {
            parts.push(format!("### Knowledge base\n{summary}"));
        }
    }

    if let Some(ctx) = extra {
        parts.push(format!("### Meeting\n{ctx}"));
    }

    let query = routine.name.clone();
    let cfg = retrieval::RetrievalConfig {
        provider,
        ollama_host,
        embed_model,
    };
    if let Ok(retrieval) = retrieval::search_for_prompt(&query, db, &cfg).await {
        if !retrieval.is_empty() {
            parts.push(format!("### Retrieved memories\n{retrieval}"));
        }
    }

    if google::is_connected().unwrap_or(false) {
        if let Ok(events) = google::list_events(
            &Local::now().to_rfc3339(),
            &(Local::now() + chrono::Duration::hours(24)).to_rfc3339(),
        )
        .await
        {
            if !events.is_empty() {
                let lines: Vec<String> = events
                    .iter()
                    .take(10)
                    .map(|e| {
                        format!(
                            "- {} ({})",
                            e.summary.as_deref().unwrap_or("(no title)"),
                            e.start
                        )
                    })
                    .collect();
                parts.push(format!("### Calendar (next 24h)\n{}", lines.join("\n")));
            }
        }

        if routine.builtin_id.as_deref() == Some("morning_briefing") {
            if let Ok(msgs) = google::list_gmail_messages(5).await {
                if !msgs.is_empty() {
                    let lines: Vec<String> = msgs
                        .iter()
                        .map(|m| {
                            format!(
                                "- {} — {}",
                                m.subject.as_deref().unwrap_or("(no subject)"),
                                m.from.as_deref().unwrap_or("unknown")
                            )
                        })
                        .collect();
                    parts.push(format!("### Recent Gmail\n{}", lines.join("\n")));
                }
            }
        }
    }

    if fathom::is_connected().unwrap_or(false) {
        if let Ok(meetings) = fathom::list_recent_meetings(5).await {
            if !meetings.is_empty() {
                let lines: Vec<String> = meetings
                    .iter()
                    .map(|m| format!("- {}", m.title.as_deref().unwrap_or("(untitled meeting)")))
                    .collect();
                parts.push(format!("### Recent Fathom meetings\n{}", lines.join("\n")));
            }
        }
    }

    if routine.builtin_id.as_deref() == Some("tech_news") {
        if let Ok(stories) = crate::integrations::news::top_stories(10).await {
            let digest = crate::integrations::news::format_digest(&stories);
            parts.push(format!("### Hacker News Top Stories\n{digest}"));
        }
    }

    if routine.builtin_id.as_deref() == Some("morning_briefing") {
        if let (Ok(Some(lat_str)), Ok(Some(lon_str))) = (db.get_setting("location_lat"), db.get_setting("location_lon")) {
            if let (Ok(lat), Ok(lon)) = (lat_str.parse::<f64>(), lon_str.parse::<f64>()) {
                if let Ok(weather) = crate::integrations::weather::fetch(lat, lon).await {
                    parts.push(format!("### Weather\n{}", crate::integrations::weather::format_summary(&weather)));
                }
            }
        }
    }

    if parts.is_empty() {
        Ok("(no additional context available)".into())
    } else {
        Ok(parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daily_routine(hour: i32, minute: i32, last_run_at: Option<String>) -> Routine {
        Routine {
            id: 1,
            name: "Morning briefing".into(),
            kind: "builtin".into(),
            builtin_id: Some("morning_briefing".into()),
            schedule_type: "daily".into(),
            hour: Some(hour),
            minute: Some(minute),
            day_of_week: None,
            minutes_before: None,
            prompt: None,
            enabled: true,
            last_run_at,
        }
    }

    #[test]
    fn daily_routine_due_at_exact_time() {
        let routine = daily_routine(8, 0, None);

        // 08:00:30 → within the due minute, no prior run → due.
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 8, 0, 30).unwrap();
        assert!(is_due(&routine, &now));
    }

    #[test]
    fn daily_routine_not_due_before_scheduled_time() {
        let routine = daily_routine(8, 0, None);

        // 07:59 → not yet due.
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 7, 59, 0).unwrap();
        assert!(!is_due(&routine, &now));
    }

    #[test]
    fn daily_routine_not_due_again_after_run_in_same_slot() {
        // Already ran at 08:00 today (same slot, dedupe via last_run_at).
        let routine = daily_routine(8, 0, Some("2026-07-07T08:00:15+00:00".into()));

        // Later tick still inside the 08:00 minute → already handled, not due again.
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 8, 0, 45).unwrap();
        assert!(!is_due(&routine, &now));
    }
}

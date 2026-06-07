//! Background scheduler — evaluates routines and emits proactive notifications.

use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, Timelike};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::db::{Db, Routine};
use crate::docs;
use crate::error::Result;
use crate::integrations::{fathom, google};
use crate::knowledge;
use crate::providers::{self, ChatTurn};
use crate::retrieval;
use crate::secrets;

/// Start the 60-second scheduler loop and seed built-in routines once.
pub fn run_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Some(db) = app.try_state::<Db>() {
            let _ = db.seed_builtin_routines();
        }

        loop {
            if let Err(e) = tick(&app).await {
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

async fn tick(app: &AppHandle) -> Result<()> {
    let db = app.state::<Db>();
    let routines = db.list_routines()?;
    let due = collect_due_routines(&db, &routines).await?;

    for item in due {
        if let Err(e) = execute_routine(app, &item).await {
            eprintln!("routine {} failed: {e}", item.routine.name);
        }
    }
    Ok(())
}

async fn collect_due_routines(db: &Db, routines: &[Routine]) -> Result<Vec<DueRoutine>> {
    let now = Local::now();
    let mut due = Vec::new();

    for routine in routines {
        if !routine.enabled {
            continue;
        }
        match routine.schedule_type.as_str() {
            "daily" => {
                if is_daily_due(routine, &now) && !already_ran_slot(routine, &now) {
                    due.push(DueRoutine {
                        routine: routine.clone(),
                        context: None,
                        dedupe_key: None,
                    });
                }
            }
            "weekly" => {
                if is_weekly_due(routine, &now) && !already_ran_slot(routine, &now) {
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
                        let mins_until = (start - now).num_minutes();
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
            _ => {}
        }
    }
    Ok(due)
}

fn is_daily_due(routine: &Routine, now: &chrono::DateTime<Local>) -> bool {
    routine.hour == Some(now.hour() as i32) && routine.minute == Some(now.minute() as i32)
}

fn is_weekly_due(routine: &Routine, now: &chrono::DateTime<Local>) -> bool {
    let expected = routine.day_of_week.unwrap_or(0);
    let actual = now.weekday().num_days_from_monday() as i32;
    is_daily_due(routine, now) && expected == actual
}

fn already_ran_slot(
    routine: &Routine,
    now: &chrono::DateTime<Local>,
) -> bool {
    let Some(ref last) = routine.last_run_at else {
        return false;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(last) else {
        return false;
    };
    let local = parsed.with_timezone(&Local);
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

async fn execute_routine(app: &AppHandle, item: &DueRoutine) -> Result<()> {
    let db = app.state::<Db>();
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
    let api_key = secrets::get_api_key(&provider)?;

    let context = gather_context(&item.routine, item.context.as_deref()).await?;
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
    let doc_id = docs::create(&db, &doc_title, &source, &content)?;

    let notif_title = item.routine.name.clone();
    let preview: String = content.chars().take(160).collect();
    db.insert_notification(
        &notif_title,
        &preview,
        Some("open_doc"),
        Some(doc_id),
    )?;

    let _ = app
        .notification()
        .builder()
        .title(&notif_title)
        .body(&preview)
        .show();

    db.mark_routine_run(item.routine.id)?;
    if let Some(ref key) = item.dedupe_key {
        db.record_routine_dedupe(item.routine.id, key)?;
    }

    Ok(())
}

async fn gather_context(routine: &Routine, extra: Option<&str>) -> Result<String> {
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
    if let Ok(retrieval) = retrieval::search_for_prompt(&query) {
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

    if parts.is_empty() {
        Ok("(no additional context available)".into())
    } else {
        Ok(parts.join("\n\n"))
    }
}

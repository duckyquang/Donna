//! Nightly background review — memory curator + pattern → suggestion engine.
//!
//! Once a day the scheduler calls [`run_background_review`]. It hands the model a window of
//! recent activity (messages + events + the current memory files) and asks for STRICT JSON:
//! durable facts to fold into USER.md/MEMORY.md, and at most a few concrete suggestions
//! (e.g. "create a standup-prep routine") when a genuinely recurring need is evident.
//!
//! Everything here is best-effort: a review must never break the scheduler tick, so the
//! model call is the only fallible step and the rest degrades to a zero outcome.

use serde_json::Value;

use crate::db::Db;
use crate::error::Result;
use crate::knowledge::{self, MemoryAction, MemoryFile};
use crate::ops;
use crate::providers::{self, ChatTurn};
use crate::secrets;

/// How many recent messages / events to feed the reviewer.
const RECENT_MESSAGES: i64 = 40;
const RECENT_EVENTS: i64 = 200;

/// Outcome of one review pass, for logging/telemetry.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReviewOutcome {
    pub memory_updated: bool,
    pub suggestions_filed: usize,
}

/// A single memory edit the model wants to make.
#[derive(Debug, PartialEq, Eq)]
pub struct MemoryOp {
    pub file: MemoryFile,
    pub action: MemoryAction,
    pub text: String,
}

/// A suggestion the model wants to file (currently only routines).
#[derive(Debug)]
pub struct SuggestionSpec {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub dedup_key: String,
    pub payload: Value,
}

/// The parsed, validated plan extracted from a model response.
#[derive(Debug, Default)]
pub struct ReviewPlan {
    pub memory: Vec<MemoryOp>,
    pub suggestions: Vec<SuggestionSpec>,
}

const REVIEW_PROMPT: &str = "\
You are Donna's nightly reviewer. You run once a day, in the background, with no user \
watching. Your job is to (1) curate long-term memory and (2) propose automations ONLY when a \
genuinely recurring need is evident. Be conservative: doing nothing is the normal, correct \
outcome. Never invent facts.

You are given the current memory files, the most recent chat messages, and a log of recent \
tool/automation events. Reply with STRICT JSON and nothing else — no prose, no markdown, no \
code fences. Shape:

{
  \"memory\": [
    {\"file\": \"user\"|\"memory\", \"action\": \"add\"|\"replace\"|\"remove\", \"text\": \"...\"}
  ],
  \"suggestions\": [
    {\"kind\": \"routine\", \"title\": \"...\", \"body\": \"why this helps (1-2 sentences)\",
     \"dedup_key\": \"routine:<stable-slug>\",
     \"payload\": {\"name\": \"...\", \"schedule_type\": \"daily\"|\"weekly\",
                   \"hour\": 8, \"minute\": 0, \"day_of_week\": 0, \"prompt\": \"...\"}}
  ]
}

Rules:
- MEMORY: only durable, reusable facts. USER.md = stable identity/preferences \
(timezone, name, how they like replies). MEMORY.md = active threads/conventions that will \
still matter next week. Do NOT store one-off chatter, secrets, or anything already recorded. \
Prefer `replace` to consolidate over piling on `add`s. Use `remove` for stale lines.
- SUGGESTIONS: propose a routine ONLY when the messages/events show the SAME need recurring \
(e.g. the user repeatedly asks for a standup summary each morning). One suggestion per \
distinct need, max 2 total. Reuse a STABLE dedup_key like \"routine:standup\" so a dismissed \
suggestion never comes back. schedule_type is \"daily\" or \"weekly\"; for weekly set \
day_of_week (0=Monday). hour/minute are 24h local time. Omit a field only if truly N/A.
- When nothing is worth doing, return {\"memory\": [], \"suggestions\": []}. That is expected.";

/// Run one nightly review pass. Never errors on \"nothing configured\" — returns a zero
/// outcome so the scheduler tick is unaffected. The single fallible step is the model call.
pub async fn run_background_review(db: &Db) -> Result<ReviewOutcome> {
    let config = ops::load_config(db)?;
    let model = ops::review_model(db);
    if model.trim().is_empty() {
        return Ok(ReviewOutcome::default());
    }
    let api_key = secrets::get_api_key(&config.provider)?;

    let user_content = gather_context(db)?;
    let turns = vec![
        ChatTurn { role: "system".into(), content: REVIEW_PROMPT.into() },
        ChatTurn { role: "user".into(), content: user_content },
    ];

    let raw = providers::complete(
        &config.provider,
        &model,
        api_key,
        &config.ollama_host,
        &turns,
    )
    .await?;

    let plan = parse_review_json(&raw);
    Ok(apply_plan(db, plan))
}

/// Assemble the review window: current memory files + recent messages + recent events.
fn gather_context(db: &Db) -> Result<String> {
    let user_md = knowledge::read_memory_file(MemoryFile::User)?;
    let memory_md = knowledge::read_memory_file(MemoryFile::Memory)?;

    let messages = db.recent_messages(RECENT_MESSAGES)?;
    // recent_messages is newest-first; present oldest-first so the transcript reads naturally.
    let transcript = messages
        .iter()
        .rev()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let events = db.recent_events(RECENT_EVENTS)?;
    let event_lines = events
        .iter()
        .map(|e| {
            format!(
                "- {} {} {}",
                e.created_at,
                e.kind,
                e.tool.as_deref().unwrap_or("")
            )
            .trim_end()
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "## Current USER.md\n{}\n\n## Current MEMORY.md\n{}\n\n## Recent messages (oldest first)\n{}\n\n## Recent events (newest first)\n{}",
        blank_placeholder(&user_md),
        blank_placeholder(&memory_md),
        blank_placeholder(&transcript),
        blank_placeholder(&event_lines),
    ))
}

fn blank_placeholder(s: &str) -> &str {
    if s.trim().is_empty() {
        "(empty)"
    } else {
        s
    }
}

/// Apply a parsed plan: memory ops (swallowing MEMORY_FULL per-op) and suggestions (honoring
/// the dedup latch; notify only on a newly-filed one). Infallible — every step degrades.
fn apply_plan(db: &Db, plan: ReviewPlan) -> ReviewOutcome {
    let mut outcome = ReviewOutcome::default();

    for op in plan.memory {
        match knowledge::apply_memory_update(op.file, op.action, &op.text) {
            Ok(_) => outcome.memory_updated = true,
            // MEMORY_FULL is expected when a file is at cap; the model consolidates next
            // round. Any other error is swallowed too — a review must never break the tick.
            Err(_) => {}
        }
    }

    for s in plan.suggestions {
        let payload = if s.payload.is_null() {
            None
        } else {
            serde_json::to_string(&s.payload).ok()
        };
        match db.insert_suggestion(&s.kind, &s.title, &s.body, payload.as_deref(), &s.dedup_key) {
            Ok(Some(_)) => {
                outcome.suggestions_filed += 1;
                let _ = db.insert_notification(
                    &format!("💡 Suggestion: {}", s.title),
                    &s.body,
                    None,
                    None,
                );
            }
            // None = dedup latch skipped it (already seen/dismissed); Err = swallowed.
            _ => {}
        }
    }

    outcome
}

/// Pull the first `{...}` object out of a possibly noisy model response and read it into a
/// [`ReviewPlan`]. Tolerant: prose, garbage, or partial JSON yields an empty plan; never panics.
pub fn parse_review_json(s: &str) -> ReviewPlan {
    let Some(value) = extract_json(s) else {
        return ReviewPlan::default();
    };

    let memory = value
        .get("memory")
        .and_then(|m| m.as_array())
        .map(|arr| arr.iter().filter_map(parse_memory_op).collect())
        .unwrap_or_default();

    let suggestions = value
        .get("suggestions")
        .and_then(|s| s.as_array())
        .map(|arr| arr.iter().filter_map(parse_suggestion).collect())
        .unwrap_or_default();

    ReviewPlan { memory, suggestions }
}

/// First balanced-ish `{...}` slice, parsed as JSON. Mirrors ops::extract_json.
fn extract_json(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

fn parse_memory_op(v: &Value) -> Option<MemoryOp> {
    let file = match v.get("file")?.as_str()?.trim().to_lowercase().as_str() {
        "user" => MemoryFile::User,
        "memory" => MemoryFile::Memory,
        _ => return None,
    };
    let action = match v.get("action")?.as_str()?.trim().to_lowercase().as_str() {
        "add" => MemoryAction::Add,
        "replace" => MemoryAction::Replace,
        "remove" => MemoryAction::Remove,
        _ => return None,
    };
    let text = v.get("text")?.as_str()?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(MemoryOp { file, action, text })
}

fn parse_suggestion(v: &Value) -> Option<SuggestionSpec> {
    let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("routine").trim().to_string();
    let title = v.get("title")?.as_str()?.trim().to_string();
    let dedup_key = v.get("dedup_key")?.as_str()?.trim().to_string();
    if title.is_empty() || dedup_key.is_empty() {
        return None;
    }
    let body = v.get("body").and_then(|b| b.as_str()).unwrap_or("").trim().to_string();
    let payload = v.get("payload").cloned().unwrap_or(Value::Null);
    Some(SuggestionSpec { kind, title, body, dedup_key, payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_review_json_extracts_plan() {
        let out = r#"Sure! {"memory":[{"file":"user","action":"add","text":"Timezone: Asia/Bangkok"}],
          "suggestions":[{"kind":"routine","title":"Morning digest","body":"...","dedup_key":"routine:digest","payload":{"name":"Morning digest","schedule_type":"daily","hour":8,"minute":0,"prompt":"..."}}]}"#;
        let plan = parse_review_json(out);
        assert_eq!(plan.memory.len(), 1);
        assert_eq!(plan.suggestions.len(), 1);
        assert_eq!(plan.suggestions[0].dedup_key, "routine:digest");
    }

    #[test]
    fn empty_plan_is_noop() {
        let plan = parse_review_json(r#"{"memory":[],"suggestions":[]}"#);
        assert!(plan.memory.is_empty() && plan.suggestions.is_empty());
        // garbage → empty plan, never panics
        let plan2 = parse_review_json("no json here");
        assert!(plan2.memory.is_empty() && plan2.suggestions.is_empty());
    }

    #[test]
    fn parse_review_json_tolerates_partial_and_bad_variants() {
        // Truncated JSON → extract_json can't parse → empty, no panic.
        let plan = parse_review_json(r#"{"memory":[{"file":"user","action":"add","text":"x"#);
        assert!(plan.memory.is_empty() && plan.suggestions.is_empty());

        // Unknown file/action and blank text are dropped; a valid op survives.
        let out = r#"{"memory":[
            {"file":"bogus","action":"add","text":"nope"},
            {"file":"memory","action":"frobnicate","text":"nope"},
            {"file":"memory","action":"add","text":"  "},
            {"file":"memory","action":"replace","text":"Standup at 9am"}
        ],"suggestions":[
            {"kind":"routine","title":"","dedup_key":"routine:x"},
            {"kind":"routine","title":"Real","dedup_key":""},
            {"title":"Standup prep","dedup_key":"routine:standup"}
        ]}"#;
        let plan = parse_review_json(out);
        assert_eq!(plan.memory.len(), 1);
        assert_eq!(plan.memory[0].file, MemoryFile::Memory);
        assert_eq!(plan.memory[0].action, MemoryAction::Replace);
        // Only the fully-specified suggestion survives; kind defaults to "routine".
        assert_eq!(plan.suggestions.len(), 1);
        assert_eq!(plan.suggestions[0].kind, "routine");
        assert_eq!(plan.suggestions[0].dedup_key, "routine:standup");
    }
}

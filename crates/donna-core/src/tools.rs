//! Agent tool registry: the tools the agent loop exposes to the model (see `TOOL_COUNT`).
//!
//! Each tool has a [`Risk`] class that the loop uses to gate execution:
//! - `Read`   — no side effects; run freely.
//! - `Write`  — mutates Donna's own data or the user's own accounts (calendar, drafts,
//!   docs, reminders). Gated by the user's autonomy setting.
//! - `Outbound` — sends a message to another person (Slack/Telegram/WhatsApp). Always
//!   surfaced on an approval card, hence [`summarize_call`] renders every Outbound tool.
//!
//! [`execute`] is one big match, rpc.rs-style: each arm deserializes the model's JSON
//! args into a tiny struct, calls the underlying `ops`/`integrations` fn, then serializes
//! the result to JSON and truncates it via [`truncate_result`]. Arg-shape errors become
//! `Error::Provider("bad arguments for {name}: {e}")` so the loop can feed them back to
//! the model.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::Db;
use crate::error::{Error, Result};
use crate::integrations::{google, news, weather};
use crate::ops;
use crate::retrieval;
use crate::skills;

const RESULT_MAX: usize = 6_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Risk {
    Read,
    Write,
    Outbound,
}

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    /// OpenAI JSON Schema object: `{"type":"object","properties":{…},"required":[…]}`.
    pub params: Value,
    pub risk: Risk,
}

/// The full registry. Order is stable but not semantically meaningful.
pub fn all() -> Vec<ToolDef> {
    vec![
        // ---- Read: external integrations -----------------------------------
        ToolDef {
            name: "calendar_list_events",
            description: "List the user's Google Calendar events in a time window. Returns \
                each event's id, summary, description, start, and end. Use to check the \
                user's schedule before answering scheduling questions or creating events.",
            params: json!({
                "type": "object",
                "properties": {
                    "time_min": {"type": "string", "description": "Window start, RFC3339 (e.g. 2026-01-01T00:00:00Z)."},
                    "time_max": {"type": "string", "description": "Window end, RFC3339."}
                },
                "required": ["time_min", "time_max"]
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "gmail_list_messages",
            description: "List recent Gmail messages (subject, sender, snippet). Use to check \
                the user's inbox or find a recent email.",
            params: json!({
                "type": "object",
                "properties": {
                    "max_results": {"type": "integer", "description": "How many messages to return (max 25, default 10).", "maximum": 25}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "drive_list_files",
            description: "List recent Google Drive files (name, id, type). Use to find a \
                document the user refers to.",
            params: json!({
                "type": "object",
                "properties": {
                    "max_results": {"type": "integer", "description": "How many files to return (max 25).", "maximum": 25}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "slack_list_channels",
            description: "List the Slack channels the user's workspace token can see (name, id). \
                Use to resolve a channel name to post to, or to see what channels exist.",
            params: json!({"type": "object", "properties": {}, "required": []}),
            risk: Risk::Read,
        },
        ToolDef {
            name: "github_list_repos",
            description: "List the user's GitHub repositories (name, description, url). Use to \
                find a repo the user mentions.",
            params: json!({
                "type": "object",
                "properties": {
                    "max_results": {"type": "integer", "description": "How many repos to return.", "maximum": 25}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "github_list_issues",
            description: "List recent GitHub issues assigned to or opened by the user (title, \
                repo, url, state). Use to review the user's open work.",
            params: json!({
                "type": "object",
                "properties": {
                    "max_results": {"type": "integer", "description": "How many issues to return.", "maximum": 25}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "linear_list_issues",
            description: "List the user's recent Linear issues (title, state, url). Use to check \
                the user's Linear workload.",
            params: json!({
                "type": "object",
                "properties": {
                    "max_results": {"type": "integer", "description": "How many issues to return.", "maximum": 25}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "notion_list_pages",
            description: "List the user's most recently edited Notion pages (title, url). Note: \
                this returns recent pages only — it does NOT filter by a search query.",
            params: json!({
                "type": "object",
                "properties": {
                    "max_results": {"type": "integer", "description": "How many pages to return.", "maximum": 25}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "fathom_recent_meetings",
            description: "List the user's recent Fathom meeting recordings with summaries and \
                action items. Use to recall what was discussed in recent calls.",
            params: json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "How many meetings to return (max 10).", "maximum": 10}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "news_top_stories",
            description: "Fetch today's top Hacker News tech stories as a formatted digest \
                (title, score, link). Use when the user asks what's happening in tech.",
            params: json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "How many stories to include (max 15).", "maximum": 15}
                },
                "required": []
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "weather_current",
            description: "Get current weather for a latitude/longitude as a short formatted \
                string (temp, feels-like, conditions, wind). No API key needed.",
            params: json!({
                "type": "object",
                "properties": {
                    "lat": {"type": "number", "description": "Latitude in decimal degrees."},
                    "lon": {"type": "number", "description": "Longitude in decimal degrees."}
                },
                "required": ["lat", "lon"]
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "session_search",
            description: "Full-text search across the user's entire message history (all past \
                conversations). Use to recall what the user told you before. Returns matching \
                messages newest-relevance first.",
            params: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "What to search for."},
                    "limit": {"type": "integer", "description": "How many messages to return (max 25, default 10).", "minimum": 1, "maximum": 25}
                },
                "required": ["query"]
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "kb_search",
            description: "Search Donna's knowledge base (the user's saved memories / mind map) \
                for facts relevant to a query. Returns the top matching memories. Use before \
                answering anything personal about the user.",
            params: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "What to look for, in natural language."}
                },
                "required": ["query"]
            }),
            risk: Risk::Read,
        },
        // ---- Read: Donna's own data ----------------------------------------
        ToolDef {
            name: "list_docs",
            description: "List all local documents Donna has stored (id, title, source). Use to \
                see what docs exist before reading one.",
            params: json!({"type": "object", "properties": {}, "required": []}),
            risk: Risk::Read,
        },
        ToolDef {
            name: "get_doc",
            description: "Read one local document's full content by id. Use after list_docs to \
                pull up a specific doc.",
            params: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "The document id from list_docs."}
                },
                "required": ["id"]
            }),
            risk: Risk::Read,
        },
        ToolDef {
            name: "list_routines",
            description: "List the user's scheduled routines / proactive automations (name, \
                schedule, prompt, enabled). Use to see what recurring tasks are set up.",
            params: json!({"type": "object", "properties": {}, "required": []}),
            risk: Risk::Read,
        },
        ToolDef {
            name: "reading_list_get",
            description: "List the user's saved reading-list items (url, title, summary, read \
                flag). Use to see what the user has bookmarked to read.",
            params: json!({"type": "object", "properties": {}, "required": []}),
            risk: Risk::Read,
        },
        ToolDef {
            name: "habit_list",
            description: "List the user's tracked habits (id, name, description). Use to see \
                which habits exist before logging one with habit_log.",
            params: json!({"type": "object", "properties": {}, "required": []}),
            risk: Risk::Read,
        },
        ToolDef {
            name: "skills_list",
            description: "List all of Donna's available skills (name + description only). Call \
                this to see what skills exist, then skill_view to load one's full instructions \
                before acting.",
            params: json!({"type": "object", "properties": {}, "required": []}),
            risk: Risk::Read,
        },
        ToolDef {
            name: "skill_view",
            description: "Load a skill's full SKILL.md instructions by name (or a reference file \
                via `path`). Read the skill BEFORE acting on it; follow its steps.",
            params: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Skill name."},
                    "path": {"type": "string", "description": "A reference file inside the skill."}
                },
                "required": ["name"]
            }),
            risk: Risk::Read,
        },
        // ---- Write ---------------------------------------------------------
        ToolDef {
            name: "kb_save_node",
            description: "Save a durable fact about the user to Donna's knowledge base / mind \
                map. Organize it under a folder path (2–5 segments, deepest holds the node). \
                Use to remember something the user tells you about themselves.",
            params: json!({
                "type": "object",
                "properties": {
                    "folder": {"type": "array", "items": {"type": "string"}, "description": "Folder path, 2–5 segments; the deepest folder holds the node."},
                    "label": {"type": "string", "description": "Short title for the memory."},
                    "note": {"type": "string", "description": "The fact itself, in a sentence or two."},
                    "node_type": {"type": "string", "description": "Kind of node, e.g. 'fact', 'preference', 'person', 'project'."}
                },
                "required": ["folder", "label", "note", "node_type"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "memory_update",
            description: "Update your durable memory about the user. USER.md = stable \
                identity/preferences (cap 1500 chars); MEMORY.md = active threads/conventions \
                (cap 2500 chars). 'add' appends a line (errors if full — then consolidate), \
                'replace' rewrites the whole file, 'remove' deletes lines containing the text. \
                Keep entries terse.",
            params: json!({
                "type": "object",
                "properties": {
                    "file": {"type": "string", "enum": ["user", "memory"], "description": "Which file: 'user' (USER.md) or 'memory' (MEMORY.md)."},
                    "action": {"type": "string", "enum": ["add", "replace", "remove"], "description": "'add' appends a line, 'replace' rewrites the whole file, 'remove' deletes lines containing text."},
                    "text": {"type": "string", "description": "For 'add'/'remove': a line or substring. For 'replace': the full new file body."}
                },
                "required": ["file", "action", "text"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "gmail_create_draft",
            description: "Create a Gmail draft (does NOT send it). Returns the draft id. Use to \
                prepare an email for the user to review and send.",
            params: json!({
                "type": "object",
                "properties": {
                    "to": {"type": "string", "description": "Recipient email address."},
                    "subject": {"type": "string", "description": "Email subject line."},
                    "body": {"type": "string", "description": "Email body (plain text)."}
                },
                "required": ["to", "subject", "body"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "google_create_doc",
            description: "Create a new empty Google Doc with the given title. Returns the doc \
                url. Use when the user wants a doc created in their Google Drive.",
            params: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Title for the new Google Doc."}
                },
                "required": ["title"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "create_doc",
            description: "Create a local Donna document with a title and full content. Returns \
                the new doc id. Use to save notes, summaries, or drafts inside Donna.",
            params: json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Document title."},
                    "content": {"type": "string", "description": "Full document body."}
                },
                "required": ["title", "content"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "calendar_create_event",
            description: "Create a Google Calendar event. start/end are RFC3339 timestamps. \
                Returns the created event. Use to schedule something on the user's calendar.",
            params: json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string", "description": "Event title."},
                    "description": {"type": "string", "description": "Optional event details."},
                    "start": {"type": "string", "description": "Start time, RFC3339."},
                    "end": {"type": "string", "description": "End time, RFC3339."}
                },
                "required": ["summary", "start", "end"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "calendar_update_event",
            description: "Update an existing Google Calendar event by id. start/end are RFC3339 \
                and required. summary/description are optional overrides. Returns the updated \
                event. Use after calendar_list_events to find the id.",
            params: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Event id from calendar_list_events."},
                    "summary": {"type": "string", "description": "New event title (optional)."},
                    "description": {"type": "string", "description": "New event details (optional)."},
                    "start": {"type": "string", "description": "New start time, RFC3339."},
                    "end": {"type": "string", "description": "New end time, RFC3339."}
                },
                "required": ["id", "start", "end"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "calendar_delete_event",
            description: "Delete a Google Calendar event by id. Use after calendar_list_events \
                to find the id. This is irreversible.",
            params: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Event id from calendar_list_events."}
                },
                "required": ["id"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "reading_list_add",
            description: "Add a url with a title to the user's reading list. Use when the user \
                wants to save something to read later.",
            params: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "Link to save."},
                    "title": {"type": "string", "description": "Title / label for the link."}
                },
                "required": ["url", "title"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "habit_log",
            description: "Log today's completion of a habit by its id (from habit_list), with an \
                optional note. Use when the user says they did a tracked habit.",
            params: json!({
                "type": "object",
                "properties": {
                    "habit_id": {"type": "integer", "description": "Habit id from habit_list."},
                    "note": {"type": "string", "description": "Optional note about this completion."}
                },
                "required": ["habit_id"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "remember",
            description: "Set a one-shot note-to-self reminder that fires at a given time. \
                due_at must be RFC3339 (any timezone offset is fine — it's normalized to UTC). \
                Use when the user wants to be reminded of something later.",
            params: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "What to remind the user about."},
                    "due_at": {"type": "string", "description": "When to fire, RFC3339 (e.g. 2026-01-01T09:00:00+07:00)."}
                },
                "required": ["text", "due_at"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "create_routine",
            description: "Create a scheduled routine that runs a prompt on a daily or weekly \
                cadence. For weekly, set day_of_week (0=Sunday..6=Saturday). Use to set up a \
                recurring proactive task for the user.",
            params: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Routine name."},
                    "schedule_type": {"type": "string", "enum": ["daily", "weekly"], "description": "Cadence: 'daily' or 'weekly'."},
                    "hour": {"type": "integer", "description": "Hour of day, 0–23.", "minimum": 0, "maximum": 23},
                    "minute": {"type": "integer", "description": "Minute of hour, 0–59.", "minimum": 0, "maximum": 59},
                    "day_of_week": {"type": "integer", "description": "For weekly: 0=Sunday..6=Saturday.", "minimum": 0, "maximum": 6},
                    "prompt": {"type": "string", "description": "The instruction Donna runs each time the routine fires."}
                },
                "required": ["name", "schedule_type", "hour", "minute"]
            }),
            risk: Risk::Write,
        },
        ToolDef {
            name: "skill_create",
            description: "Author a new reusable skill as a SKILL.md. Use when you've worked out \
                a repeatable multi-step recipe worth saving. name = short title; description = \
                one line for the catalog; category = a grouping; body = step-by-step instructions \
                in Markdown.",
            params: json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Short title for the skill."},
                    "description": {"type": "string", "description": "One line for the catalog."},
                    "category": {"type": "string", "description": "A grouping for the skill."},
                    "body": {"type": "string", "description": "Step-by-step instructions in Markdown."}
                },
                "required": ["name", "description", "category", "body"]
            }),
            risk: Risk::Write,
        },
        // ---- Outbound ------------------------------------------------------
        ToolDef {
            name: "slack_send_message",
            description: "Send a Slack message to a channel (name like '#general' or a channel \
                id). This posts publicly on the user's behalf.",
            params: json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string", "description": "Channel name (e.g. #general) or channel id."},
                    "text": {"type": "string", "description": "Message text to send."}
                },
                "required": ["channel", "text"]
            }),
            risk: Risk::Outbound,
        },
        ToolDef {
            name: "telegram_send_message",
            description: "Send a Telegram message to the user's configured chat. Use to ping the \
                user or a group via the connected bot.",
            params: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Message text to send."}
                },
                "required": ["text"]
            }),
            risk: Risk::Outbound,
        },
        ToolDef {
            name: "whatsapp_send_message",
            description: "Send a WhatsApp message to a phone number (E.164, e.g. +15551234567). \
                This messages the recipient on the user's behalf.",
            params: json!({
                "type": "object",
                "properties": {
                    "to": {"type": "string", "description": "Recipient phone number in E.164 format, e.g. +15551234567."},
                    "text": {"type": "string", "description": "Message text to send."}
                },
                "required": ["to", "text"]
            }),
            risk: Risk::Outbound,
        },
    ]
}

/// The `[{"type":"function","function":{name, description, parameters}}, …]` array the
/// OpenAI API expects for tool definitions.
pub fn openai_tools_json() -> Value {
    Value::Array(
        all()
            .into_iter()
            .map(|d| {
                json!({
                    "type": "function",
                    "function": {
                        "name": d.name,
                        "description": d.description,
                        "parameters": d.params,
                    }
                })
            })
            .collect(),
    )
}

/// Risk class for a tool name, or `None` if the tool is not in the registry.
pub fn risk_of(name: &str) -> Option<Risk> {
    all().into_iter().find(|d| d.name == name).map(|d| d.risk)
}

/// JSON-serialize a value and truncate to [`RESULT_MAX`] chars, appending a marker.
fn truncate_result(s: String) -> String {
    if s.len() <= RESULT_MAX {
        return s;
    }
    // Cut on a char boundary at/below the byte budget.
    let mut end = RESULT_MAX;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…[truncated]", &s[..end])
}

/// Serialize an ops result to a JSON string, then truncate. The shared tail of every arm.
fn ok<T: serde::Serialize>(v: T) -> Result<String> {
    Ok(truncate_result(serde_json::to_string(&v)?))
}

/// Deserialize the model's args for `name` into `T`, mapping shape errors to a descriptive
/// `Provider` error the agent loop feeds back to the model.
fn parse<T: serde::de::DeserializeOwned>(name: &str, args: &Value) -> Result<T> {
    serde_json::from_value(args.clone())
        .map_err(|e| Error::Provider(format!("bad arguments for {name}: {e}")))
}

/// Dispatch a tool call. Each arm: deserialize args → call underlying fn → serialize +
/// truncate the result. Unknown tools error with "unknown tool".
pub async fn execute(db: &Db, name: &str, args: &Value) -> Result<String> {
    match name {
        // ---- Read: external ---------------------------------------------
        "calendar_list_events" => {
            #[derive(Deserialize)]
            struct A { time_min: String, time_max: String }
            let a: A = parse(name, args)?;
            ok(ops::calendar_list_events(a.time_min, a.time_max).await?)
        }
        "gmail_list_messages" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] max_results: u32 }
            let a: A = parse(name, args)?;
            ok(ops::gmail_list_messages(a.max_results.min(25)).await?)
        }
        "drive_list_files" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] max_results: u32 }
            let a: A = parse(name, args)?;
            ok(ops::drive_list_files(a.max_results.min(25)).await?)
        }
        "slack_list_channels" => ok(ops::slack_list_channels().await?),
        "github_list_repos" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] max_results: u32 }
            let a: A = parse(name, args)?;
            ok(ops::github_list_repos(a.max_results.min(25)).await?)
        }
        "github_list_issues" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] max_results: u32 }
            let a: A = parse(name, args)?;
            ok(ops::github_list_issues(a.max_results.min(25)).await?)
        }
        "linear_list_issues" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] max_results: u32 }
            let a: A = parse(name, args)?;
            ok(ops::linear_list_issues(a.max_results.min(25)).await?)
        }
        "notion_list_pages" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] max_results: u32 }
            let a: A = parse(name, args)?;
            ok(ops::notion_search_pages(a.max_results.min(25)).await?)
        }
        "fathom_recent_meetings" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "ten")] limit: u32 }
            let a: A = parse(name, args)?;
            ok(crate::integrations::fathom::list_recent_meetings(a.limit.min(10)).await?)
        }
        "news_top_stories" => {
            #[derive(Deserialize)]
            struct A { #[serde(default = "fifteen")] limit: usize }
            let a: A = parse(name, args)?;
            let stories = news::top_stories(a.limit.min(15)).await?;
            ok(news::format_digest(&stories))
        }
        "weather_current" => {
            #[derive(Deserialize)]
            struct A { lat: f64, lon: f64 }
            let a: A = parse(name, args)?;
            let w = weather::fetch(a.lat, a.lon).await?;
            ok(weather::format_summary(&w))
        }
        "session_search" => {
            #[derive(Deserialize)]
            struct A { query: String, #[serde(default = "ten_i64")] limit: i64 }
            let a: A = parse(name, args)?;
            #[derive(serde::Serialize)]
            struct Hit { conversation_id: i64, role: String, content: String, created_at: String }
            let hits = db.search_messages(&a.query, a.limit.clamp(1, 25))?
                .into_iter()
                .map(|m| Hit { conversation_id: m.conversation_id, role: m.role, content: m.content, created_at: m.created_at })
                .collect::<Vec<_>>();
            ok(hits)
        }
        "kb_search" => {
            #[derive(Deserialize)]
            struct A { query: String }
            let a: A = parse(name, args)?;
            let config = ops::load_config(db)?;
            let cfg = retrieval::RetrievalConfig {
                provider: &config.provider,
                ollama_host: &config.ollama_host,
                embed_model: &config.embed_model,
            };
            ok(retrieval::search_for_prompt(&a.query, db, &cfg).await?)
        }
        // ---- Read: Donna's own data -------------------------------------
        "list_docs" => ok(ops::list_docs(db)?),
        "get_doc" => {
            #[derive(Deserialize)]
            struct A { id: i64 }
            let a: A = parse(name, args)?;
            ok(ops::get_doc(db, a.id)?)
        }
        "list_routines" => ok(ops::list_routines(db)?),
        "reading_list_get" => ok(ops::reading_list_get(db).await?),
        "habit_list" => ok(ops::habit_list(db).await?),
        "skills_list" => ok(skills::list_skills()?),
        "skill_view" => {
            #[derive(Deserialize)]
            struct A { name: String, #[serde(default)] path: Option<String> }
            let a: A = parse(name, args)?;
            ok(skills::view_skill(&a.name, a.path.as_deref())?)
        }
        // ---- Write ------------------------------------------------------
        "kb_save_node" => {
            #[derive(Deserialize)]
            struct A { folder: Vec<String>, label: String, note: String, node_type: String }
            let a: A = parse(name, args)?;
            ok(ops::kg_save_node(db, a.folder, a.label, a.note, a.node_type, None, None).await?)
        }
        "memory_update" => {
            #[derive(Deserialize)]
            struct A { file: String, action: String, text: String }
            let a: A = parse(name, args)?;
            ok(ops::memory_update(db, a.file, a.action, a.text).await?)
        }
        "gmail_create_draft" => {
            #[derive(Deserialize)]
            struct A { to: String, subject: String, body: String }
            let a: A = parse(name, args)?;
            ok(ops::gmail_create_draft(a.to, a.subject, a.body).await?)
        }
        "google_create_doc" => {
            #[derive(Deserialize)]
            struct A { title: String }
            let a: A = parse(name, args)?;
            ok(ops::google_create_doc(a.title).await?)
        }
        "create_doc" => {
            #[derive(Deserialize)]
            struct A { title: String, content: String }
            let a: A = parse(name, args)?;
            ok(ops::create_doc(db, a.title, a.content)?)
        }
        "calendar_create_event" => {
            #[derive(Deserialize)]
            struct A { summary: String, description: Option<String>, start: String, end: String }
            let a: A = parse(name, args)?;
            let ev = google::CalendarEvent {
                id: None,
                summary: Some(a.summary),
                description: a.description,
                start: a.start,
                end: a.end,
                html_link: None,
            };
            ok(ops::calendar_create_event(ev).await?)
        }
        "calendar_update_event" => {
            #[derive(Deserialize)]
            struct A { id: String, summary: Option<String>, description: Option<String>, start: String, end: String }
            let a: A = parse(name, args)?;
            let ev = google::CalendarEvent {
                id: Some(a.id.clone()),
                summary: a.summary,
                description: a.description,
                start: a.start,
                end: a.end,
                html_link: None,
            };
            ok(ops::calendar_update_event(a.id, ev).await?)
        }
        "calendar_delete_event" => {
            #[derive(Deserialize)]
            struct A { id: String }
            let a: A = parse(name, args)?;
            ops::calendar_delete_event(a.id).await?;
            ok("deleted")
        }
        "reading_list_add" => {
            #[derive(Deserialize)]
            struct A { url: String, title: String }
            let a: A = parse(name, args)?;
            ok(ops::reading_list_add(db, a.url, a.title).await?)
        }
        "habit_log" => {
            #[derive(Deserialize)]
            struct A { habit_id: i64, note: Option<String> }
            let a: A = parse(name, args)?;
            ops::habit_log(db, a.habit_id, a.note).await?;
            ok("logged")
        }
        "remember" => {
            #[derive(Deserialize)]
            struct A { text: String, due_at: String }
            let a: A = parse(name, args)?;
            ok(ops::remember(db, a.text, a.due_at).await?)
        }
        "create_routine" => {
            // ops::create_routine takes CreateRoutineInput, which Deserializes from the
            // same schema (extra `minutes_before` field defaults to None when absent).
            let input: ops::CreateRoutineInput = parse(name, args)?;
            ok(ops::create_routine(db, input)?)
        }
        "skill_create" => {
            #[derive(Deserialize)]
            struct A { name: String, description: String, category: String, body: String }
            let a: A = parse(name, args)?;
            ok(skills::save_skill(&a.name, &a.description, &a.category, &a.body)?)
        }
        // ---- Outbound ---------------------------------------------------
        "slack_send_message" => {
            #[derive(Deserialize)]
            struct A { channel: String, text: String }
            let a: A = parse(name, args)?;
            ops::slack_send_message(a.channel, a.text).await?;
            ok("sent")
        }
        "telegram_send_message" => {
            #[derive(Deserialize)]
            struct A { text: String }
            let a: A = parse(name, args)?;
            ops::telegram_send_message(a.text).await?;
            ok("sent")
        }
        "whatsapp_send_message" => {
            #[derive(Deserialize)]
            struct A { to: String, text: String }
            let a: A = parse(name, args)?;
            ops::whatsapp_send_message(a.to, a.text).await?;
            ok("sent")
        }
        _ => Err(Error::Provider(format!("unknown tool: {name}"))),
    }
}

fn ten() -> u32 { 10 }
fn ten_i64() -> i64 { 10 }
fn fifteen() -> usize { 15 }

/// Human one-liner for approval cards / Tool events. Renders every Outbound tool plus
/// `calendar_delete_event` clearly; other tools fall back to `"{name} {args}"` (≤120 chars).
pub fn summarize_call(name: &str, args: &Value) -> String {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match name {
        "slack_send_message" => format!("Send Slack message to {}: {}", s("channel"), s("text")),
        "telegram_send_message" => format!("Send Telegram message: {}", s("text")),
        "whatsapp_send_message" => format!("Send WhatsApp message to {}: {}", s("to"), s("text")),
        "calendar_delete_event" => format!("Delete calendar event {}", s("id")),
        "calendar_create_event" => format!("Create calendar event '{}'", s("summary")),
        "gmail_create_draft" => format!("Draft email to {}: {}", s("to"), s("subject")),
        "remember" => format!("Set reminder: {} (at {})", s("text"), s("due_at")),
        _ => truncate_120(&format!("{name} {args}")),
    }
}

fn truncate_120(s: &str) -> String {
    if s.chars().count() <= 120 {
        return s.to_string();
    }
    let end = s.char_indices().nth(120).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn test_db() -> Db {
        crate::secrets::init_test_file_store();
        let dir = std::env::temp_dir().join(format!(
            "donna-tools-{}-{}",
            std::process::id(),
            crate::db::unique_test_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Db::open(&dir.join("t.sqlite")).unwrap()
    }

    // NOTE: the brief/plan headline says "28 tools" but its own enumeration lists 31
    // (Read 12 + Donna-reads 5 + Write 11 + Outbound 3). The enumeration is the precise,
    // actionable spec; "28" is an unreconciled round number repeated in the headline.
    // Implementing all 31 named tools rather than arbitrarily dropping 3 the plan lists.
    // Phase 4 Task 1 adds `memory_update` (Write), bringing the total to 32.
    // Phase 4 Task 2 adds `session_search` (Read), bringing the total to 33.
    // Phase 6 Task 2 adds `skills_list`, `skill_view` (Read), `skill_create` (Write),
    // bringing the total to 36.
    const TOOL_COUNT: usize = 36;

    #[tokio::test]
    async fn registry_names_unique_and_schemas_valid() {
        let defs = all();
        assert_eq!(defs.len(), TOOL_COUNT);
        let names: std::collections::HashSet<_> = defs.iter().map(|d| d.name).collect();
        assert_eq!(names.len(), defs.len());
        for d in &defs {
            assert_eq!(d.params["type"], "object", "{} params must be an object schema", d.name);
        }
        let json = openai_tools_json();
        assert_eq!(json.as_array().unwrap().len(), TOOL_COUNT);
    }

    #[tokio::test]
    async fn execute_dispatches_db_tool() {
        let db = test_db();
        crate::docs::create(&db, "T", "test", "body").unwrap();
        let out = execute(&db, "list_docs", &serde_json::json!({})).await.unwrap();
        assert!(out.contains("\"T\""));
        let err = execute(&db, "no_such_tool", &serde_json::json!({})).await.unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn memory_update_tool_registered_and_dispatches() {
        let db = test_db();
        let _kb = crate::knowledge::tests::temp_kb();
        let out = execute(&db, "memory_update", &serde_json::json!({"file":"user","action":"add","text":"Likes tea"})).await.unwrap();
        assert!(out.contains("Likes tea"));
        assert_eq!(all().len(), 36);
    }

    #[test]
    fn truncation_and_summaries() {
        assert!(truncate_result("x".repeat(10_000)).len() < 6_100);
        let s = summarize_call("slack_send_message", &serde_json::json!({"channel":"#general","text":"hi"}));
        assert!(s.contains("#general"));
    }

    #[tokio::test]
    async fn session_search_tool() {
        let db = test_db();
        let c = db.create_conversation("t").unwrap();
        db.add_message(c, "user", "remember the wifi password is hunter2").unwrap();
        let out = execute(&db, "session_search", &serde_json::json!({"query":"wifi password"})).await.unwrap();
        assert!(out.contains("hunter2"));
        assert_eq!(all().len(), 36);
    }

    #[tokio::test]
    async fn skill_tools_registered_and_dispatch() {
        let db = test_db();
        let _g = crate::skills::tests::skills_test_guard();
        assert_eq!(all().len(), 36);
        let created = execute(&db, "skill_create", &serde_json::json!({
            "name":"Trip Planner","description":"Plan a trip","category":"travel","body":"1. Ask dates\n2. ..."})).await.unwrap();
        assert!(created.contains("trip-planner"));
        let listed = execute(&db, "skills_list", &serde_json::json!({})).await.unwrap();
        assert!(listed.contains("Trip Planner"));
        let viewed = execute(&db, "skill_view", &serde_json::json!({"name":"Trip Planner"})).await.unwrap();
        assert!(viewed.contains("Ask dates"));
    }
}

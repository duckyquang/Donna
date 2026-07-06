//! Tauri commands exposed to the frontend over IPC.
//!
//! Covers Phase 1: app config, secure key management, model listing, chat history,
//! and streaming chat completions.

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

use crate::db::{Conversation, Db, Doc, Message, Notification, Routine};
use crate::embeddings;
use crate::error::{Error, Result};
use crate::integrations::{self, discord, github, google, linear, notion, slack, telegram, whatsapp};
use crate::knowledge;
use crate::providers::{self, ChatTurn};
use crate::retrieval;
use crate::secrets;

const DONNA_SYSTEM_PROMPT: &str = "You are Donna, a warm, sharp, and proactive personal \
assistant who is private and runs locally on the user's own device. You learn about the \
user over time, help them think, draft, and stay organized, and you are concise and \
practical.\n\n## Knowledge audit (do this every reply)\nBefore you answer, check \
\"What Donna knows\" and \"Donna setup status\" below. Contrast what you KNOW vs what \
you DO NOT. When gaps exist, ask — never guess, never vaguely agree, never invent facts.\n\n\
## Question priority (strict order)\nAsk about higher tiers BEFORE lower ones. Never skip \
to hobbies or casual interests while basics are missing.\n\
Tier 1 — Core identity (collect these early; do not skip):\n\
1. Preferred name — what to call them (not just \"they prefer a nickname\"; get the actual name)\n\
2. Age or age range\n\
3. Nationality / country they identify with\n\
4. Birthday (full date, or at least month and day)\n\
5. Location or timezone (city/country — needed for scheduling)\n\
6. Work OR study situation (role, field, organization)\n\n\
If the Basics checklist below shows any missing items, your reply MUST include at least \
one donna-ask question about the highest-priority missing basic BEFORE hobbies, casual \
chat, or lower-tier topics. On early conversations, greet warmly and start with their \
name if unknown.\n\n\
Tier 2 — Structure: daily/weekly routines, key people (manager, team, clients), active \
projects or goals.\n\
Tier 3 — Preferences: how they want you to communicate, priorities, feedback on your help.\n\
Tier 4 — Interests: hobbies, casual topics (only after Tier 1 is reasonably complete).\n\n\
## Also proactively ask about\n\
- Tasks & to-dos: anything they need to do, deadlines, follow-ups, blockers?\n\
- Donna setup: integrations not connected, model choice, routines not configured, empty \
knowledge base?\n\
- Open loops: things they mentioned but have not finished.\n\n\
Ask 1–2 focused questions per reply when real gaps exist (do not interrogate). You may \
include multiple donna-ask blocks in one reply; the user answers all of them at once in \
a numbered list. Embed questions using a donna-ask block:\n\nMultiple choice (always include \"Other\" last):\n\
```donna-ask\n{\"type\":\"choice\",\"prompt\":\"Your question?\",\"options\":[\"A\",\"B\",\"Other\"]}\n```\n\n\
Free-text:\n```donna-ask\n{\"type\":\"text\",\"prompt\":\"Your question?\"}\n```\n\n\
You may write normal Markdown before and after question blocks. When the user tells you \
to remember something, acknowledge it clearly.";

/// Assemble the full system prompt: persona + basics audit + live knowledge + setup status.
fn build_system_prompt(
    config: &AppConfig,
    retrieval_ctx: Option<&str>,
) -> Result<String> {
    let basics = knowledge::basics_checklist_for_prompt()?;
    let known = knowledge::summary_for_prompt()?;
    let setup = build_setup_context(config)?;

    let mut prompt = format!(
        "{DONNA_SYSTEM_PROMPT}\n\n## Basics checklist\n{basics}\n\n## What Donna knows about this user\n{known}\n\n{setup}"
    );

    if let Some(ctx) = retrieval_ctx {
        if !ctx.is_empty() {
            prompt.push_str("\n\n## Relevant memories (retrieval)\n");
            prompt.push_str(ctx);
        }
    }

    prompt.push_str("\n\n## Autonomy level\n");
    prompt.push_str(autonomy_note(&config.autonomy_level));

    Ok(prompt)
}

fn autonomy_note(level: &str) -> &'static str {
    match level {
        "act" => "The user set autonomy to **act**: take reasonable low-risk actions without asking first.",
        "autonomous" => "The user set autonomy to **autonomous**: proceed proactively; only confirm high-impact actions.",
        _ => "The user set autonomy to **confirm**: ask before acting on their behalf (calendar, email, messages, etc.).",
    }
}

fn build_setup_context(config: &AppConfig) -> Result<String> {
    let model = if config.model.is_empty() {
        "not selected"
    } else {
        &config.model
    };
    let mut lines = vec![format!(
        "- AI: {} / {}",
        config.provider, model
    )];
    for s in integrations::status()? {
        let state = if s.connected {
            "connected"
        } else if s.needs_setup {
            "needs setup"
        } else {
            "not connected"
        };
        lines.push(format!("- {}: {state}", s.name));
    }
    Ok(format!(
        "## Donna & app setup status\n{}\n\nIf integrations are not connected or the \
         knowledge base is empty, include a setup question when relevant.",
        lines.join("\n")
    ))
}

// --- Config -----------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub provider: String,
    pub model: String,
    pub ollama_host: String,
    pub onboarded: bool,
    /// User finished the first-conversation profile basics wizard.
    pub profile_onboarded: bool,
    /// confirm | act | autonomous
    #[serde(default = "default_autonomy_level")]
    pub autonomy_level: String,
    /// Ollama embedding model for semantic memory retrieval.
    #[serde(default = "default_embed_model")]
    pub embed_model: String,
}

fn default_embed_model() -> String {
    embeddings::DEFAULT_EMBED_MODEL.into()
}

fn default_autonomy_level() -> String {
    "confirm".into()
}

fn load_config(db: &Db) -> Result<AppConfig> {
    Ok(AppConfig {
        provider: db.get_setting("provider")?.unwrap_or_else(|| "ollama".into()),
        model: db.get_setting("model")?.unwrap_or_default(),
        ollama_host: db
            .get_setting("ollama_host")?
            .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into()),
        onboarded: db.get_setting("onboarded")?.as_deref() == Some("true"),
        profile_onboarded: db.get_setting("profile_onboarded")?.as_deref() == Some("true"),
        autonomy_level: db
            .get_setting("autonomy_level")?
            .unwrap_or_else(|| "confirm".into()),
        embed_model: db
            .get_setting("embed_model")?
            .unwrap_or_else(|| embeddings::DEFAULT_EMBED_MODEL.into()),
    })
}

#[tauri::command]
pub fn get_config(db: State<Db>) -> Result<AppConfig> {
    load_config(&db)
}

#[tauri::command]
pub fn basics_status() -> Result<Vec<knowledge::BasicFieldStatus>> {
    knowledge::basics_status()
}

fn spawn_ollama_warmup(host: String, model: String) {
    if model.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = providers::warm_ollama_model(&host, &model).await;
    });
}

#[tauri::command]
pub fn save_config(db: State<Db>, config: AppConfig) -> Result<()> {
    db.set_setting("provider", &config.provider)?;
    db.set_setting("model", &config.model)?;
    db.set_setting("ollama_host", &config.ollama_host)?;
    db.set_setting("onboarded", if config.onboarded { "true" } else { "false" })?;
    db.set_setting(
        "profile_onboarded",
        if config.profile_onboarded {
            "true"
        } else {
            "false"
        },
    )?;
    db.set_setting("autonomy_level", &config.autonomy_level)?;
    db.set_setting("embed_model", &config.embed_model)?;
    if config.provider == "ollama" {
        spawn_ollama_warmup(config.ollama_host, config.model);
    }
    Ok(())
}

// --- Secrets ----------------------------------------------------------------

#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<()> {
    secrets::set_api_key(&provider, &key)
}

#[tauri::command]
pub fn has_api_key(provider: String) -> Result<bool> {
    secrets::has_api_key(&provider)
}

#[tauri::command]
pub fn delete_api_key(provider: String) -> Result<()> {
    secrets::delete_api_key(&provider)
}

// --- Models -----------------------------------------------------------------

#[tauri::command]
pub async fn list_models(db: State<'_, Db>, provider: String) -> Result<Vec<String>> {
    let ollama_host = db
        .get_setting("ollama_host")?
        .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;
    providers::list_models(&provider, api_key, &ollama_host).await
}

// --- Conversations & messages ----------------------------------------------

#[tauri::command]
pub fn create_conversation(db: State<Db>, title: String) -> Result<i64> {
    db.create_conversation(&title)
}

#[tauri::command]
pub fn list_conversations(db: State<Db>) -> Result<Vec<Conversation>> {
    db.list_conversations()
}

#[tauri::command]
pub fn rename_conversation(db: State<Db>, id: i64, title: String) -> Result<()> {
    db.rename_conversation(id, &title)
}

#[tauri::command]
pub fn delete_conversation(db: State<Db>, id: i64) -> Result<()> {
    db.delete_conversation(id)
}

#[tauri::command]
pub fn get_messages(db: State<Db>, conversation_id: i64) -> Result<Vec<Message>> {
    db.get_messages(conversation_id)
}

#[tauri::command]
pub fn add_message(
    db: State<Db>,
    conversation_id: i64,
    role: String,
    content: String,
) -> Result<i64> {
    db.add_message(conversation_id, &role, &content)
}

const PLACEHOLDER_TITLE: &str = "New conversation";

/// After the first exchange, replace the placeholder title with one Donna generates.
async fn maybe_generate_title(
    db: &Db,
    conversation_id: i64,
    provider: &str,
    model: &str,
    api_key: Option<String>,
    ollama_host: &str,
) -> Result<()> {
    let current = db
        .list_conversations()?
        .into_iter()
        .find(|c| c.id == conversation_id)
        .map(|c| c.title)
        .unwrap_or_default();

    if current != PLACEHOLDER_TITLE {
        return Ok(());
    }

    let messages = db.get_messages(conversation_id)?;
    if !messages.iter().any(|m| m.role == "assistant") {
        return Ok(());
    }

    let transcript: String = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: "Write a short conversation title (3-6 words) that captures the topic. \
                      No quotes, no trailing punctuation. Return ONLY the title."
                .into(),
        },
        ChatTurn {
            role: "user".into(),
            content: transcript,
        },
    ];

    let raw = providers::complete(provider, model, api_key, ollama_host, &turns).await?;
    let title = raw
        .trim()
        .trim_matches('"')
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    if !title.is_empty() && title.len() <= 80 {
        db.rename_conversation(conversation_id, &title)?;
    }
    Ok(())
}

// --- Streaming chat ---------------------------------------------------------

/// Events streamed to the frontend over a Tauri channel during a chat completion.
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatEvent {
    Token { content: String },
    Done { message_id: i64 },
    Error { message: String },
}

/// Generate an assistant reply for a conversation, streaming tokens to the frontend
/// and persisting the final assistant message.
#[tauri::command]
pub async fn send_chat(
    db: State<'_, Db>,
    conversation_id: i64,
    on_event: Channel<ChatEvent>,
) -> Result<()> {
    let config = load_config(&db)?;
    if config.model.is_empty() {
        let _ = on_event.send(ChatEvent::Error {
            message: "No model selected. Pick one in Settings.".into(),
        });
        return Ok(());
    }

    let api_key = secrets::get_api_key(&config.provider)?;

    // Build the prompt: persona + basics audit + live knowledge + conversation history.
    let retrieval_query = db
        .get_messages(conversation_id)?
        .into_iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content);
    let retrieval_ctx = if let Some(ref query) = retrieval_query {
        let cfg = retrieval::RetrievalConfig {
            provider: &config.provider,
            ollama_host: &config.ollama_host,
            embed_model: &config.embed_model,
        };
        retrieval::search_for_prompt(query, &db, &cfg).await?
    } else {
        String::new()
    };
    let mut system_content = build_system_prompt(&config, Some(&retrieval_ctx))?;
    let user_message_count = db
        .get_messages(conversation_id)?
        .iter()
        .filter(|m| m.role == "user")
        .count();
    if user_message_count <= 2 {
        system_content.push_str(
            "\n\n## Session note\nThis is an early conversation. Core identity basics \
             are likely still missing. Prioritize donna-ask questions for name, age, \
             nationality, and birthday before anything else.",
        );
    }

    let mut turns: Vec<ChatTurn> = vec![ChatTurn {
        role: "system".into(),
        content: system_content,
    }];
    for m in db.get_messages(conversation_id)? {
        turns.push(ChatTurn {
            role: m.role,
            content: m.content,
        });
    }

    let mut answer = String::new();
    let result = providers::stream_chat(
        &config.provider,
        &config.model,
        api_key,
        &config.ollama_host,
        &turns,
        |token| {
            answer.push_str(token);
            let _ = on_event.send(ChatEvent::Token {
                content: token.to_string(),
            });
        },
    )
    .await;

    match result {
        Ok(()) => {
            let id = db.add_message(conversation_id, "assistant", &answer)?;
            let _ = maybe_generate_title(
                &db,
                conversation_id,
                &config.provider,
                &config.model,
                secrets::get_api_key(&config.provider)?,
                &config.ollama_host,
            )
            .await;
            let _ = on_event.send(ChatEvent::Done { message_id: id });
            Ok(())
        }
        Err(e) => {
            let _ = on_event.send(ChatEvent::Error {
                message: e.to_string(),
            });
            Err(Error::Provider(e.to_string()))
        }
    }
}

// --- Knowledge base / Mind Map ---------------------------------------------
//
// The knowledge base is a folder tree on disk (see `knowledge.rs`). The Mind Map UI
// consumes a flat node list where each node's `group` is its folder path, so categories
// and sub-folder branches render as clusters.

#[derive(Serialize)]
pub struct GraphNode {
    /// Globally-unique id: folder path + file id.
    pub id: String,
    pub label: String,
    /// Folder path joined with " / " — drives clustering/branching in the UI.
    pub group: String,
    pub note: String,
    pub updated_at: String,
    /// Raw folder path components, for editing/moving the node.
    pub folder: Vec<String>,
    /// File id (slug) within the folder.
    pub file_id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub has_image: bool,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn to_graph_node(n: knowledge::KbNode) -> GraphNode {
    GraphNode {
        id: knowledge::content_node_id(&n.folder, &n.id),
        label: n.label,
        group: n.folder.join(" / "),
        note: n.note,
        updated_at: n.updated,
        folder: n.folder,
        file_id: n.id,
        node_type: n.node_type,
        has_image: n.has_image,
    }
}

fn to_folder_graph_node(path: &[String], name: &str) -> GraphNode {
    GraphNode {
        id: knowledge::folder_node_id(path),
        label: name.to_string(),
        group: path.join(" / "),
        note: String::new(),
        updated_at: String::new(),
        folder: path.to_vec(),
        file_id: String::new(),
        node_type: "folder".into(),
        has_image: false,
    }
}

#[tauri::command]
pub fn kg_graph() -> Result<GraphResponse> {
    let g = knowledge::graph()?;
    let mut nodes: Vec<GraphNode> = g
        .folders
        .iter()
        .map(|f| to_folder_graph_node(&f.path, &f.name))
        .collect();
    nodes.extend(g.nodes.iter().map(|n| to_graph_node(n.clone())));

    let edges = knowledge::hierarchy_edges(&g)
        .into_iter()
        .map(|(source, target)| GraphEdge { source, target })
        .collect();

    Ok(GraphResponse { nodes, edges })
}

/// Wipe the knowledge base, all chat history, and profile onboarding so Donna starts fresh.
#[tauri::command]
pub fn kg_reset(db: State<Db>) -> Result<()> {
    knowledge::reset()?;
    db.delete_all_conversations()?;
    db.clear_embeddings()?;
    db.set_setting("profile_onboarded", "false")?;
    Ok(())
}

#[tauri::command]
pub async fn kg_save_node(
    db: State<'_, Db>,
    folder: Vec<String>,
    label: String,
    note: String,
    node_type: String,
    from_folder: Option<Vec<String>>,
    from_id: Option<String>,
) -> Result<GraphNode> {
    let node = knowledge::save_node(
        &folder,
        &label,
        &note,
        &node_type,
        from_folder.as_deref(),
        from_id.as_deref(),
    )?;
    let config = load_config(&db)?;
    if config.provider == "ollama" && !config.embed_model.is_empty() {
        let _ = embeddings::index_node(
            &db,
            &config.ollama_host,
            &config.embed_model,
            &node,
        )
        .await;
    }
    Ok(to_graph_node(node))
}

#[tauri::command]
pub fn kg_delete_node(folder: Vec<String>, id: String) -> Result<()> {
    knowledge::delete_node(&folder, &id)
}

#[tauri::command]
pub fn kg_node_image(folder: Vec<String>, id: String) -> Result<Option<String>> {
    knowledge::node_image(&folder, &id)
}

#[tauri::command]
pub fn kg_set_node_image(folder: Vec<String>, id: String, source_path: String) -> Result<()> {
    knowledge::set_node_image(&folder, &id, &source_path)
}

#[tauri::command]
pub fn kg_remove_node_image(folder: Vec<String>, id: String) -> Result<()> {
    knowledge::remove_node_image(&folder, &id)
}

const CURATION_PROMPT: &str = "You are Donna's memory curator. Decide whether the \
conversation below contains durable, useful knowledge about the USER that is worth \
remembering long-term. Save ONLY things specifically about this user: facts about their \
life, work, or study; their routines; their stated preferences; explicit feedback they \
give Donna; and important people or projects in their life. DO NOT save general world \
knowledge, your own answers, or transient/trivial chit-chat. It is good and normal to \
save nothing.\n\n## Folder hierarchy (important)\nOrganize each memory with a \"path\" \
array of 2–5 folder segments (deepest folder holds the node). Think like a mind map: \
category → branch → sub-branch → … → node.\n\
- Segment 1: top category — About You, Work, Study, People, Projects, Routines, Feedback\n\
- Segment 2+: meaningful branches YOU invent — reuse existing branches from the tree below \
when the fact belongs there; create new branches to group related facts\n\nExamples:\n\
- Vietnamese, loves pho → path [\"About You\",\"Nationality\",\"Vietnam\"], label \
\"Favorite food\", note …\n\
- Favorite city HCMC → path [\"About You\",\"Nationality\",\"Vietnam\"], label \
\"Favorite city\", note …\n\
- Works at Google as engineer → path [\"Work\",\"Google\"], label \"Role\", note …\n\
- MS at MIT → path [\"Study\",\"MIT\"], label \"Degree\", note …\n\
- Manager Alex → path [\"People\",\"Work\",\"Alex\"], label \"Manager\", note …\n\n\
Identity basics: use branches like About You/Identity (name, age, birthday), \
About You/Nationality (country + culture/food/places), About You/Location (city, \
timezone). Never save vague meta without the actual value.\n\nFor each memory: path \
(array), label (short node title), type (info|routine|feedback|preference|person|project), \
note (1-2 sentences in your words).\n\nReturn ONLY valid JSON (no prose, no code fences):\n\
{\"memories\":[{\"path\":[\"About You\",\"Nationality\",\"Vietnam\"],\"label\":\
\"Favorite food\",\"type\":\"preference\",\"note\":\"…\"}]}\nIf nothing qualifies, return \
{\"memories\":[]}.";

/// Ask Donna to decide what (if anything) from a conversation is worth remembering, then
/// write those memories into the folder-based knowledge base. Returns the count saved.
#[tauri::command]
pub async fn kg_extract(db: State<'_, Db>, conversation_id: i64) -> Result<usize> {
    let config = load_config(&db)?;
    if config.model.is_empty() {
        return Ok(0);
    }
    let api_key = secrets::get_api_key(&config.provider)?;

    let transcript: String = db
        .get_messages(conversation_id)?
        .iter()
        .map(|m| format!("{}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    if transcript.trim().is_empty() {
        return Ok(0);
    }

    let tree = knowledge::tree_context_for_prompt()?;
    let user_content = format!(
        "Existing knowledge tree (reuse these branches when facts fit; extend with new \
         sub-folders as needed):\n{tree}\n\nConversation:\n{transcript}"
    );

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: CURATION_PROMPT.into(),
        },
        ChatTurn {
            role: "user".into(),
            content: user_content,
        },
    ];

    let raw = providers::complete(
        &config.provider,
        &config.model,
        api_key,
        &config.ollama_host,
        &turns,
    )
    .await?;

    let parsed = match extract_json(&raw) {
        Some(v) => v,
        None => return Ok(0),
    };

    let mut count = 0usize;
    if let Some(memories) = parsed.get("memories").and_then(|m| m.as_array()) {
        for mem in memories {
            let label = mem.get("label").and_then(|v| v.as_str()).unwrap_or("").trim();
            if label.is_empty() {
                continue;
            }
            let note = mem.get("note").and_then(|v| v.as_str()).unwrap_or("");
            let node_type = mem.get("type").and_then(|v| v.as_str()).unwrap_or("info");

            let folder = memory_folder_path(mem);
            if folder.is_empty() {
                continue;
            }

            knowledge::save_node(&folder, label, note, node_type, None, None)?;
            if config.provider == "ollama" && !config.embed_model.is_empty() {
                if let Ok(graph) = knowledge::graph() {
                    if let Some(node) = graph.nodes.iter().find(|n| {
                        n.folder == folder && n.label.eq_ignore_ascii_case(label)
                    }) {
                        let _ = embeddings::index_node(
                            &db,
                            &config.ollama_host,
                            &config.embed_model,
                            node,
                        )
                        .await;
                    }
                }
            }
            count += 1;
        }
    }

    Ok(count)
}

/// Resolve a memory's folder path from `path` (preferred) or legacy category/subcategory.
fn memory_folder_path(mem: &serde_json::Value) -> Vec<String> {
    if let Some(parts) = mem.get("path").and_then(|v| v.as_array()) {
        let path: Vec<String> = parts
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if !path.is_empty() {
            return path;
        }
    }

    let category = mem
        .get("category")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("About You");
    let mut folder = vec![category.to_string()];
    if let Some(sub) = mem
        .get("subcategory")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        folder.push(sub.to_string());
    }
    folder
}

/// Pull the first JSON object out of a possibly noisy model response.
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

#[tauri::command]
pub async fn kg_reindex_embeddings(db: State<'_, Db>) -> Result<usize> {
    let config = load_config(&db)?;
    if config.provider != "ollama" || config.embed_model.is_empty() {
        return Ok(0);
    }
    embeddings::reindex_all(&db, &config.ollama_host, &config.embed_model).await
}

// --- Integrations ----------------------------------------------------------

#[tauri::command]
pub fn integrations_status() -> Result<Vec<integrations::IntegrationStatus>> {
    integrations::status()
}

#[tauri::command]
pub fn google_set_client(client_id: String, client_secret: String) -> Result<()> {
    google::set_client(&client_id, &client_secret)
}

#[tauri::command]
pub async fn google_connect() -> Result<()> {
    google::connect().await
}

#[tauri::command]
pub fn google_disconnect() -> Result<()> {
    google::disconnect()
}

#[tauri::command]
pub async fn calendar_list_events(
    time_min: String,
    time_max: String,
) -> Result<Vec<google::CalendarEvent>> {
    google::list_events(&time_min, &time_max).await
}

#[tauri::command]
pub async fn calendar_create_event(
    event: google::CalendarEvent,
) -> Result<google::CalendarEvent> {
    google::create_event(&event).await
}

#[tauri::command]
pub async fn calendar_update_event(
    id: String,
    event: google::CalendarEvent,
) -> Result<google::CalendarEvent> {
    google::update_event(&id, &event).await
}

#[tauri::command]
pub async fn calendar_delete_event(id: String) -> Result<()> {
    google::delete_event(&id).await
}

#[tauri::command]
pub fn slack_set_token(token: String) -> Result<()> {
    slack::set_token(&token)
}

#[tauri::command]
pub fn slack_disconnect() -> Result<()> {
    slack::disconnect()
}

#[tauri::command]
pub async fn slack_list_channels() -> Result<Vec<slack::SlackChannel>> {
    slack::list_channels().await
}

#[tauri::command]
pub async fn slack_send_message(channel: String, text: String) -> Result<()> {
    slack::send_message(&channel, &text).await
}

#[tauri::command]
pub fn fathom_set_key(key: String) -> Result<()> {
    integrations::fathom::set_key(&key)
}

#[tauri::command]
pub fn fathom_disconnect() -> Result<()> {
    integrations::fathom::disconnect()
}

// --- Routines ---------------------------------------------------------------

#[tauri::command]
pub fn list_routines(db: State<Db>) -> Result<Vec<Routine>> {
    db.list_routines()
}

#[tauri::command]
pub fn toggle_routine(db: State<Db>, id: i64, enabled: bool) -> Result<()> {
    db.toggle_routine(id, enabled)
}

#[derive(Deserialize)]
pub struct CreateRoutineInput {
    pub name: String,
    pub schedule_type: String,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub day_of_week: Option<i32>,
    pub minutes_before: Option<i32>,
    pub prompt: Option<String>,
}

#[tauri::command]
pub fn create_routine(db: State<Db>, input: CreateRoutineInput) -> Result<i64> {
    db.create_routine(
        &input.name,
        &input.schedule_type,
        input.hour,
        input.minute,
        input.day_of_week,
        input.minutes_before,
        input.prompt.as_deref(),
    )
}

#[tauri::command]
pub fn delete_routine(db: State<Db>, id: i64) -> Result<()> {
    db.delete_routine(id)
}

// --- Notifications ----------------------------------------------------------

#[tauri::command]
pub fn list_notifications(db: State<Db>) -> Result<Vec<Notification>> {
    db.list_notifications()
}

#[tauri::command]
pub fn mark_notification_read(db: State<Db>, id: i64) -> Result<()> {
    db.mark_notification_read(id)
}

// --- Docs -------------------------------------------------------------------

#[tauri::command]
pub fn list_docs(db: State<Db>) -> Result<Vec<Doc>> {
    db.list_docs()
}

#[tauri::command]
pub fn get_doc(db: State<Db>, id: i64) -> Result<Doc> {
    db.get_doc(id)?
        .ok_or_else(|| Error::Provider(format!("Document {id} not found")))
}

#[tauri::command]
pub fn delete_doc(db: State<Db>, id: i64) -> Result<()> {
    db.delete_doc(id)
}

// --- Gmail ------------------------------------------------------------------

#[tauri::command]
pub async fn gmail_list_messages(max_results: u32) -> Result<Vec<google::GmailMessage>> {
    google::list_gmail_messages(max_results).await
}

#[tauri::command]
pub async fn google_create_doc(title: String) -> Result<String> {
    google::create_google_doc(&title).await
}

#[tauri::command]
pub async fn gmail_create_draft(to: String, subject: String, body: String) -> Result<String> {
    google::create_gmail_draft(&to, &subject, &body).await
}

#[tauri::command]
pub async fn drive_list_files(max_results: u32) -> Result<Vec<google::DriveFile>> {
    google::list_drive_files(max_results).await
}

// --- GitHub -----------------------------------------------------------------

#[tauri::command]
pub fn github_set_token(token: String) -> Result<()> {
    github::set_token(&token)
}

#[tauri::command]
pub fn github_disconnect() -> Result<()> {
    github::disconnect()
}

#[tauri::command]
pub async fn github_list_repos(max_results: u32) -> Result<Vec<github::GitHubRepo>> {
    github::list_repos(max_results).await
}

#[tauri::command]
pub async fn github_list_issues(max_results: u32) -> Result<Vec<github::GitHubIssue>> {
    github::list_issues(max_results).await
}

// --- Linear -----------------------------------------------------------------

#[tauri::command]
pub fn linear_set_key(key: String) -> Result<()> {
    linear::set_key(&key)
}

#[tauri::command]
pub fn linear_disconnect() -> Result<()> {
    linear::disconnect()
}

#[tauri::command]
pub async fn linear_list_issues(max_results: u32) -> Result<Vec<linear::LinearIssue>> {
    linear::list_issues(max_results).await
}

// --- Notion -----------------------------------------------------------------

#[tauri::command]
pub fn notion_set_token(token: String) -> Result<()> {
    notion::set_token(&token)
}

#[tauri::command]
pub fn notion_disconnect() -> Result<()> {
    notion::disconnect()
}

#[tauri::command]
pub async fn notion_search_pages(max_results: u32) -> Result<Vec<notion::NotionPage>> {
    notion::search_pages(max_results).await
}

// --- Telegram ---------------------------------------------------------------

#[tauri::command]
pub fn telegram_set_credentials(bot_token: String, chat_id: String) -> Result<()> {
    telegram::set_credentials(&bot_token, &chat_id)
}

#[tauri::command]
pub fn telegram_disconnect() -> Result<()> {
    telegram::disconnect()
}

#[tauri::command]
pub async fn telegram_send_message(text: String) -> Result<()> {
    telegram::send_message(&text).await
}

// --- WhatsApp ---------------------------------------------------------------

#[tauri::command]
pub fn whatsapp_set_credentials(access_token: String, phone_number_id: String) -> Result<()> {
    whatsapp::set_credentials(&access_token, &phone_number_id)
}

#[tauri::command]
pub fn whatsapp_disconnect() -> Result<()> {
    whatsapp::disconnect()
}

#[tauri::command]
pub async fn whatsapp_send_message(to: String, text: String) -> Result<()> {
    whatsapp::send_message(&to, &text).await
}

// --- Projects ----------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[tauri::command]
pub async fn project_list(db: State<'_, Db>) -> Result<Vec<crate::db::Project>> {
    db.list_projects()
}

#[tauri::command]
pub async fn project_create(
    db: State<'_, Db>,
    name: String,
    template: String,
    path: String,
) -> Result<crate::db::Project> {
    // Create the directory structure based on template
    let project_path = std::path::Path::new(&path);
    std::fs::create_dir_all(project_path).map_err(|e| Error::Provider(e.to_string()))?;

    match template.as_str() {
        "coding" => {
            std::fs::write(project_path.join("README.md"), format!("# {name}\n\nProject description here.\n")).ok();
            std::fs::write(project_path.join(".gitignore"), "target/\nnode_modules/\n.env\n*.lock\ndist/\n").ok();
            std::fs::create_dir_all(project_path.join("src")).ok();
        }
        "research" => {
            let paper = format!("# {name}\n\n## Abstract\n\n## Introduction\n\n## Literature Review\n\n## Methodology\n\n## Results\n\n## Discussion\n\n## Conclusion\n\n## References\n");
            std::fs::write(project_path.join("paper.md"), paper).ok();
            std::fs::write(project_path.join("notes.md"), "# Research Notes\n\n").ok();
            std::fs::write(project_path.join("references.md"), "# References\n\n| # | Title | Authors | Year | DOI | Notes |\n|---|-------|---------|------|-----|-------|\n").ok();
            std::fs::create_dir_all(project_path.join("data")).ok();
            std::fs::create_dir_all(project_path.join("figures")).ok();
        }
        _ => {
            std::fs::write(project_path.join("README.md"), format!("# {name}\n\n")).ok();
        }
    }

    let id = db.create_project(&name, &template, &path)?;
    Ok(crate::db::Project {
        id,
        name,
        template,
        path,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn project_delete(db: State<'_, Db>, id: i64) -> Result<()> {
    db.delete_project(id)
}

#[tauri::command]
pub async fn project_open_in_editor(path: String) -> Result<()> {
    // Try VS Code first, then Cursor, then system default
    let editors = ["cursor", "code", "zed"];
    for editor in &editors {
        if std::process::Command::new(editor).arg(&path).spawn().is_ok() {
            return Ok(());
        }
    }
    open::that(&path).map_err(|e| Error::Provider(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn project_list_files(project_id: i64, db: State<'_, Db>) -> Result<Vec<ProjectFile>> {
    let projects = db.list_projects()?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Ok(vec![]);
    };
    let root = std::path::Path::new(&project.path);
    let mut files = Vec::new();
    collect_files(root, root, &mut files, 0)?;
    Ok(files)
}

fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<ProjectFile>,
    depth: usize,
) -> Result<()> {
    if depth > 4 {
        return Ok(());
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| {
        let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
        (is_file as u8, e.file_name())
    });
    for entry in entries {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name != ".gitignore" {
            continue;
        }
        let rel = entry_path.strip_prefix(root).unwrap_or(&entry_path);
        let is_dir = entry_path.is_dir();
        out.push(ProjectFile {
            name: name.clone(),
            path: rel.to_string_lossy().to_string(),
            is_dir,
        });
        if is_dir {
            collect_files(root, &entry_path, out, depth + 1)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn project_read_file(project_id: i64, path: String, db: State<'_, Db>) -> Result<String> {
    let projects = db.list_projects()?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Err(Error::Provider("Project not found".into()));
    };
    let full_path = std::path::Path::new(&project.path).join(&path);
    std::fs::read_to_string(&full_path).map_err(|e| Error::Provider(e.to_string()))
}

#[tauri::command]
pub async fn project_write_file(project_id: i64, path: String, content: String, db: State<'_, Db>) -> Result<()> {
    let projects = db.list_projects()?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Err(Error::Provider("Project not found".into()));
    };
    let full_path = std::path::Path::new(&project.path).join(&path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&full_path, content).map_err(|e| Error::Provider(e.to_string()))
}

// --- Discord -----------------------------------------------------------------

#[tauri::command]
pub async fn discord_set_token(token: String) -> Result<()> {
    discord::set_token(&token)
}

#[tauri::command]
pub async fn discord_disconnect() -> Result<()> {
    discord::disconnect()
}

// --- Fathom post-meeting processing -----------------------------------------

#[tauri::command]
pub async fn fathom_process_recent_meeting(db: State<'_, Db>) -> Result<String> {
    use crate::integrations::fathom;

    let provider = db.get_setting("provider")?.unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model")?.unwrap_or_default();
    let ollama_host = db.get_setting("ollama_host")?.unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;

    if !fathom::is_connected().unwrap_or(false) {
        return Err(Error::Provider("Fathom not connected".into()));
    }
    let meetings = fathom::list_recent_meetings(1).await?;
    let Some(meeting) = meetings.first() else {
        return Err(Error::Provider("No recent Fathom meetings found".into()));
    };

    let title = meeting.title.as_deref().unwrap_or("Meeting");
    let summary = meeting.summary.as_deref().unwrap_or("(no summary)");

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: "You are Donna, a proactive personal assistant. Analyze this meeting summary and produce: 1) Key decisions, 2) Action items with owners, 3) Follow-up emails to draft, 4) Things to add to the knowledge base. Use Markdown.".into(),
        },
        ChatTurn {
            role: "user".into(),
            content: format!("## Meeting: {title}\n\n## Summary\n{summary}"),
        },
    ];

    let content = providers::complete(&provider, &model, api_key, &ollama_host, &turns).await?;
    let doc_title = format!("Post-Meeting: {title}");
    let doc_id = crate::docs::create(&db, &doc_title, "fathom_post_meeting", &content)?;
    db.insert_notification(
        &format!("Meeting processed: {title}"),
        "Donna has analysed your meeting and created action items.",
        Some("open_doc"),
        Some(doc_id),
    )?;
    Ok(content)
}

// --- News --------------------------------------------------------------------

#[tauri::command]
pub async fn news_fetch_latest() -> Result<String> {
    let stories = crate::integrations::news::top_stories(15).await?;
    Ok(crate::integrations::news::format_digest(&stories))
}

// --- Reading list ------------------------------------------------------------

#[tauri::command]
pub async fn reading_list_add(db: State<'_, Db>, url: String, title: String) -> Result<crate::db::ReadingListItem> {
    let id = db.reading_list_add(&url, &title)?;
    Ok(crate::db::ReadingListItem {
        id,
        url,
        title,
        summary: None,
        tags: None,
        read: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn reading_list_get(db: State<'_, Db>) -> Result<Vec<crate::db::ReadingListItem>> {
    db.reading_list_get()
}

#[tauri::command]
pub async fn reading_list_summarize(db: State<'_, Db>, id: i64) -> Result<String> {
    let items = db.reading_list_get()?;
    let Some(item) = items.iter().find(|i| i.id == id) else {
        return Err(Error::Provider("Item not found".into()));
    };
    let provider = db.get_setting("provider")?.unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model")?.unwrap_or_default();
    let ollama_host = db.get_setting("ollama_host")?.unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;
    let turns = vec![
        ChatTurn { role: "system".into(), content: "Summarize this article URL in 3-5 bullet points for a busy professional. Focus on key insights and actionable takeaways.".into() },
        ChatTurn { role: "user".into(), content: format!("URL: {}\nTitle: {}", item.url, item.title) },
    ];
    let summary = providers::complete(&provider, &model, api_key, &ollama_host, &turns).await?;
    db.reading_list_update_summary(id, &summary)?;
    Ok(summary)
}

#[tauri::command]
pub async fn reading_list_delete(db: State<'_, Db>, id: i64) -> Result<()> {
    db.reading_list_delete(id)
}

// --- Focus sessions ----------------------------------------------------------

#[tauri::command]
pub async fn focus_start(db: State<'_, Db>, label: String, duration_min: i32) -> Result<crate::db::FocusSession> {
    let id = db.focus_start(&label, duration_min)?;
    // Schedule a notification when time is up
    let _notif_title = format!("Focus session complete: {label}");
    let _body = format!("Your {duration_min}-minute focus block is done. Time for a break.");
    // We can't sleep here, so just log it and let user end manually
    Ok(crate::db::FocusSession {
        id,
        label,
        duration_min,
        started_at: chrono::Utc::now().to_rfc3339(),
        ended_at: None,
    })
}

#[tauri::command]
pub async fn focus_end(db: State<'_, Db>, id: i64) -> Result<()> {
    db.focus_end(id)
}

#[tauri::command]
pub async fn focus_active(db: State<'_, Db>) -> Result<Option<crate::db::FocusSession>> {
    db.focus_active()
}

// --- Habits ------------------------------------------------------------------

#[tauri::command]
pub async fn habit_create(db: State<'_, Db>, name: String, description: Option<String>) -> Result<crate::db::Habit> {
    let id = db.habit_create(&name, description.as_deref())?;
    Ok(crate::db::Habit {
        id,
        name,
        description,
        enabled: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn habit_list(db: State<'_, Db>) -> Result<Vec<crate::db::Habit>> {
    db.habit_list()
}

#[tauri::command]
pub async fn habit_log(db: State<'_, Db>, habit_id: i64, note: Option<String>) -> Result<()> {
    db.habit_log(habit_id, note.as_deref())
}

#[tauri::command]
pub async fn habit_logged_today(db: State<'_, Db>, habit_id: i64) -> Result<bool> {
    db.habit_logged_today(habit_id)
}

// --- Project status report ---------------------------------------------------

#[tauri::command]
pub async fn project_status_report(db: State<'_, Db>, project_id: i64) -> Result<String> {
    let projects = db.list_projects()?;
    let Some(project) = projects.iter().find(|p| p.id == project_id) else {
        return Err(Error::Provider("Project not found".into()));
    };
    let provider = db.get_setting("provider")?.unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model")?.unwrap_or_default();
    let ollama_host = db.get_setting("ollama_host")?.unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;

    // Collect file contents for context
    let root = std::path::Path::new(&project.path);
    let mut file_contents = String::new();
    for entry in walkdir_shallow(root) {
        let content = std::fs::read_to_string(&entry).unwrap_or_default();
        if !content.is_empty() {
            file_contents.push_str(&format!("\n### {}\n{content}", entry.display()));
        }
    }

    let turns = vec![
        ChatTurn { role: "system".into(), content: "Generate a concise project status report with: current status, what's done, what's in progress, blockers, and next steps. Use Markdown.".into() },
        ChatTurn { role: "user".into(), content: format!("Project: {}\nTemplate: {}\n\nFiles:\n{file_contents}", project.name, project.template) },
    ];
    let report = providers::complete(&provider, &model, api_key, &ollama_host, &turns).await?;
    let doc_id = crate::docs::create(&db, &format!("Status Report: {}", project.name), "project_status", &report)?;
    db.insert_notification(
        &format!("Status report: {}", project.name),
        "Project status report is ready in Docs.",
        Some("open_doc"),
        Some(doc_id),
    )?;
    Ok(report)
}

fn walkdir_shallow(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else { return files; };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if matches!(ext.to_str(), Some("md" | "txt" | "rs" | "ts" | "tsx" | "py" | "js" | "toml")) {
                    files.push(path);
                }
            }
        }
    }
    files
}

// --- Quick Chat ---------------------------------------------------------------

#[tauri::command]
pub fn quick_chat_context(
    state: tauri::State<'_, crate::quick_chat::QuickChatState>,
) -> crate::quick_chat::QuickChatContext {
    state.ctx.lock().unwrap().clone()
}

#[tauri::command]
pub async fn quick_chat_send(
    db: State<'_, Db>,
    message: String,
    app_name: String,
    on_event: Channel<ChatEvent>,
) -> Result<()> {
    let provider = db.get_setting("provider")?.unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model")?.unwrap_or_default();
    let ollama_host = db
        .get_setting("ollama_host")?
        .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;

    let system = format!(
        "You are Donna, a sharp and concise AI assistant. \
        The user pressed Cmd+D while working in \"{app_name}\" and asked you a quick question. \
        Answer briefly and practically. No pleasantries needed."
    );

    let turns = vec![
        ChatTurn { role: "system".into(), content: system },
        ChatTurn { role: "user".into(), content: message },
    ];

    let mut answer = String::new();
    let result = providers::stream_chat(
        &provider,
        &model,
        api_key,
        &ollama_host,
        &turns,
        |token| {
            answer.push_str(token);
            let _ = on_event.send(ChatEvent::Token { content: token.to_string() });
        },
    )
    .await;

    match result {
        Ok(()) => {
            let _ = on_event.send(ChatEvent::Done { message_id: 0 });
            Ok(())
        }
        Err(e) => {
            let _ = on_event.send(ChatEvent::Error { message: e.to_string() });
            Err(e)
        }
    }
}

// --- News items (structured) --------------------------------------------------

#[tauri::command]
pub async fn news_list_items(
    limit: Option<usize>,
) -> Result<Vec<crate::integrations::news::NewsItem>> {
    crate::integrations::news::top_stories(limit.unwrap_or(10)).await
}

#[tauri::command]
pub async fn news_article_summary(db: State<'_, Db>, url: String) -> Result<String> {
    let provider = db.get_setting("provider")?.unwrap_or_else(|| "ollama".into());
    let model = db.get_setting("model")?.unwrap_or_default();
    let ollama_host = db
        .get_setting("ollama_host")?
        .unwrap_or_else(|| providers::DEFAULT_OLLAMA_HOST.into());
    let api_key = secrets::get_api_key(&provider)?;

    let html = reqwest::get(&url)
        .await
        .map_err(|e| Error::Provider(e.to_string()))?
        .text()
        .await
        .map_err(|e| Error::Provider(e.to_string()))?;

    let text = strip_html_text(&html);
    let excerpt: String = text.chars().take(4500).collect();

    let turns = vec![
        ChatTurn {
            role: "system".into(),
            content: "You are a concise news summarizer. Write a 3-4 sentence plain summary \
                of the key points from the article. Be factual and skip metadata/ads/nav."
                .into(),
        },
        ChatTurn {
            role: "user".into(),
            content: format!("Source: {url}\n\n{excerpt}"),
        },
    ];

    providers::complete(&provider, &model, api_key, &ollama_host, &turns).await
}

fn strip_html_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut skip_block = false;
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Detect <script> and <style> block starts
        if !in_tag && i + 7 <= len {
            let chunk = html[i..].to_lowercase();
            if chunk.starts_with("<script") || chunk.starts_with("<style") {
                skip_block = true;
                in_tag = true;
            }
            if skip_block && (chunk.starts_with("</script>") || chunk.starts_with("</style>")) {
                skip_block = false;
                // skip to '>'
                while i < len && bytes[i] != b'>' { i += 1; }
                i += 1;
                continue;
            }
        }

        let c = bytes[i] as char;
        match c {
            '<' => { in_tag = true; }
            '>' => { in_tag = false; if !skip_block { out.push(' '); } }
            _ if !in_tag && !skip_block => out.push(c),
            _ => {}
        }
        i += 1;
    }

    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

//! The agent loop: tool-calling chat for OpenAI. `send_chat` routes here when the
//! provider is OpenAI and an API key is set; every other provider stays on plain
//! `providers::stream_chat`.
//!
//! One turn = up to [`MAX_ITERATIONS`] model steps. Each step streams text (forwarded
//! as `ChatEvent::Token`) and may request tool calls. Read/Write tools run
//! automatically; outbound tools are gated by [`crate::trust`] and, when they need
//! approval, filed out-of-band with a `ChatEvent::Approval` while the model is told to
//! move on. The loop ends when the model returns no tool calls (final answer), the
//! token budget is exceeded, or the iteration cap is hit — every path persists an
//! assistant message and emits exactly one `Done`.

use std::collections::HashMap;

use crate::db::{Approval, Db, Message};
use crate::error::{Error, Result};
use crate::integrations::whatsapp;
use crate::ops::{self, ChatEvent};
use crate::providers::{self, AgentTurn, FunctionCall, ToolCallOut};
use crate::{tools, trust};

/// Hard cap on model steps per user turn (a runaway tool loop can't spin forever).
const MAX_ITERATIONS: usize = 12;
/// Once cumulative tokens for this turn exceed this, wrap up with an apology.
const TOKEN_BUDGET: u64 = 60_000;

const TOOL_DISABLED_MSG: &str = "TOOL_DISABLED: repeated failures";
const PENDING_APPROVAL_MSG: &str = "PENDING_APPROVAL: the user has been asked to approve \
this action out-of-band. Do not retry or work around it; acknowledge and continue.";

/// Appended to the shared chat system prompt for the agent (tool-calling) path only —
/// plain chat via `providers::stream_chat` never sees this.
const AGENT_SYSTEM_PROMPT_ADDENDUM: &str = "\n\n## Acting with tools\nYou have tools — use \
them to act rather than describing what you would do. Read and write tools run \
automatically. Outbound actions (messages to other people) may require the user's \
approval: when a tool returns PENDING_APPROVAL, tell the user you've asked for their \
approval and stop pursuing that action — never retry it or work around it. Never fabricate \
tool results. Prefer checking real data over guessing. You have skills (listed under \
'Available skills'). When a skill fits the task, call skill_view with its name to load \
its full instructions BEFORE acting, and follow its steps. If you work out a new \
repeatable multi-step recipe, consider skill_create to save it.";

/// Build the model-facing history: a system turn followed by each user/assistant
/// message as a content-only `AgentTurn`. Pure; the loop's own tool-call/tool-result
/// turns are appended later during iteration.
fn build_history_turns(messages: &[Message], system: String) -> Vec<AgentTurn> {
    let mut turns = Vec::with_capacity(messages.len() + 1);
    turns.push(AgentTurn {
        role: "system".into(),
        content: Some(system),
        tool_calls: None,
        tool_call_id: None,
    });
    for m in messages {
        turns.push(AgentTurn {
            role: m.role.clone(),
            content: Some(m.content.clone()),
            tool_calls: None,
            tool_call_id: None,
        });
    }
    turns
}

/// Per-turn tool failure counter. A tool that errors twice in one turn is disabled for
/// the rest of the turn so the model can't burn the whole budget retrying a broken call.
#[derive(Default)]
struct ToolErrorTracker(HashMap<String, u32>);

impl ToolErrorTracker {
    /// Record an error for `name`; return true once it should be disabled (2nd+ error).
    fn record_error(&mut self, name: &str) -> bool {
        let count = self.0.entry(name.to_string()).or_insert(0);
        *count += 1;
        *count >= 2
    }

    /// Whether `name` has already been disabled this turn.
    fn is_disabled(&self, name: &str) -> bool {
        self.0.get(name).is_some_and(|&c| c >= 2)
    }
}

/// Run one full agent turn for `conversation_id`, streaming events via `on_event` and
/// persisting the final assistant message. See the module docs for loop mechanics.
pub async fn run_agent_turn(
    db: &Db,
    conversation_id: i64,
    api_key: &str,
    model: &str,
    on_event: &(dyn Fn(ChatEvent) + Send + Sync),
) -> Result<()> {
    let config = ops::load_config(db)?;
    let mut system = ops::assemble_chat_system_prompt(db, &config, conversation_id).await?;
    system.push_str(AGENT_SYSTEM_PROMPT_ADDENDUM);
    let mut turns = build_history_turns(&db.get_messages(conversation_id)?, system);

    let tools_json = tools::openai_tools_json();
    let mut total_tokens: u64 = 0;
    let mut errors = ToolErrorTracker::default();

    for _ in 0..MAX_ITERATIONS {
        let mut step_content = String::new();
        let step = {
            let mut on_token = |t: &str| {
                step_content.push_str(t);
                on_event(ChatEvent::Token { content: t.to_string() });
            };
            match providers::openai_agent_step(api_key, model, &turns, &tools_json, &mut on_token)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    on_event(ChatEvent::Error { message: e.to_string() });
                    return Err(Error::Provider(e.to_string()));
                }
            }
        };
        total_tokens += step.total_tokens;

        // Budget exceeded: keep any partial content, apologize, persist, and stop.
        if total_tokens > TOKEN_BUDGET {
            let mut msg = step.content.clone();
            if !msg.is_empty() {
                msg.push_str("\n\n");
            }
            msg.push_str("I hit my token budget for this request.");
            let id = db.add_message(conversation_id, "assistant", &msg)?;
            on_event(ChatEvent::Done { message_id: id });
            return Ok(());
        }

        // No tool calls → this is the final answer.
        if step.tool_calls.is_empty() {
            let id = db.add_message(conversation_id, "assistant", &step.content)?;
            let _ = ops::maybe_generate_title(
                db,
                conversation_id,
                &config.provider,
                &config.model,
                Some(api_key.to_string()),
                &config.ollama_host,
            )
            .await;
            on_event(ChatEvent::Done { message_id: id });
            return Ok(());
        }

        // Push the assistant turn carrying the exact tool_calls the API returned (ids
        // preserved) — OpenAI 400s if a later tool result references an unknown id.
        turns.push(AgentTurn {
            role: "assistant".into(),
            content: (!step.content.is_empty()).then(|| step.content.clone()),
            tool_calls: Some(
                step.tool_calls
                    .iter()
                    .map(|c| ToolCallOut {
                        id: c.id.clone(),
                        kind: "function".into(),
                        function: FunctionCall {
                            name: c.name.clone(),
                            arguments: c.arguments.clone(),
                        },
                    })
                    .collect(),
            ),
            tool_call_id: None,
        });

        // Run each call in order; each produces exactly one tool-result turn.
        for call in &step.tool_calls {
            let result = handle_tool_call(db, conversation_id, call, &mut errors, on_event).await;
            turns.push(AgentTurn {
                role: "tool".into(),
                content: Some(result),
                tool_calls: None,
                tool_call_id: Some(call.id.clone()),
            });
        }
    }

    // Iteration cap: persist whatever the model last said, or a stock message.
    let last = last_assistant_content(&turns);
    let content = if last.is_empty() {
        "I hit my step limit on this one.".to_string()
    } else {
        last
    };
    let id = db.add_message(conversation_id, "assistant", &content)?;
    on_event(ChatEvent::Done { message_id: id });
    Ok(())
}

/// Resolve one tool call into the text that becomes its tool-result turn, emitting the
/// matching `Tool`/`Approval` events. Never returns Err — every failure is folded into
/// a string the model reads and self-corrects from.
async fn handle_tool_call(
    db: &Db,
    conversation_id: i64,
    call: &providers::ToolCall,
    errors: &mut ToolErrorTracker,
    on_event: &(dyn Fn(ChatEvent) + Send + Sync),
) -> String {
    // Already disabled this turn: skip execution entirely.
    if errors.is_disabled(&call.name) {
        return TOOL_DISABLED_MSG.to_string();
    }

    // Parse arguments; a parse failure becomes the result so the model can retry.
    let args: serde_json::Value = match serde_json::from_str(&call.arguments) {
        Ok(v) => v,
        Err(e) => {
            let result = format!("bad arguments for {}: {e}", call.name);
            return if errors.record_error(&call.name) {
                TOOL_DISABLED_MSG.to_string()
            } else {
                result
            };
        }
    };

    match trust::decide(db, &call.name) {
        Ok(trust::Decision::Auto) => {
            let label = tools::summarize_call(&call.name, &args);
            on_event(ChatEvent::Tool {
                name: call.name.clone(),
                label: label.clone(),
                status: "running".into(),
            });
            let result = tools::execute(db, &call.name, &args).await;
            let _ = db.insert_event("tool_call", Some(conversation_id), Some(&call.name), None);
            match result {
                Ok(result) => {
                    on_event(ChatEvent::Tool {
                        name: call.name.clone(),
                        label,
                        status: "done".into(),
                    });
                    result
                }
                Err(e) => {
                    on_event(ChatEvent::Tool {
                        name: call.name.clone(),
                        label,
                        status: "error".into(),
                    });
                    if errors.record_error(&call.name) {
                        TOOL_DISABLED_MSG.to_string()
                    } else {
                        e.to_string()
                    }
                }
            }
        }
        Ok(trust::Decision::Ask) => match trust::request_approval(db, conversation_id, &call.name, &args) {
            Ok((approval, newly_created)) => {
                on_event(ChatEvent::Approval {
                    approval_id: approval.id,
                    summary: approval.summary.clone(),
                    tool: call.name.clone(),
                });
                // Only push WhatsApp buttons for a freshly-filed approval — request_approval
                // dedupes identical re-asks, and pushing again on every model retry would
                // spam the user with duplicate button messages for the same approval.
                if newly_created {
                    push_whatsapp_approval(db, &approval).await;
                }
                PENDING_APPROVAL_MSG.to_string()
            }
            Err(e) => e.to_string(),
        },
        // Unknown tool (or trust lookup failure): feed the error back to the model.
        Err(e) => {
            let result = e.to_string();
            if errors.record_error(&call.name) {
                TOOL_DISABLED_MSG.to_string()
            } else {
                result
            }
        }
    }
}

/// Best-effort push of approve/reject buttons to WhatsApp for a freshly-filed approval.
/// No-op (silently) if WhatsApp isn't connected, no number is configured, or the send
/// fails — this is a convenience notification, not the approval's system of record.
async fn push_whatsapp_approval(db: &Db, approval: &Approval) {
    if !whatsapp::is_connected().unwrap_or(false) {
        return;
    }
    let Ok(Some(my_number)) = db.get_setting("whatsapp_my_number") else {
        return;
    };
    if my_number.is_empty() {
        return;
    }
    let _ = whatsapp::send_approval_buttons(&my_number, approval.id, &approval.summary).await;
}

/// The content of the most recent assistant `AgentTurn`, if any (used at the iteration cap).
fn last_assistant_content(turns: &[AgentTurn]) -> String {
    turns
        .iter()
        .rev()
        .find(|t| t.role == "assistant")
        .and_then(|t| t.content.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> Message {
        Message {
            id: 0,
            conversation_id: 1,
            role: role.into(),
            content: content.into(),
            created_at: String::new(),
        }
    }

    #[test]
    fn history_turns_shape() {
        let msgs = vec![msg("user", "hi"), msg("assistant", "hello")];
        let turns = build_history_turns(&msgs, "SYS".into());
        assert_eq!(turns[0].role, "system");
        assert_eq!(turns[0].content.as_deref(), Some("SYS"));
        assert_eq!(turns[1].content.as_deref(), Some("hi"));
        assert_eq!(turns.len(), 3);
    }

    #[test]
    fn tool_error_tracker_disables_on_second_failure() {
        let mut t = ToolErrorTracker::default();
        assert!(!t.record_error("kb_search"));
        assert!(t.record_error("kb_search")); // second → disable
        assert!(!t.record_error("list_docs")); // independent per tool
        assert!(t.is_disabled("kb_search"));
        assert!(!t.is_disabled("list_docs"));
    }

    fn test_db() -> Db {
        crate::secrets::init_test_file_store();
        let dir = std::env::temp_dir().join(format!(
            "donna-agent-{}-{}",
            std::process::id(),
            crate::db::unique_test_suffix()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Db::open(&dir.join("t.sqlite")).unwrap()
    }

    /// Bad JSON arguments must count as a strike, same as an execute() error: the
    /// second occurrence in a turn disables the tool rather than retrying forever.
    #[tokio::test]
    async fn parse_failure_counts_toward_two_strike_disable() {
        let db = test_db();
        let mut errors = ToolErrorTracker::default();
        let call = providers::ToolCall {
            id: "1".into(),
            name: "list_docs".into(),
            arguments: "{not json".into(),
        };

        let first = handle_tool_call(&db, 1, &call, &mut errors, &|_| {}).await;
        assert!(first.contains("bad arguments"));
        assert!(!errors.is_disabled("list_docs"));

        let second = handle_tool_call(&db, 1, &call, &mut errors, &|_| {}).await;
        assert_eq!(second, TOOL_DISABLED_MSG);
        assert!(errors.is_disabled("list_docs"));

        // Once disabled, short-circuits before even parsing.
        let third = handle_tool_call(&db, 1, &call, &mut errors, &|_| {}).await;
        assert_eq!(third, TOOL_DISABLED_MSG);
    }

    /// An unknown tool name (trust::decide's Err arm) also counts toward the strike.
    #[tokio::test]
    async fn unknown_tool_counts_toward_two_strike_disable() {
        let db = test_db();
        let mut errors = ToolErrorTracker::default();
        let call = providers::ToolCall {
            id: "1".into(),
            name: "no_such_tool".into(),
            arguments: "{}".into(),
        };

        let first = handle_tool_call(&db, 1, &call, &mut errors, &|_| {}).await;
        assert!(first.contains("unknown tool"));
        assert!(!errors.is_disabled("no_such_tool"));

        let second = handle_tool_call(&db, 1, &call, &mut errors, &|_| {}).await;
        assert_eq!(second, TOOL_DISABLED_MSG);
        assert!(errors.is_disabled("no_such_tool"));
    }
}

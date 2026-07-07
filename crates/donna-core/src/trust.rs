//! Trust engine: decides whether a tool call runs automatically or needs the user's
//! explicit approval, and files approval requests when it doesn't.
//!
//! Consulted by the agent loop (Task 5) before every tool call:
//! - Read/Write tools always run (`Decision::Auto`).
//! - Outbound tools (sending a message to someone else) default to `Decision::Ask`
//!   unless the user has set a per-tool `trust_policies` row to `"auto"`.

use serde_json::Value;

use crate::db::{Approval, Db};
use crate::error::{Error, Result};
use crate::tools::{self, Risk};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    Auto,
    Ask,
}

/// Decide whether `tool_name` may run without asking the user.
pub fn decide(db: &Db, tool_name: &str) -> Result<Decision> {
    match tools::risk_of(tool_name) {
        Some(Risk::Read) | Some(Risk::Write) => Ok(Decision::Auto),
        Some(Risk::Outbound) => match db.get_trust_policy(tool_name)?.as_deref() {
            Some("auto") => Ok(Decision::Auto),
            _ => Ok(Decision::Ask),
        },
        None => Err(Error::Provider(format!("unknown tool: {tool_name}"))),
    }
}

/// File an approval request for a tool call: inserts the `approvals` row and a matching
/// notification, then returns the full inserted row.
pub fn request_approval(
    db: &Db,
    conversation_id: i64,
    tool: &str,
    args: &Value,
) -> Result<Approval> {
    let summary = tools::summarize_call(tool, args);
    let args_json = serde_json::to_string(args)?;
    let id = db.insert_approval(conversation_id, tool, &args_json, &summary)?;
    db.insert_notification("Approval needed", &summary, None, None)?;
    db.get_approval(id)?
        .ok_or_else(|| Error::Provider(format!("approval {id} vanished after insert")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn test_db() -> Db {
        let dir = std::env::temp_dir().join(format!("donna-trust-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        Db::open(&dir.join("t.sqlite")).unwrap()
    }

    fn rand_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
    }

    #[test]
    fn decide_by_risk_and_policy() {
        let db = test_db();
        assert!(matches!(decide(&db, "list_docs").unwrap(), Decision::Auto)); // Read
        assert!(matches!(decide(&db, "kb_save_node").unwrap(), Decision::Auto)); // Write
        assert!(matches!(decide(&db, "slack_send_message").unwrap(), Decision::Ask)); // Outbound default
        db.set_trust_policy("slack_send_message", "auto").unwrap();
        assert!(matches!(decide(&db, "slack_send_message").unwrap(), Decision::Auto));
        db.set_trust_policy("slack_send_message", "ask").unwrap();
        assert!(matches!(decide(&db, "slack_send_message").unwrap(), Decision::Ask));
        assert!(decide(&db, "nonexistent").is_err());
    }

    #[test]
    fn request_approval_creates_row_and_notification() {
        let db = test_db();
        let a = request_approval(
            &db,
            7,
            "whatsapp_send_message",
            &serde_json::json!({"to":"+15550100","text":"yo"}),
        )
        .unwrap();
        assert_eq!(a.status, "pending");
        assert!(a.summary.contains("+15550100"));
        assert!(db
            .list_notifications()
            .unwrap()
            .iter()
            .any(|n| n.title == "Approval needed"));
    }
}

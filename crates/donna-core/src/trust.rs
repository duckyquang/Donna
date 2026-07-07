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
/// notification, then returns the full inserted row. If an identical call
/// (conversation, tool, args) is already pending — a model that ignores
/// `PENDING_APPROVAL` and re-asks — returns the existing row instead of filing a
/// duplicate row/notification.
pub fn request_approval(
    db: &Db,
    conversation_id: i64,
    tool: &str,
    args: &Value,
) -> Result<Approval> {
    let args_json = serde_json::to_string(args)?;
    if let Some(existing) = db.find_pending_approval(conversation_id, tool, &args_json)? {
        return Ok(existing);
    }
    let summary = tools::summarize_call(tool, args);
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
        crate::secrets::init_test_file_store();
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

    #[test]
    fn request_approval_dedupes_identical_pending_calls() {
        let db = test_db();
        let args = serde_json::json!({"to":"+15550100","text":"yo"});

        let first = request_approval(&db, 7, "whatsapp_send_message", &args).unwrap();
        let second = request_approval(&db, 7, "whatsapp_send_message", &args).unwrap();

        assert_eq!(first.id, second.id, "identical re-ask must return the same row");
        assert_eq!(db.list_approvals(false).unwrap().len(), 1, "no duplicate row");
        assert_eq!(
            db.list_notifications().unwrap().len(),
            1,
            "no duplicate notification"
        );

        // A different args_json for the same tool/conversation still creates a new row.
        let different = serde_json::json!({"to":"+15550199","text":"yo"});
        let third = request_approval(&db, 7, "whatsapp_send_message", &different).unwrap();
        assert_ne!(third.id, first.id);
        assert_eq!(db.list_approvals(false).unwrap().len(), 2);
    }
}

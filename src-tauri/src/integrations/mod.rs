//! Integration connectors: how Donna reaches the user's external tools.
//!
//! Each connector owns its auth (OAuth tokens or API keys, stored in the OS keychain)
//! and the actions Donna can take. Phase 2 ships Google (Calendar), Slack, and Fathom.

pub mod fathom;
pub mod google;
pub mod slack;

use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Serialize)]
pub struct IntegrationStatus {
    pub id: String,
    pub name: String,
    pub connected: bool,
    /// Whether the integration still needs configuration before it can connect
    /// (e.g. Google needs OAuth client credentials first).
    pub needs_setup: bool,
}

/// Snapshot of every integration's connection state for the Integrations Hub.
pub fn status() -> Result<Vec<IntegrationStatus>> {
    Ok(vec![
        IntegrationStatus {
            id: "google".into(),
            name: "Google Workspace".into(),
            connected: google::is_connected()?,
            needs_setup: !google::has_client()?,
        },
        IntegrationStatus {
            id: "slack".into(),
            name: "Slack".into(),
            connected: slack::is_connected()?,
            needs_setup: false,
        },
        IntegrationStatus {
            id: "fathom".into(),
            name: "Fathom".into(),
            connected: fathom::is_connected()?,
            needs_setup: false,
        },
    ])
}

//! Integration connectors: how Donna reaches the user's external tools.
//!
//! Each connector owns its auth (OAuth tokens or API keys, stored in the OS keychain)
//! and the actions Donna can take.

pub mod discord;
pub mod fathom;
pub mod github;
pub mod google;
pub mod linear;
pub mod notion;
pub mod slack;
pub mod telegram;
pub mod whatsapp;

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
        IntegrationStatus {
            id: "github".into(),
            name: "GitHub".into(),
            connected: github::is_connected()?,
            needs_setup: false,
        },
        IntegrationStatus {
            id: "linear".into(),
            name: "Linear".into(),
            connected: linear::is_connected()?,
            needs_setup: false,
        },
        IntegrationStatus {
            id: "notion".into(),
            name: "Notion".into(),
            connected: notion::is_connected()?,
            needs_setup: false,
        },
        IntegrationStatus {
            id: "telegram".into(),
            name: "Telegram".into(),
            connected: telegram::is_connected()?,
            needs_setup: false,
        },
        IntegrationStatus {
            id: "whatsapp".into(),
            name: "WhatsApp".into(),
            connected: whatsapp::is_connected()?,
            needs_setup: false,
        },
        IntegrationStatus {
            id: "discord".into(),
            name: "Discord".into(),
            connected: discord::is_connected().unwrap_or(false),
            needs_setup: false,
        },
    ])
}

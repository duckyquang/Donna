//! Google Workspace connector.
//!
//! Uses the user's own OAuth client (Desktop app type) so this stays a bring-your-own
//! credentials, local-first integration. Connect runs the loopback OAuth flow; tokens
//! are stored in the keychain and auto-refreshed. Phase 2 implements Google Calendar
//! two-way sync; the granted scopes also cover Gmail/Docs/Drive for later phases.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::oauth;
use crate::secrets;

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const CALENDAR_BASE: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const GMAIL_BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const DOCS_BASE: &str = "https://docs.googleapis.com/v1/documents";

const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/calendar",
    "https://www.googleapis.com/auth/gmail.modify",
    "https://www.googleapis.com/auth/documents",
    "https://www.googleapis.com/auth/drive.file",
    "openid",
    "email",
];

const CLIENT_KEY: &str = "google_client";
const TOKEN_KEY: &str = "oauth:google";

#[derive(Debug, Serialize, Deserialize)]
struct ClientCreds {
    client_id: String,
    client_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredToken {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: i64,
    scope: Option<String>,
}

// --- Client credentials ----------------------------------------------------

pub fn set_client(client_id: &str, client_secret: &str) -> Result<()> {
    let creds = ClientCreds {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    };
    secrets::set_secret(CLIENT_KEY, &serde_json::to_string(&creds)?)
}

pub fn has_client() -> Result<bool> {
    secrets::has_secret(CLIENT_KEY)
}

fn get_client() -> Result<ClientCreds> {
    let raw = secrets::get_secret(CLIENT_KEY)?
        .ok_or_else(|| Error::Provider("Google OAuth client not configured".into()))?;
    Ok(serde_json::from_str(&raw)?)
}

// --- Connection lifecycle --------------------------------------------------

pub fn is_connected() -> Result<bool> {
    secrets::has_secret(TOKEN_KEY)
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(TOKEN_KEY)
}

pub async fn connect() -> Result<()> {
    let creds = get_client()?;
    let (code, redirect_uri, verifier) =
        oauth::authorize(AUTH_ENDPOINT, &creds.client_id, SCOPES).await?;
    let token = oauth::exchange_code(
        TOKEN_ENDPOINT,
        &creds.client_id,
        &creds.client_secret,
        &code,
        &redirect_uri,
        &verifier,
    )
    .await?;

    let stored = StoredToken {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at: now() + token.expires_in.max(0),
        scope: token.scope,
    };
    secrets::set_secret(TOKEN_KEY, &serde_json::to_string(&stored)?)?;
    Ok(())
}

/// Return a valid access token, refreshing it if it has expired.
async fn access_token() -> Result<String> {
    let raw = secrets::get_secret(TOKEN_KEY)?
        .ok_or_else(|| Error::Provider("Google is not connected".into()))?;
    let mut token: StoredToken = serde_json::from_str(&raw)?;

    if token.expires_at > now() + 60 {
        return Ok(token.access_token);
    }

    let refresh = token
        .refresh_token
        .clone()
        .ok_or_else(|| Error::Provider("Google session expired; please reconnect".into()))?;
    let creds = get_client()?;
    let refreshed = oauth::refresh(
        TOKEN_ENDPOINT,
        &creds.client_id,
        &creds.client_secret,
        &refresh,
    )
    .await?;

    token.access_token = refreshed.access_token;
    token.expires_at = now() + refreshed.expires_in.max(0);
    if refreshed.refresh_token.is_some() {
        token.refresh_token = refreshed.refresh_token;
    }
    secrets::set_secret(TOKEN_KEY, &serde_json::to_string(&token)?)?;
    Ok(token.access_token)
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

// --- Calendar --------------------------------------------------------------

/// A calendar event in Donna's simplified shape. `start`/`end` are RFC3339 timestamps.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalendarEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub html_link: Option<String>,
}

fn parse_event(v: &serde_json::Value) -> Option<CalendarEvent> {
    let start = v
        .get("start")
        .and_then(|s| s.get("dateTime").or_else(|| s.get("date")))
        .and_then(|d| d.as_str())?
        .to_string();
    let end = v
        .get("end")
        .and_then(|s| s.get("dateTime").or_else(|| s.get("date")))
        .and_then(|d| d.as_str())
        .unwrap_or(&start)
        .to_string();
    Some(CalendarEvent {
        id: v.get("id").and_then(|x| x.as_str()).map(String::from),
        summary: v.get("summary").and_then(|x| x.as_str()).map(String::from),
        description: v
            .get("description")
            .and_then(|x| x.as_str())
            .map(String::from),
        start,
        end,
        html_link: v.get("htmlLink").and_then(|x| x.as_str()).map(String::from),
    })
}

fn event_body(ev: &CalendarEvent) -> serde_json::Value {
    serde_json::json!({
        "summary": ev.summary,
        "description": ev.description,
        "start": { "dateTime": ev.start },
        "end": { "dateTime": ev.end },
    })
}

pub async fn list_events(time_min: &str, time_max: &str) -> Result<Vec<CalendarEvent>> {
    let token = access_token().await?;
    let resp = reqwest::Client::new()
        .get(CALENDAR_BASE)
        .bearer_auth(&token)
        .query(&[
            ("timeMin", time_min),
            ("timeMax", time_max),
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("maxResults", "250"),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Google Calendar error ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    let items = body
        .get("items")
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items.iter().filter_map(parse_event).collect())
}

pub async fn create_event(ev: &CalendarEvent) -> Result<CalendarEvent> {
    let token = access_token().await?;
    let resp = reqwest::Client::new()
        .post(CALENDAR_BASE)
        .bearer_auth(&token)
        .json(&event_body(ev))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Could not create event ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    parse_event(&body).ok_or_else(|| Error::Provider("unexpected calendar response".into()))
}

pub async fn update_event(id: &str, ev: &CalendarEvent) -> Result<CalendarEvent> {
    let token = access_token().await?;
    let resp = reqwest::Client::new()
        .patch(format!("{CALENDAR_BASE}/{id}"))
        .bearer_auth(&token)
        .json(&event_body(ev))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Could not update event ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    parse_event(&body).ok_or_else(|| Error::Provider("unexpected calendar response".into()))
}

pub async fn delete_event(id: &str) -> Result<()> {
    let token = access_token().await?;
    let resp = reqwest::Client::new()
        .delete(format!("{CALENDAR_BASE}/{id}"))
        .bearer_auth(&token)
        .send()
        .await?;
    if !resp.status().is_success() && resp.status().as_u16() != 410 {
        return Err(Error::Provider(format!(
            "Could not delete event ({})",
            resp.status()
        )));
    }
    Ok(())
}

// --- Gmail -----------------------------------------------------------------

/// A Gmail message in Donna's simplified shape.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GmailMessage {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub snippet: Option<String>,
}

fn header_value(headers: &[serde_json::Value], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|h| h.get("name").and_then(|n| n.as_str()) == Some(name))
        .and_then(|h| h.get("value").and_then(|v| v.as_str()))
        .map(String::from)
}

pub async fn list_gmail_messages(max_results: u32) -> Result<Vec<GmailMessage>> {
    let token = access_token().await?;
    let client = reqwest::Client::new();
    let list_resp = client
        .get(format!("{GMAIL_BASE}/messages"))
        .bearer_auth(&token)
        .query(&[("maxResults", max_results.to_string())])
        .send()
        .await?;
    if !list_resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Gmail list error ({})",
            list_resp.status()
        )));
    }
    let body: serde_json::Value = list_resp.json().await?;
    let ids: Vec<String> = body
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for id in ids {
        let msg_resp = client
            .get(format!("{GMAIL_BASE}/messages/{id}"))
            .bearer_auth(&token)
            .query(&[
                ("format", "metadata".to_string()),
                ("metadataHeaders", "Subject".to_string()),
                ("metadataHeaders", "From".to_string()),
            ])
            .send()
            .await?;
        if !msg_resp.status().is_success() {
            continue;
        }
        let msg: serde_json::Value = msg_resp.json().await?;
        let headers = msg
            .get("payload")
            .and_then(|p| p.get("headers"))
            .and_then(|h| h.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);
        out.push(GmailMessage {
            id: id.clone(),
            subject: header_value(headers, "Subject"),
            from: header_value(headers, "From"),
            snippet: msg.get("snippet").and_then(|s| s.as_str()).map(String::from),
        });
    }
    Ok(out)
}

// --- Google Docs -----------------------------------------------------------

/// Create an empty Google Doc and return its document id.
pub async fn create_google_doc(title: &str) -> Result<String> {
    let token = access_token().await?;
    let resp = reqwest::Client::new()
        .post(DOCS_BASE)
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": title }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Provider(format!(
            "Google Docs error ({})",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("documentId")
        .and_then(|id| id.as_str())
        .map(String::from)
        .ok_or_else(|| Error::Provider("unexpected Google Docs response".into()))
}

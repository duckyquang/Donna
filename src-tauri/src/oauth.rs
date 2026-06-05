//! Generic OAuth 2.0 Authorization Code + PKCE flow for native/desktop apps.
//!
//! Uses a loopback redirect (`http://127.0.0.1:<port>`) per Google's recommended flow
//! for installed apps: we spin up a one-shot local HTTP listener, open the system
//! browser to the consent screen, capture the redirect, and exchange the code for
//! tokens. Refresh tokens are persisted by the caller (in the OS keychain).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use base64::Engine;
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
        Pkce { verifier, challenge }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

fn random_state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

/// Run the interactive authorization step. Opens the browser and blocks (with a timeout)
/// until the user approves and the loopback redirect delivers a code.
///
/// Returns `(code, redirect_uri, pkce_verifier)`.
pub async fn authorize(
    auth_endpoint: &str,
    client_id: &str,
    scopes: &[&str],
) -> Result<(String, String, String)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| Error::Provider(format!("could not start local auth server: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| Error::Provider(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let pkce = Pkce::generate();
    let state = random_state();

    let mut url = url::Url::parse(auth_endpoint).map_err(|e| Error::Provider(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &scopes.join(" "))
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", &state);

    open::that(url.as_str())
        .map_err(|e| Error::Provider(format!("could not open browser: {e}")))?;

    let expected_state = state.clone();
    let join = tokio::task::spawn_blocking(move || wait_for_code(listener, &expected_state));
    let code = tokio::time::timeout(Duration::from_secs(300), join)
        .await
        .map_err(|_| Error::Provider("authorization timed out".into()))?
        .map_err(|e| Error::Provider(e.to_string()))??;

    Ok((code, redirect_uri, pkce.verifier))
}

/// Accept loopback connections until one carries the `code` (validating `state`).
fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String> {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or("");
        let path = first_line.split_whitespace().nth(1).unwrap_or("");

        let full = format!("http://127.0.0.1{path}");
        let parsed = url::Url::parse(&full).ok();
        let (mut code, mut state, mut err) = (None, None, None);
        if let Some(u) = parsed {
            for (k, v) in u.query_pairs() {
                match k.as_ref() {
                    "code" => code = Some(v.into_owned()),
                    "state" => state = Some(v.into_owned()),
                    "error" => err = Some(v.into_owned()),
                    _ => {}
                }
            }
        }

        if err.is_some() || code.is_some() {
            let body = "<html><body style=\"font-family:sans-serif;background:#0b0b0f;color:#eee;text-align:center;padding-top:80px\"><h2>Donna is connected.</h2><p>You can close this tab and return to the app.</p></body></html>";
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            );
            if let Some(e) = err {
                return Err(Error::Provider(format!("authorization denied: {e}")));
            }
            if state.as_deref() != Some(expected_state) {
                return Err(Error::Provider("state mismatch (possible CSRF)".into()));
            }
            return code.ok_or_else(|| Error::Provider("no code in redirect".into()));
        }

        // Unrelated request (e.g. favicon) — answer and keep waiting.
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
    }
    Err(Error::Provider("auth server closed unexpectedly".into()))
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code(
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    verifier: &str,
) -> Result<TokenResponse> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ];
    post_token(token_endpoint, &params).await
}

/// Use a refresh token to obtain a fresh access token.
pub async fn refresh(
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenResponse> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];
    post_token(token_endpoint, &params).await
}

async fn post_token(endpoint: &str, params: &[(&str, &str)]) -> Result<TokenResponse> {
    let resp = reqwest::Client::new()
        .post(endpoint)
        .form(params)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(Error::Provider(format!("token request failed ({status}): {detail}")));
    }
    Ok(resp.json().await?)
}

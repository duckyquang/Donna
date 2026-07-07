//! donna-server library — axum app assembly.
//!
//! Kept as a lib (with a thin main.rs binary) so integration tests can build the app
//! in-process via `build_app`/`test_state` without binding a real socket.

pub mod auth;
pub mod rpc;
pub mod state;
pub mod voice;
pub mod webhook;
pub mod ws;

use axum::{routing::{get, post}, Router};
pub use state::{AppState, test_state};

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/rpc/:command", post(rpc::handle))
        .route("/ws", get(ws::handle))
        .route("/voice/transcribe", post(voice::transcribe))
        .route("/voice/speak", post(voice::speak))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::require_bearer))
        // Registered AFTER the auth layer, so these are unauthenticated: Meta authenticates
        // the GET with a verify token and the POST with an HMAC signature, not our bearer.
        .route("/health", get(|| async { "ok" }))
        .route("/webhook/whatsapp", get(webhook::verify).post(webhook::receive))
        .with_state(state)
}

//! donna-server library — axum app assembly.
//!
//! Kept as a lib (with a thin main.rs binary) so integration tests can build the app
//! in-process via `build_app`/`test_state` without binding a real socket.

pub mod auth;
pub mod rpc;
pub mod state;
pub mod ws;

use axum::{routing::{get, post}, Router};
pub use state::{AppState, test_state};

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/rpc/:command", post(rpc::handle))
        .route("/ws", get(ws::handle))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::require_bearer))
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
}

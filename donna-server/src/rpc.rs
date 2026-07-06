//! JSON-RPC-ish command dispatch: `POST /rpc/:command` with a JSON body.
//!
//! ponytail: stub for Task 7 — always returns `{"ok":true}`. Task 8 wires this into
//! `donna_core::ops` per-command dispatch.

use axum::{extract::{Path, State}, Json};
use serde_json::{json, Value};
use crate::state::AppState;

pub async fn handle(
    State(_st): State<AppState>,
    Path(_command): Path<String>,
    _body: Json<Value>,
) -> Json<Value> {
    Json(json!({ "ok": true }))
}

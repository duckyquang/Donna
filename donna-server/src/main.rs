//! donna-server — always-on headless companion for Donna.
//!
//! Runs on any Linux machine (VPS, Raspberry Pi, Docker). All config via environment
//! variables (see README): `DONNA_DATA_DIR`, `DONNA_TOKEN` (required), `DONNA_PORT`.

use donna_server::{build_app, AppState};
use donna_core::{db::Db, secrets};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let data_dir = std::path::PathBuf::from(std::env::var("DONNA_DATA_DIR").unwrap_or("./donna-data".into()));
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    std::env::set_var("DONNA_KB_DIR", data_dir.join("knowledge-base"));
    let _ = donna_core::knowledge::ensure_root();
    secrets::init(Box::new(secrets::FileStore::new(data_dir.join("secrets.json"))));
    let token = std::env::var("DONNA_TOKEN").expect("DONNA_TOKEN is required");
    let port: u16 = std::env::var("DONNA_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8377);
    let db = Arc::new(Db::open(&data_dir.join("donna.sqlite")).expect("open db"));
    let (events, _) = tokio::sync::broadcast::channel(256);
    let state = AppState { db, token, events };
    // Run scheduled routines; due ones fire notify() → broadcast to WS clients.
    donna_core::scheduler::run_loop(state.db.clone(), Arc::new(state.clone()));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("donna-server listening on :{port}");
    axum::serve(listener, build_app(state)).await.unwrap();
}

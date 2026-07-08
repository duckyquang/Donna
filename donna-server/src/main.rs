//! donna-server — always-on headless companion for Donna.
//!
//! Runs on any Linux machine (VPS, Raspberry Pi, Docker). All config via environment
//! variables (see README): `DONNA_DATA_DIR`, `DONNA_TOKEN` (required), `DONNA_PORT`.

use donna_server::{build_app, AppState};
use donna_core::{bundle, db::Db, secrets};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let data_dir = std::path::PathBuf::from(std::env::var("DONNA_DATA_DIR").unwrap_or("./donna-data".into()));
    std::fs::create_dir_all(&data_dir).expect("create data dir");

    // ponytail: two subcommands don't need a CLI framework.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "import" {
        let db_path = data_dir.join("donna.sqlite");
        if db_path.exists() {
            eprintln!("data dir not empty, aborting");
            std::process::exit(1);
        }
        bundle::import_bundle(std::path::Path::new(&args[2]), &data_dir).expect("import bundle");
        println!("imported {} into {}", args[2], data_dir.display());
        return;
    }

    std::env::set_var("DONNA_KB_DIR", data_dir.join("knowledge-base"));
    let _ = donna_core::knowledge::ensure_root();
    std::env::set_var("DONNA_SKILLS_DIR", data_dir.join("skills"));
    let _ = donna_core::skills::ensure_root();
    secrets::init(Box::new(secrets::FileStore::new(data_dir.join("secrets.json"))));
    let token = std::env::var("DONNA_TOKEN").expect("DONNA_TOKEN is required");
    let port: u16 = std::env::var("DONNA_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8377);
    let bind = std::env::var("DONNA_BIND").unwrap_or("0.0.0.0".into());
    let db = Arc::new(Db::open(&data_dir.join("donna.sqlite")).expect("open db"));
    let (events, _) = tokio::sync::broadcast::channel(256);
    let wa_verify_token = std::env::var("DONNA_WA_VERIFY_TOKEN").ok();
    let wa_app_secret = std::env::var("DONNA_WA_APP_SECRET").ok();
    let state = AppState { db, token, events, wa_verify_token, wa_app_secret };
    // Run scheduled routines; due ones fire notify() → broadcast to WS clients.
    donna_core::scheduler::run_loop(state.db.clone(), Arc::new(state.clone()));
    let listener = tokio::net::TcpListener::bind((bind.as_str(), port)).await.unwrap();
    println!("donna-server listening on {bind}:{port}");
    axum::serve(listener, build_app(state)).await.unwrap();
}

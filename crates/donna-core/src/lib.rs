//! Donna's portable brain: DB, knowledge base, providers, integrations.
//! Consumed by the Tauri desktop app and donna-server.

pub mod agent;
pub mod bundle;
pub mod db;
pub mod docs;
pub mod embeddings;
pub mod error;
pub mod integrations;
pub mod knowledge;
pub mod oauth;
pub mod ops;
pub mod providers;
pub mod retrieval;
pub mod review;
pub mod scheduler;
pub mod tools;
pub mod secrets;
pub mod trust;

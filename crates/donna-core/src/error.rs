//! Unified error type for Donna commands.
//!
//! Implements `serde::Serialize` so errors can cross the Tauri IPC boundary as plain
//! strings the frontend can display.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("no API key found for provider '{0}'")]
    MissingApiKey(String),

    #[error("provider '{0}' is not supported yet")]
    UnsupportedProvider(String),

    #[error("{0}")]
    Provider(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

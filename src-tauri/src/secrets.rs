//! Secure storage for provider API keys using the operating system keychain.
//!
//! Keys are NEVER written to the database, logs, or disk in plaintext. Each provider's
//! key is stored under a stable service/account pair in the OS keychain.

use keyring::Entry;

use crate::error::{Error, Result};

const SERVICE: &str = "ai.donna.app";

fn entry(key: &str) -> Result<Entry> {
    Entry::new(SERVICE, key).map_err(Error::from)
}

/// Store (or replace) an arbitrary secret under a stable key.
pub fn set_secret(key: &str, value: &str) -> Result<()> {
    entry(key)?.set_password(value)?;
    Ok(())
}

/// Retrieve a secret, if one is stored under `key`.
pub fn get_secret(key: &str) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::from(e)),
    }
}

/// Whether a secret exists under `key`.
pub fn has_secret(key: &str) -> Result<bool> {
    Ok(get_secret(key)?.is_some())
}

/// Remove a secret.
pub fn delete_secret(key: &str) -> Result<()> {
    match entry(key)?.delete_password() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::from(e)),
    }
}

// --- Convenience wrappers for model-provider API keys ----------------------

pub fn set_api_key(provider: &str, key: &str) -> Result<()> {
    set_secret(&format!("api_key:{provider}"), key)
}

pub fn get_api_key(provider: &str) -> Result<Option<String>> {
    get_secret(&format!("api_key:{provider}"))
}

pub fn has_api_key(provider: &str) -> Result<bool> {
    has_secret(&format!("api_key:{provider}"))
}

pub fn delete_api_key(provider: &str) -> Result<()> {
    delete_secret(&format!("api_key:{provider}"))
}

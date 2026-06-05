//! Secure storage for provider API keys using the operating system keychain.
//!
//! Keys are NEVER written to the database, logs, or disk in plaintext. Each provider's
//! key is stored under a stable service/account pair in the OS keychain.

use keyring::Entry;

use crate::error::{Error, Result};

const SERVICE: &str = "ai.donna.app";

fn entry(provider: &str) -> Result<Entry> {
    Entry::new(SERVICE, &format!("api_key:{provider}")).map_err(Error::from)
}

/// Store (or replace) the API key for a provider.
pub fn set_api_key(provider: &str, key: &str) -> Result<()> {
    entry(provider)?.set_password(key)?;
    Ok(())
}

/// Retrieve the API key for a provider, if one is stored.
pub fn get_api_key(provider: &str) -> Result<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::from(e)),
    }
}

/// Whether a key exists for a provider (without returning it).
pub fn has_api_key(provider: &str) -> Result<bool> {
    Ok(get_api_key(provider)?.is_some())
}

/// Remove a provider's stored key.
pub fn delete_api_key(provider: &str) -> Result<()> {
    match entry(provider)?.delete_password() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::from(e)),
    }
}

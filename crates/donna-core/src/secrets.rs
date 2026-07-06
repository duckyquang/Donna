//! Secure storage for provider API keys.
//!
//! Keys are NEVER written to the database, logs, or plaintext files by default. On
//! desktop, secrets live in the OS keychain (`KeychainStore`). Environments without a
//! keychain (e.g. the headless server) can `init()` a `FileStore` instead — a
//! 0600-permissioned JSON file written atomically. Callers always go through the
//! module-level `*_secret`/`*_api_key` functions below, which delegate to whichever
//! store is active.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use keyring::Entry;

use crate::error::{Error, Result};

const SERVICE: &str = "ai.donna.app";

/// Pluggable backend for secret storage. `get` returns `Ok(None)` for a missing key
/// rather than an error; `delete` is idempotent (missing key is not an error).
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

/// Stores secrets in the OS keychain. This is the default backend on desktop.
pub struct KeychainStore;

impl KeychainStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KeychainStore {
    fn default() -> Self {
        Self::new()
    }
}

fn entry(key: &str) -> Result<Entry> {
    Entry::new(SERVICE, key).map_err(Error::from)
}

impl SecretStore for KeychainStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::from(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        entry(key)?.set_password(value)?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        match entry(key)?.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
    }
}

/// Stores secrets as a JSON map in a single file, written atomically (tmp + rename)
/// with 0600 permissions on unix. For environments without an OS keychain (the server).
pub struct FileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn read_map(&self) -> Result<BTreeMap<String, String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn write_map(&self, map: &BTreeMap<String, String>) -> Result<()> {
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl SecretStore for FileStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        let _g = self.lock.lock().unwrap();
        Ok(self.read_map()?.get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        let _g = self.lock.lock().unwrap();
        let mut m = self.read_map()?;
        m.insert(key.into(), value.into());
        self.write_map(&m)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let _g = self.lock.lock().unwrap();
        let mut m = self.read_map()?;
        m.remove(key);
        self.write_map(&m)
    }
}

static STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();

/// Install the store backend to use for the rest of the process lifetime. Must be
/// called before the first secret access; later calls are ignored (first wins). If
/// never called, the store defaults to `KeychainStore` — desktop behavior is
/// unchanged.
pub fn init(store: Box<dyn SecretStore>) {
    let _ = STORE.set(store);
}

fn store() -> &'static dyn SecretStore {
    STORE.get_or_init(|| Box::new(KeychainStore::new())).as_ref()
}

/// Store (or replace) an arbitrary secret under a stable key.
pub fn set_secret(key: &str, value: &str) -> Result<()> {
    store().set(key, value)
}

/// Retrieve a secret, if one is stored under `key`.
pub fn get_secret(key: &str) -> Result<Option<String>> {
    store().get(key)
}

/// Whether a secret exists under `key`.
pub fn has_secret(key: &str) -> Result<bool> {
    Ok(get_secret(key)?.is_some())
}

/// Remove a secret.
pub fn delete_secret(key: &str) -> Result<()> {
    store().delete(key)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_roundtrip() {
        let dir = std::env::temp_dir().join(format!("donna-secrets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = FileStore::new(dir.join("secrets.json"));
        assert_eq!(store.get("api_key").unwrap(), None);
        store.set("api_key", "sk-123").unwrap();
        assert_eq!(store.get("api_key").unwrap(), Some("sk-123".into()));
        store.delete("api_key").unwrap();
        assert_eq!(store.get("api_key").unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn file_store_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("donna-perm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secrets.json");
        FileStore::new(path.clone()).set("k", "v").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

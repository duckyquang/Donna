//! Fathom connector (API key based).
//!
//! Stores the user's Fathom API key in the keychain so Donna can pull meeting recaps
//! and transcripts. The doc-generation actions that consume Fathom data are built with
//! the Docs feature (Phase 3); Phase 2 establishes the secure connection.

use crate::error::Result;
use crate::secrets;

const KEY: &str = "api_key:fathom";

pub fn set_key(key: &str) -> Result<()> {
    secrets::set_secret(KEY, key)
}

pub fn is_connected() -> Result<bool> {
    secrets::has_secret(KEY)
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(KEY)
}

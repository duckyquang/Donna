//! Discord bot integration — Donna as a bot in any server/channel.
//!
//! Provides token storage and status checking. A full live event loop (serenity
//! Client) can be added later; for now we just persist the token so the
//! Integrations Hub can show a "connected" state.

use crate::error::Result;
use crate::secrets;

const SECRET_KEY: &str = "discord_bot_token";

pub fn is_connected() -> Result<bool> {
    secrets::has_secret(SECRET_KEY)
}

pub fn set_token(token: &str) -> Result<()> {
    secrets::set_secret(SECRET_KEY, token)?;
    Ok(())
}

pub fn disconnect() -> Result<()> {
    secrets::delete_secret(SECRET_KEY)?;
    Ok(())
}

pub fn get_token() -> Result<Option<String>> {
    secrets::get_secret(SECRET_KEY)
}

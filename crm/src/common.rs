//! Helpers shared by the parts of the plugin.

pub use aether_sdk::records::{id_of, require, text, Record};

use aether_sdk::prelude::*;

pub const ADMIN_ROLE: &str = "crm.crm_admin";
pub const READER_ROLE: &str = "crm.crm_reader";

pub fn require_admin() -> Result<()> {
    context::current()?.require_role(ADMIN_ROLE)
}

pub fn require_reader() -> Result<()> {
    let context = context::current()?;
    if context.has_role(READER_ROLE) || context.has_role(ADMIN_ROLE) {
        Ok(())
    } else {
        Err(Error::msg(format!("you need the role `{READER_ROLE}` to do this")))
    }
}

pub const STAGE_KINDS: &[&str] = &["open", "hold", "won", "lost", "converted"];

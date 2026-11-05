//! Helpers shared by the parts of the plugin.

pub use aether_sdk::records::{id_of, require, text, Record};

use aether_sdk::prelude::*;

pub const USER_ROLE: &str = "crm_campaign.crm_user";
pub const ADMIN_ROLE: &str = "crm_campaign.crm_admin";

pub fn require_user() -> Result<()> {
    let context = context::current()?;
    if context.has_role(USER_ROLE) || context.has_role(ADMIN_ROLE) {
        Ok(())
    } else {
        Err(Error::msg(format!("you need the role `{USER_ROLE}` to do this")))
    }
}

pub fn require_admin() -> Result<()> {
    let context = context::current()?;
    if context.has_role(ADMIN_ROLE) {
        Ok(())
    } else {
        Err(Error::msg(format!("you need the role `{ADMIN_ROLE}` to do this")))
    }
}

pub const CHANNELS: &[&str] = &["email", "web", "ads", "event", "other"];
pub const MEMBER_KINDS: &[&str] = &["party", "lead", "deal"];

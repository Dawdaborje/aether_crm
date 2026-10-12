//! Campaigns and membership for UTM attribution.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_admin, require_user, text, Record, CHANNELS, MEMBER_KINDS};

#[derive(Deserialize)]
struct CreateCampaign {
    code: String,
    name: String,
    channel: String,
    #[serde(default)]
    starts_on: Option<String>,
    #[serde(default)]
    ends_on: Option<String>,
    #[serde(default)]
    utm_campaign: Option<String>,
}

#[derive(Deserialize)]
struct SetCampaign {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    starts_on: Option<String>,
    #[serde(default)]
    ends_on: Option<String>,
    #[serde(default)]
    utm_campaign: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct AddMember {
    campaign: String,
    subject_kind: String,
    subject_id: String,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    active_only: Option<bool>,
}

#[derive(Deserialize)]
struct ListMembers {
    campaign: String,
}

fn one_of(value: &str, allowed: &[&str], label: &str) -> Result<String> {
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(Error::msg(format!("{label} is {}", allowed.join(", "))))
    }
}

fn assert_member(kind: &str, id: &str) -> Result<()> {
    match kind {
        "party" => {
            require("party.party", id, "party")?;
        }
        "lead" => {
            require("crm_lead.lead", id, "lead")?;
        }
        "deal" => {
            require("crm_deal.deal", id, "deal")?;
        }
        _ => return Err(Error::msg("subject_kind is party, lead or deal")),
    }
    Ok(())
}

fn do_create_campaign(input: CreateCampaign) -> Result<Record> {
    require_admin()?;
    let code = input.code.trim();
    let name = input.name.trim();
    if code.is_empty() || name.is_empty() {
        return Err(Error::msg("code and name are required"));
    }
    let mut data = json!({
        "code": code,
        "name": name,
        "channel": one_of(input.channel.as_str(), CHANNELS, "channel")?,
        "utm_campaign": input.utm_campaign.unwrap_or_else(|| code.to_string()).trim(),
        "active": true,
    });
    if let Some(d) = input.starts_on.filter(|d| !d.is_empty()) {
        data["starts_on"] = json!(d);
    }
    if let Some(d) = input.ends_on.filter(|d| !d.is_empty()) {
        data["ends_on"] = json!(d);
    }
    db::create("campaign", &data).map_err(|e| e.or("could not create the campaign"))
}

fn do_set_campaign(input: SetCampaign) -> Result<Record> {
    require_admin()?;
    let _ = require("campaign", &input.id, "campaign")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::msg("name is required"));
        }
        changes["name"] = json!(name);
    }
    if let Some(channel) = input.channel {
        changes["channel"] = json!(one_of(&channel, CHANNELS, "channel")?);
    }
    if let Some(d) = input.starts_on {
        changes["starts_on"] = if d.is_empty() { Value::Null } else { json!(d) };
    }
    if let Some(d) = input.ends_on {
        changes["ends_on"] = if d.is_empty() { Value::Null } else { json!(d) };
    }
    if let Some(utm) = input.utm_campaign {
        changes["utm_campaign"] = json!(utm.trim());
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("campaign", &input.id, &changes)?.ok_or_else(|| Error::msg("the campaign is gone"))
}

fn do_add_member(input: AddMember) -> Result<Record> {
    require_user()?;
    let campaign = require("campaign", &input.campaign, "campaign")?;
    let kind = one_of(input.subject_kind.as_str(), MEMBER_KINDS, "subject_kind")?;
    let subject_id = input.subject_id.trim();
    if subject_id.is_empty() {
        return Err(Error::msg("subject_id is required"));
    }
    assert_member(&kind, subject_id)?;
    let existing = db::find::<Record>("campaign_member")
        .filter("campaign", id_of(&campaign)?)
        .limit(500)
        .all()?;
    if let Some(found) = existing.into_iter().find(|row| {
        text(row, "subject_kind") == Some(kind.as_str()) && text(row, "subject_id") == Some(subject_id)
    }) {
        return Ok(found);
    }
    let ctx = context::current()?;
    let member = db::create(
        "campaign_member",
        &json!({
            "campaign": id_of(&campaign)?,
            "subject_kind": kind,
            "subject_id": subject_id,
            "joined_at": ctx.now,
        }),
    )?;
    // Attribute UTM onto lead/deal when blank (via owning plugins).
    if let Some(utm) = text(&campaign, "utm_campaign") {
        if kind == "lead" {
            let _: Result<Value> = plugins::call("crm_lead", "set_lead", &json!({ "id": subject_id, "utm_campaign": utm }));
        } else if kind == "deal" {
            // set_deal has no utm fields today — membership is the source of truth.
            let _ = utm;
        }
    }
    Ok(member)
}

fn do_get_campaign(input: Id) -> Result<Record> {
    require_user()?;
    require("campaign", &input.id, "campaign")
}

fn do_search_campaigns(input: Search) -> Result<Vec<Record>> {
    require_user()?;
    let mut rows = db::find::<Record>("campaign").limit(200).all()?;
    if input.active_only.unwrap_or(true) {
        rows.retain(|r| r.get("active") != Some(&json!(false)));
    }
    if let Some(needle) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let needle = needle.to_lowercase();
        rows.retain(|r| {
            ["code", "name", "utm_campaign"].iter().any(|f| {
                text(r, f).map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
            })
        });
    }
    Ok(rows)
}

fn do_list_members(input: ListMembers) -> Result<Vec<Record>> {
    require_user()?;
    let _ = require("campaign", &input.campaign, "campaign")?;
    db::find::<Record>("campaign_member").filter("campaign", &input.campaign).limit(500).all()
}

handler! {
    fn create_campaign(input: CreateCampaign) -> Record { do_create_campaign(input) }
    fn set_campaign(input: SetCampaign) -> Record { do_set_campaign(input) }
    fn add_member(input: AddMember) -> Record { do_add_member(input) }
    fn get_campaign(input: Id) -> Record { do_get_campaign(input) }
    fn search_campaigns(input: Search) -> Vec<Record> { do_search_campaigns(input) }
    fn list_members(input: ListMembers) -> Vec<Record> { do_list_members(input) }
}

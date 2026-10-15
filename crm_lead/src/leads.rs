//! Leads: create, stage moves, lose, and idempotent convert to deal.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record};
use crate::convert_mode::{parse_convert_mode, ConvertMode};

#[derive(Deserialize)]
struct CreateLead {
    company: String,
    title: String,
    stage: String,
    #[serde(default)]
    contact_party: Option<String>,
    #[serde(default)]
    org_party: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    person_name: Option<String>,
    #[serde(default)]
    org_name: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    industry: Option<String>,
    #[serde(default)]
    territory: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    utm_source: Option<String>,
    #[serde(default)]
    utm_medium: Option<String>,
    #[serde(default)]
    utm_campaign: Option<String>,
    #[serde(default)]
    score: Option<i64>,
}

#[derive(Deserialize)]
struct SetLead {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    contact_party: Option<String>,
    #[serde(default)]
    org_party: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    person_name: Option<String>,
    #[serde(default)]
    org_name: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    industry: Option<String>,
    #[serde(default)]
    territory: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    score: Option<i64>,
    #[serde(default)]
    utm_source: Option<String>,
    #[serde(default)]
    utm_medium: Option<String>,
    #[serde(default)]
    utm_campaign: Option<String>,
}

#[derive(Deserialize)]
struct SetStage {
    id: String,
    stage: String,
}

#[derive(Deserialize)]
struct LoseLead {
    id: String,
    lost_reason: String,
    #[serde(default)]
    lost_notes: Option<String>,
}

#[derive(Deserialize)]
struct ConvertInput {
    lead: String,
    /// `return_existing` (default) | `create` | `fail`
    #[serde(default)]
    if_converted: Option<String>,
    #[serde(default)]
    create_another: Option<bool>,
    #[serde(default)]
    contact_party: Option<String>,
    #[serde(default)]
    org_party: Option<String>,
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
    company: Option<String>,
    #[serde(default)]
    active_only: Option<bool>,
}

fn stage_kind(stage: &Record) -> &str {
    text(stage, "kind").unwrap_or("open")
}

fn refuse_if_converted(lead: &Record) -> Result<()> {
    if text(lead, "converted_deal").is_some() {
        return Err(Error::msg("this lead is already converted"));
    }
    Ok(())
}

fn set_optional_link(data: &mut Value, field: &str, model: &str, what: &str, id: Option<String>) -> Result<()> {
    if let Some(id) = id.filter(|v| !v.is_empty()) {
        let row = require(model, &id, what)?;
        data[field] = json!(id_of(&row)?);
    }
    Ok(())
}

fn do_create_lead(input: CreateLead) -> Result<Record> {
    require_user()?;
    let company = require("company.company", &input.company, "company")?;
    let stage = require("crm.crm_stage", &input.stage, "stage")?;
    if stage_kind(&stage) == "lost" {
        return Err(Error::msg("cannot create a lead in a lost stage"));
    }
    let title = input.title.trim();
    if title.is_empty() {
        return Err(Error::msg("title is required"));
    }
    let mut data = json!({
        "company": id_of(&company)?,
        "title": title,
        "stage": id_of(&stage)?,
        "active": true,
    });
    set_optional_link(&mut data, "contact_party", "party.party", "party", input.contact_party)?;
    set_optional_link(&mut data, "org_party", "party.party", "party", input.org_party)?;
    set_optional_link(&mut data, "source", "crm.crm_source", "source", input.source)?;
    set_optional_link(&mut data, "industry", "crm.crm_industry", "industry", input.industry)?;
    set_optional_link(&mut data, "territory", "crm.crm_territory", "territory", input.territory)?;
    set_optional_link(&mut data, "team", "crm.crm_team", "team", input.team)?;
    if let Some(owner) = input.owner.filter(|o| !o.is_empty()) {
        data["owner"] = json!(owner.trim());
    }
    for (field, value) in [
        ("email", input.email),
        ("phone", input.phone),
        ("person_name", input.person_name),
        ("org_name", input.org_name),
        ("utm_source", input.utm_source),
        ("utm_medium", input.utm_medium),
        ("utm_campaign", input.utm_campaign),
    ] {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            data[field] = json!(value.trim());
        }
    }
    if let Some(score) = input.score {
        data["score"] = json!(score);
    }
    let lead = db::create("lead", &data).map_err(|e| e.or("could not create the lead"))?;
    let lead_id = id_of(&lead)?.to_string();
    let _: Value = plugins::call(
        "crm",
        "record_pipeline_event",
        &json!({
            "subject_kind": "lead",
            "subject_id": lead_id,
            "to_stage": id_of(&stage)?,
            "kind": "enter",
        }),
    )?;
    let _ = events::emit("lead_created", &json!({ "id": lead_id }));
    Ok(lead)
}

fn do_set_lead(input: SetLead) -> Result<Record> {
    require_user()?;
    let lead = require("lead", &input.id, "lead")?;
    refuse_if_converted(&lead)?;
    let mut changes = json!({});
    if let Some(title) = input.title {
        let title = title.trim();
        if title.is_empty() {
            return Err(Error::msg("title is required"));
        }
        changes["title"] = json!(title);
    }
    for (field, model, what, value) in [
        ("contact_party", "party.party", "party", input.contact_party),
        ("org_party", "party.party", "party", input.org_party),
        ("source", "crm.crm_source", "source", input.source),
        ("industry", "crm.crm_industry", "industry", input.industry),
        ("territory", "crm.crm_territory", "territory", input.territory),
        ("team", "crm.crm_team", "team", input.team),
    ] {
        if let Some(value) = value {
            if value.is_empty() {
                changes[field] = Value::Null;
            } else {
                let row = require(model, &value, what)?;
                changes[field] = json!(id_of(&row)?);
            }
        }
    }
    if let Some(owner) = input.owner {
        changes["owner"] = if owner.is_empty() { Value::Null } else { json!(owner.trim()) };
    }
    for (field, value) in [
        ("email", input.email),
        ("phone", input.phone),
        ("person_name", input.person_name),
        ("org_name", input.org_name),
        ("utm_source", input.utm_source),
        ("utm_medium", input.utm_medium),
        ("utm_campaign", input.utm_campaign),
    ] {
        if let Some(value) = value {
            changes[field] = json!(value.trim());
        }
    }
    if let Some(score) = input.score {
        changes["score"] = json!(score);
    }
    db::update("lead", &input.id, &changes)?.ok_or_else(|| Error::msg("the lead is gone"))
}

fn do_lead_stage_options(input: Id) -> Result<Vec<Record>> {
    let lead = require("lead", &input.id, "lead")?;
    let Some(stage_id) = text(&lead, "stage") else { return Ok(Vec::new()) };
    let stage = require("crm.crm_stage", stage_id, "stage")?;
    let Some(pipeline) = text(&stage, "pipeline") else { return Ok(Vec::new()) };
    plugins::call("crm", "list_stages", &json!({ "id": pipeline }))
}

fn do_set_stage(input: SetStage) -> Result<Record> {
    require_user()?;
    let lead = require("lead", &input.id, "lead")?;
    refuse_if_converted(&lead)?;
    let from = text(&lead, "stage").map(str::to_string);
    let stage = require("crm.crm_stage", &input.stage, "stage")?;
    if stage_kind(&stage) == "lost" {
        return Err(Error::msg("use lose_lead to mark a lead lost"));
    }
    let updated = db::update("lead", &input.id, &json!({ "stage": id_of(&stage)? }))?
        .ok_or_else(|| Error::msg("the lead is gone"))?;
    let _: Value = plugins::call(
        "crm",
        "record_pipeline_event",
        &json!({
            "subject_kind": "lead",
            "subject_id": input.id,
            "from_stage": from,
            "to_stage": id_of(&stage)?,
            "kind": "enter",
        }),
    )?;
    if stage_kind(&stage) == "converted" || text(&stage, "name").map(|n| n.eq_ignore_ascii_case("qualified")).unwrap_or(false) {
        let _ = events::emit("lead_qualified", &json!({ "id": input.id }));
    }
    Ok(updated)
}

fn do_lose_lead(input: LoseLead) -> Result<Record> {
    require_user()?;
    let lead = require("lead", &input.id, "lead")?;
    refuse_if_converted(&lead)?;
    let reason = require("crm.crm_lost_reason", &input.lost_reason, "lost reason")?;
    if reason.get("requires_notes") == Some(&json!(true)) {
        let notes = input.lost_notes.as_deref().unwrap_or("").trim();
        if notes.is_empty() {
            return Err(Error::msg("lost notes are required for this reason"));
        }
    }
    let from = text(&lead, "stage").map(str::to_string);
    // Prefer an active lost stage if one exists on the same pipeline as the current stage.
    let mut changes = json!({
        "lost_reason": id_of(&reason)?,
        "lost_notes": input.lost_notes.as_deref().unwrap_or("").trim(),
        "active": false,
    });
    if let Some(stage_id) = from.as_deref() {
        if let Ok(current) = require("crm.crm_stage", stage_id, "stage") {
            if let Some(pipeline) = text(&current, "pipeline") {
                if let Some(lost_stage) = db::find::<Record>("crm.crm_stage")
                    .filter("pipeline", pipeline)
                    .filter("kind", "lost")
                    .first()?
                {
                    changes["stage"] = json!(id_of(&lost_stage)?);
                    let _: Value = plugins::call(
                        "crm",
                        "record_pipeline_event",
                        &json!({
                            "subject_kind": "lead",
                            "subject_id": input.id,
                            "from_stage": stage_id,
                            "to_stage": id_of(&lost_stage)?,
                            "kind": "lose",
                        }),
                    )?;
                }
            }
        }
    }
    let updated = db::update("lead", &input.id, &changes)?.ok_or_else(|| Error::msg("the lead is gone"))?;
    let _ = events::emit("lead_lost", &json!({ "id": input.id }));
    Ok(updated)
}

fn resolve_parties(lead: &Record, input: &ConvertInput) -> Result<(Option<String>, Option<String>)> {
    let mut contact = input
        .contact_party
        .clone()
        .filter(|p| !p.is_empty())
        .or_else(|| text(lead, "contact_party").map(str::to_string));
    let mut org = input
        .org_party
        .clone()
        .filter(|p| !p.is_empty())
        .or_else(|| text(lead, "org_party").map(str::to_string));

    if contact.is_none() {
        if let Some(email) = text(lead, "email") {
            let found: Value = plugins::call("party", "search", &json!({ "q": email, "kind": "person" }))?;
            if let Some(rows) = found.as_array() {
                if let Some(first) = rows.first() {
                    contact = text(first, "id").map(str::to_string);
                }
            }
        }
        if contact.is_none() {
            let name = text(lead, "person_name").unwrap_or("Lead contact");
            let mut payload = json!({ "name": name });
            if let Some(email) = text(lead, "email") {
                payload["email"] = json!(email);
            }
            if let Some(phone) = text(lead, "phone") {
                payload["phone"] = json!(phone);
            }
            let person: Record = plugins::call("party", "create_person", &payload)?;
            contact = Some(id_of(&person)?.to_string());
        }
    }

    if org.is_none() {
        if let Some(org_name) = text(lead, "org_name") {
            let found: Value = plugins::call("party", "search", &json!({ "q": org_name, "kind": "organization" }))?;
            if let Some(rows) = found.as_array() {
                if let Some(first) = rows.first() {
                    org = text(first, "id").map(str::to_string);
                }
            }
            if org.is_none() {
                let organization: Record =
                    plugins::call("party", "create_organization", &json!({ "name": org_name }))?;
                org = Some(id_of(&organization)?.to_string());
            }
        }
    }

    if let (Some(contact_id), Some(org_id)) = (&contact, &org) {
        let _: Value = plugins::call(
            "party",
            "update_party",
            &json!({ "id": contact_id, "parent": org_id }),
        )?;
    }
    Ok((contact, org))
}

fn do_convert(input: ConvertInput) -> Result<Record> {
    require_user()?;
    let lead = require("lead", &input.lead, "lead")?;
    let mode = parse_convert_mode(input.if_converted.as_deref(), input.create_another.unwrap_or(false));

    if let Some(existing) = text(&lead, "converted_deal") {
        match mode {
            ConvertMode::Fail => {
                return Err(Error::msg(format!("lead is already converted to deal `{existing}`")));
            }
            ConvertMode::Create => {}
            ConvertMode::ReturnExisting => {
                let deal: Record = plugins::call("crm_deal", "get_deal", &json!({ "id": existing }))?;
                return Ok(deal);
            }
        }
    }

    if let Some(stage_id) = text(&lead, "stage") {
        let stage = require("crm.crm_stage", stage_id, "stage")?;
        if stage_kind(&stage) == "lost" {
            return Err(Error::msg("cannot convert a lost lead"));
        }
    }

    let (contact, org) = resolve_parties(&lead, &input)?;
    let company = text(&lead, "company").ok_or_else(|| Error::msg("lead has no company"))?;

    let customer_party = org.clone().or_else(|| contact.clone());
    let customer = if let Some(party_id) = customer_party {
        let customer: Record = plugins::call(
            "crm_customer",
            "ensure_customer",
            &json!({ "party": party_id, "company": company }),
        )?;
        Some(id_of(&customer)?.to_string())
    } else {
        None
    };

    let mut deal_input = json!({
        "company": company,
        "title": text(&lead, "title").unwrap_or("Deal"),
        "lead": input.lead,
        "owner": text(&lead, "owner"),
        "source": text(&lead, "source"),
        "territory": text(&lead, "territory"),
        "team": text(&lead, "team"),
        "utm_source": text(&lead, "utm_source"),
        "utm_medium": text(&lead, "utm_medium"),
        "utm_campaign": text(&lead, "utm_campaign"),
        "from_convert": true,
    });
    if let Some(contact) = &contact {
        deal_input["contact_party"] = json!(contact);
    }
    if let Some(org) = &org {
        deal_input["org_party"] = json!(org);
    }
    if let Some(customer) = &customer {
        deal_input["customer"] = json!(customer);
    }

    let deal: Record = plugins::call("crm_deal", "create_deal", &deal_input)?;
    let deal_id = id_of(&deal)?.to_string();

    let mut lead_changes = json!({ "converted_deal": deal_id });
    // Move lead to a converted stage when the pipeline has one.
    if let Some(stage_id) = text(&lead, "stage") {
        if let Ok(current) = require("crm.crm_stage", stage_id, "stage") {
            if let Some(pipeline) = text(&current, "pipeline") {
                if let Some(converted) = db::find::<Record>("crm.crm_stage")
                    .filter("pipeline", pipeline)
                    .filter("kind", "converted")
                    .first()?
                {
                    lead_changes["stage"] = json!(id_of(&converted)?);
                    let _: Value = plugins::call(
                        "crm",
                        "record_pipeline_event",
                        &json!({
                            "subject_kind": "lead",
                            "subject_id": input.lead,
                            "from_stage": stage_id,
                            "to_stage": id_of(&converted)?,
                            "kind": "convert",
                        }),
                    )?;
                }
            }
        }
    }
    let _: Option<Record> = db::update("lead", &input.lead, &lead_changes)?;

    let _ = events::emit(
        "lead_converted",
        &json!({
            "lead": input.lead,
            "deal": deal_id,
            "contact_party": contact,
            "org_party": org,
        }),
    );
    Ok(deal)
}

fn do_get_lead(input: Id) -> Result<Record> {
    require_user()?;
    require("lead", &input.id, "lead")
}

fn do_search_leads(input: Search) -> Result<Vec<Record>> {
    require_user()?;
    let mut q = db::find::<Record>("lead").limit(200);
    if let Some(company) = input.company.filter(|c| !c.is_empty()) {
        q = q.filter("company", company);
    }
    let mut rows = q.all()?;
    if input.active_only.unwrap_or(true) {
        rows.retain(|row| row.get("active") != Some(&json!(false)));
    }
    if let Some(needle) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let needle = needle.to_lowercase();
        rows.retain(|row| {
            ["title", "number", "email", "person_name", "org_name"].iter().any(|field| {
                text(row, field).map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
            })
        });
    }
    Ok(rows)
}

handler! {
    fn create_lead(input: CreateLead) -> Record { do_create_lead(input) }
    fn set_lead(input: SetLead) -> Record { do_set_lead(input) }
    fn set_stage(input: SetStage) -> Record { do_set_stage(input) }
    /// The stages of the pipeline the lead's current stage belongs to, in order (for the state bar).
    fn lead_stage_options(input: Id) -> Vec<Record> { do_lead_stage_options(input) }
    fn lose_lead(input: LoseLead) -> Record { do_lose_lead(input) }
    fn convert(input: ConvertInput) -> Record { do_convert(input) }
    fn get_lead(input: Id) -> Record { do_get_lead(input) }
    fn search_leads(input: Search) -> Vec<Record> { do_search_leads(input) }
}

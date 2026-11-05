//! Customer 360 overlay on party.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record, LIFECYCLES};

#[derive(Deserialize)]
struct EnsureInput {
    party: String,
    company: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    territory: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    segment: Option<String>,
    #[serde(default)]
    lifecycle: Option<String>,
}

#[derive(Deserialize)]
struct SetCustomer {
    id: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    territory: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    segment: Option<String>,
    #[serde(default)]
    lifecycle: Option<String>,
    #[serde(default)]
    active: Option<bool>,
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

fn lifecycle_or(value: Option<&str>, fallback: &str) -> Result<String> {
    let life = value.unwrap_or(fallback);
    if LIFECYCLES.contains(&life) {
        Ok(life.to_string())
    } else {
        Err(Error::msg("lifecycle is prospect, active or churned"))
    }
}

fn do_ensure_customer(input: EnsureInput) -> Result<Record> {
    require_user()?;
    let party: Record = plugins::call("party", "get_party", &json!({ "id": input.party }))?;
    let party_id = id_of(&party)?.to_string();
    let company = require("company.company", &input.company, "company")?;
    let company_id = id_of(&company)?.to_string();

    if let Some(existing) = db::find::<Record>("customer")
        .filter("party", &party_id)
        .filter("company", &company_id)
        .first()?
    {
        return Ok(existing);
    }

    let _: Value = plugins::call(
        "party",
        "update_party",
        &json!({ "id": party_id, "is_customer": true }),
    )?;

    let mut data = json!({
        "party": party_id,
        "company": company_id,
        "lifecycle": lifecycle_or(input.lifecycle.as_deref(), "prospect")?,
        "active": true,
    });
    if let Some(owner) = input.owner.filter(|o| !o.is_empty()) {
        data["owner"] = json!(owner.trim());
    }
    if let Some(territory_id) = input.territory.filter(|t| !t.is_empty()) {
        let territory = require("crm.crm_territory", &territory_id, "territory")?;
        data["territory"] = json!(id_of(&territory)?);
    }
    if let Some(team_id) = input.team.filter(|t| !t.is_empty()) {
        let team = require("crm.crm_team", &team_id, "team")?;
        data["team"] = json!(id_of(&team)?);
    }
    if let Some(segment) = input.segment.filter(|s| !s.is_empty()) {
        data["segment"] = json!(segment.trim());
    }

    let customer = db::create("customer", &data).map_err(|e| e.or("could not create the customer"))?;
    let _ = events::emit("customer_created", &json!({ "id": id_of(&customer)? }));
    Ok(customer)
}

fn do_set_customer(input: SetCustomer) -> Result<Record> {
    require_user()?;
    let _ = require("customer", &input.id, "customer")?;
    let mut changes = json!({});
    if let Some(owner) = input.owner {
        changes["owner"] = if owner.is_empty() { Value::Null } else { json!(owner.trim()) };
    }
    if let Some(territory_id) = input.territory {
        if territory_id.is_empty() {
            changes["territory"] = Value::Null;
        } else {
            let territory = require("crm.crm_territory", &territory_id, "territory")?;
            changes["territory"] = json!(id_of(&territory)?);
        }
    }
    if let Some(team_id) = input.team {
        if team_id.is_empty() {
            changes["team"] = Value::Null;
        } else {
            let team = require("crm.crm_team", &team_id, "team")?;
            changes["team"] = json!(id_of(&team)?);
        }
    }
    if let Some(segment) = input.segment {
        changes["segment"] = json!(segment.trim());
    }
    if let Some(lifecycle) = input.lifecycle {
        changes["lifecycle"] = json!(lifecycle_or(Some(&lifecycle), "prospect")?);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("customer", &input.id, &changes)?.ok_or_else(|| Error::msg("the customer is gone"))
}

fn do_get_customer(input: Id) -> Result<Record> {
    require_user()?;
    require("customer", &input.id, "customer")
}

fn do_search_customers(input: Search) -> Result<Vec<Record>> {
    require_user()?;
    let mut q = db::find::<Record>("customer").limit(200);
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
            text(row, "segment").map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
                || text(row, "owner").map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
                || text(row, "party").map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
        });
    }
    Ok(rows)
}

handler! {
    fn ensure_customer(input: EnsureInput) -> Record { do_ensure_customer(input) }
    fn set_customer(input: SetCustomer) -> Record { do_set_customer(input) }
    fn get_customer(input: Id) -> Record { do_get_customer(input) }
    fn search_customers(input: Search) -> Vec<Record> { do_search_customers(input) }
}

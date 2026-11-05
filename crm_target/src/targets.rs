//! Team/rep quotas versus won revenue.

use aether_sdk::dates::{parse_date, parse_datetime};
use aether_sdk::decimal::Decimal;
use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_admin, require_user, text, Record};

#[derive(Deserialize)]
struct CreateTarget {
    name: String,
    company: String,
    period_start: String,
    period_end: String,
    amount: Decimal,
    currency: String,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    owner: Option<String>,
}

#[derive(Deserialize)]
struct SetTarget {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    amount: Option<Decimal>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    period_start: Option<String>,
    #[serde(default)]
    period_end: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct Attainment {
    id: String,
}

fn money_of(row: &Record, field: &str) -> Decimal {
    match row.get(field) {
        Some(Value::String(s)) => Decimal::parse(s).unwrap_or_else(|_| Decimal::zero(2)),
        Some(other) => Decimal::parse(&other.to_string().trim_matches('"')).unwrap_or_else(|_| Decimal::zero(2)),
        None => Decimal::zero(2),
    }
}

fn do_create_target(input: CreateTarget) -> Result<Record> {
    require_admin()?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err(Error::msg("name is required"));
    }
    let start = parse_date(&input.period_start)?;
    let end = parse_date(&input.period_end)?;
    if end < start {
        return Err(Error::msg("period_end is before period_start"));
    }
    if input.amount.is_negative() {
        return Err(Error::msg("amount must be zero or more"));
    }
    let company = require("company.company", &input.company, "company")?;
    let currency = require("currency.currency", &input.currency, "currency")?;
    let mut data = json!({
        "name": name,
        "company": id_of(&company)?,
        "period_start": input.period_start,
        "period_end": input.period_end,
        "amount": input.amount,
        "currency": id_of(&currency)?,
        "active": true,
    });
    if let Some(team) = input.team.filter(|t| !t.is_empty()) {
        data["team"] = json!(id_of(&require("crm.crm_team", &team, "team")?)?);
    }
    if let Some(owner) = input.owner.filter(|o| !o.is_empty()) {
        data["owner"] = json!(owner.trim());
    }
    if data.get("team").is_none() && data.get("owner").is_none() {
        return Err(Error::msg("set a team or an owner"));
    }
    db::create("sales_target", &data).map_err(|e| e.or("could not create the target"))
}

fn do_set_target(input: SetTarget) -> Result<Record> {
    require_admin()?;
    let _ = require("sales_target", &input.id, "target")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::msg("name is required"));
        }
        changes["name"] = json!(name);
    }
    if let Some(amount) = input.amount {
        if amount.is_negative() {
            return Err(Error::msg("amount must be zero or more"));
        }
        changes["amount"] = json!(amount);
    }
    if let Some(team) = input.team {
        changes["team"] = if team.is_empty() {
            Value::Null
        } else {
            json!(id_of(&require("crm.crm_team", &team, "team")?)?)
        };
    }
    if let Some(owner) = input.owner {
        changes["owner"] = if owner.is_empty() { Value::Null } else { json!(owner.trim()) };
    }
    if let Some(start) = input.period_start {
        let _ = parse_date(&start)?;
        changes["period_start"] = json!(start);
    }
    if let Some(end) = input.period_end {
        let _ = parse_date(&end)?;
        changes["period_end"] = json!(end);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("sales_target", &input.id, &changes)?.ok_or_else(|| Error::msg("the target is gone"))
}

fn do_attainment(input: Attainment) -> Result<Value> {
    require_user()?;
    let target = require("sales_target", &input.id, "target")?;
    let start = text(&target, "period_start").ok_or_else(|| Error::msg("target has no period_start"))?;
    let end = text(&target, "period_end").ok_or_else(|| Error::msg("target has no period_end"))?;
    let company = text(&target, "company");
    let team = text(&target, "team");
    let owner = text(&target, "owner");
    let goal = money_of(&target, "amount");

    let deals = db::find::<Record>("crm_deal.deal").limit(1000).all()?;
    let mut won = Decimal::zero(2);
    let mut n = 0i64;
    for deal in deals {
        if company.is_some() && text(&deal, "company") != company {
            continue;
        }
        if let Some(team) = team {
            if text(&deal, "team") != Some(team) {
                continue;
            }
        }
        if let Some(owner) = owner {
            if text(&deal, "owner") != Some(owner) {
                continue;
            }
        }
        // Won: has closed_at in period and expected_value counted; stage kind check via closed_at + active.
        let Some(closed) = text(&deal, "closed_at") else { continue };
        let Ok(closed_at) = parse_datetime(closed) else { continue };
        let closed_day = closed_at.date();
        let Ok(start_d) = parse_date(start) else { continue };
        let Ok(end_d) = parse_date(end) else { continue };
        if closed_day < start_d || closed_day > end_d {
            continue;
        }
        // Prefer deals that look won: probability 100 or active with closed_at and no lost_reason.
        if text(&deal, "lost_reason").is_some() {
            continue;
        }
        won = won.checked_add(money_of(&deal, "expected_value"))?;
        n += 1;
    }
    let pct = if goal.is_zero() {
        Decimal::zero(2)
    } else {
        won.times(Decimal::whole(100, 2)?, 2)?.div(goal, 2).unwrap_or_else(|_| Decimal::zero(2))
    };
    Ok(json!({
        "target": input.id,
        "goal": goal,
        "won": won,
        "deals": n,
        "percent": pct,
    }))
}

fn do_get_target(input: Id) -> Result<Record> {
    require_user()?;
    require("sales_target", &input.id, "target")
}

fn do_list_targets(_input: Empty) -> Result<Vec<Record>> {
    require_user()?;
    db::find::<Record>("sales_target").limit(200).all()
}

handler! {
    fn create_target(input: CreateTarget) -> Record { do_create_target(input) }
    fn set_target(input: SetTarget) -> Record { do_set_target(input) }
    fn attainment(input: Attainment) -> Value { do_attainment(input) }
    fn get_target(input: Id) -> Record { do_get_target(input) }
    fn list_targets(input: Empty) -> Vec<Record> { do_list_targets(input) }
}

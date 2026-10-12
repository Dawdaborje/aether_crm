//! Deals: pipeline moves, lines, win/lose/reopen.

use aether_sdk::decimal::Decimal;
use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record};

#[derive(Deserialize)]
struct CreateDeal {
    company: String,
    title: String,
    #[serde(default)]
    pipeline: Option<String>,
    #[serde(default)]
    stage: Option<String>,
    #[serde(default)]
    lead: Option<String>,
    #[serde(default)]
    contact_party: Option<String>,
    #[serde(default)]
    org_party: Option<String>,
    #[serde(default)]
    customer: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    territory: Option<String>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    expected_value: Option<Decimal>,
    #[serde(default)]
    expected_close: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    utm_source: Option<String>,
    #[serde(default)]
    utm_medium: Option<String>,
    #[serde(default)]
    utm_campaign: Option<String>,
    #[serde(default)]
    from_convert: Option<bool>,
}

#[derive(Deserialize)]
struct SetDeal {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    territory: Option<String>,
    #[serde(default)]
    expected_value: Option<Decimal>,
    #[serde(default)]
    probability: Option<Decimal>,
    #[serde(default)]
    expected_close: Option<String>,
    #[serde(default)]
    contact_party: Option<String>,
    #[serde(default)]
    org_party: Option<String>,
    #[serde(default)]
    customer: Option<String>,
}

#[derive(Deserialize)]
struct SetStage {
    id: String,
    stage: String,
    #[serde(default)]
    probability: Option<Decimal>,
}

#[derive(Deserialize)]
struct SetLines {
    deal: String,
    lines: Vec<LineInput>,
}

#[derive(Deserialize)]
struct LineInput {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    qty: Option<Decimal>,
    #[serde(default)]
    unit_price: Option<Decimal>,
}

#[derive(Deserialize)]
struct LoseDeal {
    id: String,
    lost_reason: String,
    #[serde(default)]
    lost_notes: Option<String>,
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

fn refuse_closed(deal: &Record) -> Result<()> {
    if let Some(stage_id) = text(deal, "stage") {
        let stage = require("crm.crm_stage", stage_id, "stage")?;
        let kind = stage_kind(&stage);
        if kind == "won" || kind == "lost" {
            return Err(Error::msg("this deal is closed; reopen it first"));
        }
    }
    Ok(())
}

fn default_pipeline_and_stage(company_id: &str) -> Result<(String, Record)> {
    if let Ok(Some(settings)) = db::find::<Record>("crm.crm_company_settings").filter("company", company_id).first() {
        if let Some(pipeline_id) = text(&settings, "default_pipeline") {
            let pipeline = require("crm.crm_pipeline", pipeline_id, "pipeline")?;
            if let Some(stage) = first_open_stage(id_of(&pipeline)?)? {
                return Ok((id_of(&pipeline)?.to_string(), stage));
            }
        }
    }
    let pipeline = db::find::<Record>("crm.crm_pipeline")
        .filter("company", company_id)
        .first()?
        .ok_or_else(|| Error::msg("create a pipeline for this company first"))?;
    let pipeline_id = id_of(&pipeline)?.to_string();
    let stage = first_open_stage(&pipeline_id)?.ok_or_else(|| Error::msg("the pipeline has no open stage"))?;
    Ok((pipeline_id, stage))
}

fn first_open_stage(pipeline_id: &str) -> Result<Option<Record>> {
    let mut stages = db::find::<Record>("crm.crm_stage").filter("pipeline", pipeline_id).limit(100).all()?;
    stages.retain(|s| s.get("active") != Some(&json!(false)) && stage_kind(s) == "open");
    stages.sort_by(|a, b| a.get("sequence").and_then(Value::as_i64).cmp(&b.get("sequence").and_then(Value::as_i64)));
    Ok(stages.into_iter().next())
}

fn stage_of_kind(pipeline_id: &str, kind: &str) -> Result<Record> {
    db::find::<Record>("crm.crm_stage")
        .filter("pipeline", pipeline_id)
        .filter("kind", kind)
        .first()?
        .ok_or_else(|| Error::msg(format!("the pipeline has no {kind} stage")))
}

fn record_event(subject_id: &str, from: Option<&str>, to: &str, kind: &str) -> Result<()> {
    let _: Value = plugins::call(
        "crm",
        "record_pipeline_event",
        &json!({
            "subject_kind": "deal",
            "subject_id": subject_id,
            "from_stage": from,
            "to_stage": to,
            "kind": kind,
        }),
    )?;
    Ok(())
}

fn do_create_deal(input: CreateDeal) -> Result<Record> {
    require_user()?;
    let company = require("company.company", &input.company, "company")?;
    let company_id = id_of(&company)?.to_string();
    let title = input.title.trim();
    if title.is_empty() {
        return Err(Error::msg("title is required"));
    }

    let (pipeline_id, stage) = if let Some(stage_id) = input.stage.filter(|s| !s.is_empty()) {
        let stage = require("crm.crm_stage", &stage_id, "stage")?;
        let pipeline_id = text(&stage, "pipeline")
            .map(str::to_string)
            .ok_or_else(|| Error::msg("stage has no pipeline"))?;
        (pipeline_id, stage)
    } else if let Some(pipeline_id) = input.pipeline.filter(|p| !p.is_empty()) {
        let pipeline = require("crm.crm_pipeline", &pipeline_id, "pipeline")?;
        let pipeline_id = id_of(&pipeline)?.to_string();
        let stage = first_open_stage(&pipeline_id)?.ok_or_else(|| Error::msg("the pipeline has no open stage"))?;
        (pipeline_id, stage)
    } else {
        default_pipeline_and_stage(&company_id)?
    };

    let probability = text(&stage, "default_probability")
        .and_then(|p| Decimal::parse(p).ok())
        .unwrap_or_else(|| Decimal::zero(2));

    let mut data = json!({
        "company": company_id,
        "title": title,
        "pipeline": pipeline_id,
        "stage": id_of(&stage)?,
        "probability": probability,
        "probability_manual": false,
        "expected_value": input.expected_value.unwrap_or_else(|| Decimal::zero(2)),
        "active": true,
    });
    if let Some(lead) = input.lead.filter(|l| !l.is_empty()) {
        data["lead"] = json!(lead);
    }
    for (field, model, what, value) in [
        ("contact_party", "party.party", "party", input.contact_party),
        ("org_party", "party.party", "party", input.org_party),
        ("customer", "crm_customer.customer", "customer", input.customer),
        ("team", "crm.crm_team", "team", input.team),
        ("territory", "crm.crm_territory", "territory", input.territory),
        ("currency", "currency.currency", "currency", input.currency),
        ("source", "crm.crm_source", "source", input.source),
    ] {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            let row = require(model, &value, what)?;
            data[field] = json!(id_of(&row)?);
        }
    }
    if let Some(owner) = input.owner.filter(|o| !o.is_empty()) {
        data["owner"] = json!(owner.trim());
    }
    if let Some(close) = input.expected_close.filter(|d| !d.is_empty()) {
        data["expected_close"] = json!(close);
    }
    for (field, value) in [
        ("utm_source", input.utm_source),
        ("utm_medium", input.utm_medium),
        ("utm_campaign", input.utm_campaign),
    ] {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            data[field] = json!(value.trim());
        }
    }

    let deal = db::create("deal", &data).map_err(|e| e.or("could not create the deal"))?;
    let deal_id = id_of(&deal)?.to_string();
    let event_kind = if input.from_convert.unwrap_or(false) { "convert" } else { "enter" };
    record_event(&deal_id, None, id_of(&stage)?, event_kind)?;
    let _ = events::emit("deal_stage_changed", &json!({ "id": deal_id, "stage": id_of(&stage)?, "kind": event_kind }));
    Ok(deal)
}

fn do_set_deal(input: SetDeal) -> Result<Record> {
    require_user()?;
    let deal = require("deal", &input.id, "deal")?;
    refuse_closed(&deal)?;
    let mut changes = json!({});
    if let Some(title) = input.title {
        let title = title.trim();
        if title.is_empty() {
            return Err(Error::msg("title is required"));
        }
        changes["title"] = json!(title);
    }
    if let Some(owner) = input.owner {
        changes["owner"] = if owner.is_empty() { Value::Null } else { json!(owner.trim()) };
    }
    if let Some(value) = input.expected_value {
        changes["expected_value"] = json!(value);
    }
    if let Some(probability) = input.probability {
        let hundred = Decimal::whole(100, 2)?;
        if probability.is_negative() || probability > hundred {
            return Err(Error::msg("probability must be between 0 and 100"));
        }
        changes["probability"] = json!(probability);
        changes["probability_manual"] = json!(true);
    }
    if let Some(close) = input.expected_close {
        changes["expected_close"] = if close.is_empty() { Value::Null } else { json!(close) };
    }
    for (field, model, what, value) in [
        ("team", "crm.crm_team", "team", input.team),
        ("territory", "crm.crm_territory", "territory", input.territory),
        ("contact_party", "party.party", "party", input.contact_party),
        ("org_party", "party.party", "party", input.org_party),
        ("customer", "crm_customer.customer", "customer", input.customer),
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
    db::update("deal", &input.id, &changes)?.ok_or_else(|| Error::msg("the deal is gone"))
}

fn do_set_stage(input: SetStage) -> Result<Record> {
    require_user()?;
    let deal = require("deal", &input.id, "deal")?;
    refuse_closed(&deal)?;
    let from = text(&deal, "stage").map(str::to_string);
    let stage = require("crm.crm_stage", &input.stage, "stage")?;
    let kind = stage_kind(&stage);
    if kind == "won" || kind == "lost" {
        return Err(Error::msg("use win_deal or lose_deal to close a deal"));
    }
    let mut changes = json!({ "stage": id_of(&stage)? });
    if let Some(probability) = input.probability {
        changes["probability"] = json!(probability);
        changes["probability_manual"] = json!(true);
    } else if deal.get("probability_manual") != Some(&json!(true)) {
        if let Some(default) = text(&stage, "default_probability") {
            changes["probability"] = json!(default);
        }
    }
    let updated = db::update("deal", &input.id, &changes)?.ok_or_else(|| Error::msg("the deal is gone"))?;
    record_event(&input.id, from.as_deref(), id_of(&stage)?, "enter")?;
    let _ = events::emit("deal_stage_changed", &json!({ "id": input.id, "stage": id_of(&stage)? }));
    Ok(updated)
}

fn do_set_lines(input: SetLines) -> Result<Vec<Record>> {
    require_user()?;
    let deal = require("deal", &input.deal, "deal")?;
    refuse_closed(&deal)?;
    let existing = db::find::<Record>("deal_line").filter("deal", &input.deal).limit(500).all()?;
    for row in existing {
        db::delete::<Record>("deal_line", id_of(&row)?)?;
    }
    let mut created = Vec::new();
    let mut total = Decimal::zero(2);
    for (index, line) in input.lines.into_iter().enumerate() {
        let qty = line.qty.unwrap_or_else(|| Decimal::whole(1, 4).unwrap_or_else(|_| Decimal::zero(4)));
        let unit = line.unit_price.unwrap_or_else(|| Decimal::zero(2));
        let amount = qty.times(unit, 2).unwrap_or_else(|_| Decimal::zero(2));
        total = total.checked_add(amount)?;
        let data = json!({
            "deal": input.deal,
            "line_no": (index as i64) + 1,
            "description": line.description.unwrap_or_default(),
            "qty": qty,
            "unit_price": unit,
            "amount": amount,
        });
        created.push(db::create("deal_line", &data)?);
    }
    let _: Option<Record> = db::update("deal", &input.deal, &json!({ "expected_value": total }))?;
    Ok(created)
}

fn do_win_deal(input: Id) -> Result<Record> {
    require_user()?;
    let deal = require("deal", &input.id, "deal")?;
    refuse_closed(&deal)?;
    let pipeline = text(&deal, "pipeline").ok_or_else(|| Error::msg("deal has no pipeline"))?;
    let from = text(&deal, "stage").map(str::to_string);
    let stage = stage_of_kind(pipeline, "won")?;
    let ctx = context::current()?;
    let updated = db::update(
        "deal",
        &input.id,
        &json!({
            "stage": id_of(&stage)?,
            "probability": Decimal::whole(100, 2)?,
            "closed_at": ctx.now,
            "active": true,
        }),
    )?
    .ok_or_else(|| Error::msg("the deal is gone"))?;
    record_event(&input.id, from.as_deref(), id_of(&stage)?, "win")?;
    let _ = events::emit("deal_won", &json!({ "id": input.id }));
    Ok(updated)
}

fn do_lose_deal(input: LoseDeal) -> Result<Record> {
    require_user()?;
    let deal = require("deal", &input.id, "deal")?;
    refuse_closed(&deal)?;
    let reason = require("crm.crm_lost_reason", &input.lost_reason, "lost reason")?;
    if reason.get("requires_notes") == Some(&json!(true)) {
        let notes = input.lost_notes.as_deref().unwrap_or("").trim();
        if notes.is_empty() {
            return Err(Error::msg("lost notes are required for this reason"));
        }
    }
    let pipeline = text(&deal, "pipeline").ok_or_else(|| Error::msg("deal has no pipeline"))?;
    let from = text(&deal, "stage").map(str::to_string);
    let stage = stage_of_kind(pipeline, "lost")?;
    let ctx = context::current()?;
    let updated = db::update(
        "deal",
        &input.id,
        &json!({
            "stage": id_of(&stage)?,
            "probability": Decimal::zero(2),
            "lost_reason": id_of(&reason)?,
            "lost_notes": input.lost_notes.as_deref().unwrap_or("").trim(),
            "closed_at": ctx.now,
            "active": false,
        }),
    )?
    .ok_or_else(|| Error::msg("the deal is gone"))?;
    record_event(&input.id, from.as_deref(), id_of(&stage)?, "lose")?;
    let _ = events::emit("deal_lost", &json!({ "id": input.id }));
    Ok(updated)
}

fn do_reopen_deal(input: Id) -> Result<Record> {
    require_user()?;
    let deal = require("deal", &input.id, "deal")?;
    let pipeline = text(&deal, "pipeline").ok_or_else(|| Error::msg("deal has no pipeline"))?;
    let from = text(&deal, "stage").map(str::to_string);
    let stage = first_open_stage(pipeline)?.ok_or_else(|| Error::msg("the pipeline has no open stage"))?;
    let probability = text(&stage, "default_probability").unwrap_or("0");
    let updated = db::update(
        "deal",
        &input.id,
        &json!({
            "stage": id_of(&stage)?,
            "probability": probability,
            "probability_manual": false,
            "closed_at": Value::Null,
            "lost_reason": Value::Null,
            "lost_notes": Value::Null,
            "active": true,
        }),
    )?
    .ok_or_else(|| Error::msg("the deal is gone"))?;
    record_event(&input.id, from.as_deref(), id_of(&stage)?, "reopen")?;
    let _ = events::emit("deal_stage_changed", &json!({ "id": input.id, "stage": id_of(&stage)?, "kind": "reopen" }));
    Ok(updated)
}

fn do_get_deal(input: Id) -> Result<Record> {
    require_user()?;
    require("deal", &input.id, "deal")
}

fn do_search_deals(input: Search) -> Result<Vec<Record>> {
    require_user()?;
    let mut q = db::find::<Record>("deal").limit(200);
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
            ["title", "number", "owner"].iter().any(|field| {
                text(row, field).map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
            })
        });
    }
    Ok(rows)
}

fn do_list_lines(input: Id) -> Result<Vec<Record>> {
    require_user()?;
    let _ = require("deal", &input.id, "deal")?;
    let mut rows = db::find::<Record>("deal_line").filter("deal", &input.id).limit(500).all()?;
    rows.sort_by(|a, b| a.get("line_no").and_then(Value::as_i64).cmp(&b.get("line_no").and_then(Value::as_i64)));
    Ok(rows)
}

fn do_stage_history(input: Id) -> Result<Vec<Record>> {
    require_user()?;
    let _ = require("deal", &input.id, "deal")?;
    plugins::call(
        "crm",
        "stage_history",
        &json!({ "subject_kind": "deal", "subject_id": input.id }),
    )
}

handler! {
    fn create_deal(input: CreateDeal) -> Record { do_create_deal(input) }
    fn set_deal(input: SetDeal) -> Record { do_set_deal(input) }
    fn set_stage(input: SetStage) -> Record { do_set_stage(input) }
    fn set_lines(input: SetLines) -> Vec<Record> { do_set_lines(input) }
    fn win_deal(input: Id) -> Record { do_win_deal(input) }
    fn lose_deal(input: LoseDeal) -> Record { do_lose_deal(input) }
    fn reopen_deal(input: Id) -> Record { do_reopen_deal(input) }
    fn get_deal(input: Id) -> Record { do_get_deal(input) }
    fn search_deals(input: Search) -> Vec<Record> { do_search_deals(input) }
    fn list_lines(input: Id) -> Vec<Record> { do_list_lines(input) }
    fn stage_history(input: Id) -> Vec<Record> { do_stage_history(input) }
}

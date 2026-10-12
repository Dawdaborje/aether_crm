//! Pipeline funnels, conversion rates and stage-duration metrics.

use aether_sdk::dates::{parse_datetime, Duration};
use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record};

#[derive(Deserialize)]
struct PipelineInput {
    pipeline: String,
}

#[derive(Deserialize)]
struct CompanyFilter {
    #[serde(default)]
    company: Option<String>,
}

#[derive(Deserialize)]
struct SaveFunnel {
    name: String,
    #[serde(default)]
    pipeline: Option<String>,
    #[serde(default)]
    filters: Option<String>,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

fn do_funnel(input: PipelineInput) -> Result<Value> {
    require_user()?;
    let pipeline = require("crm.crm_pipeline", &input.pipeline, "pipeline")?;
    let pipeline_id = id_of(&pipeline)?.to_string();
    let mut stages = db::find::<Record>("crm.crm_stage").filter("pipeline", &pipeline_id).limit(100).all()?;
    stages.sort_by(|a, b| a.get("sequence").and_then(Value::as_i64).cmp(&b.get("sequence").and_then(Value::as_i64)));

    let deals = db::find::<Record>("crm_deal.deal").filter("pipeline", &pipeline_id).limit(1000).all()?;
    let mut counts = Vec::new();
    for stage in &stages {
        let stage_id = id_of(stage)?;
        let n = deals.iter().filter(|d| text(d, "stage") == Some(stage_id)).count();
        counts.push(json!({
            "stage": stage_id,
            "name": text(stage, "name"),
            "kind": text(stage, "kind"),
            "count": n,
        }));
    }
    Ok(json!({ "pipeline": pipeline_id, "stages": counts, "total": deals.len() }))
}

fn do_conversion_rates(input: CompanyFilter) -> Result<Value> {
    require_user()?;
    let mut leads = db::find::<Record>("crm_lead.lead").limit(2000).all()?;
    let mut deals = db::find::<Record>("crm_deal.deal").limit(2000).all()?;
    if let Some(company) = input.company.filter(|c| !c.is_empty()) {
        leads.retain(|r| text(r, "company") == Some(company.as_str()));
        deals.retain(|r| text(r, "company") == Some(company.as_str()));
    }
    let lead_n = leads.len() as i64;
    let converted = leads.iter().filter(|l| text(l, "converted_deal").is_some()).count() as i64;
    let won = deals
        .iter()
        .filter(|d| text(d, "closed_at").is_some() && text(d, "lost_reason").is_none())
        .count() as i64;
    let lost = deals.iter().filter(|d| text(d, "lost_reason").is_some()).count() as i64;
    let lead_to_deal_pct = if lead_n == 0 { 0.0 } else { (converted as f64) * 100.0 / (lead_n as f64) };
    let closed = won + lost;
    let win_pct = if closed == 0 { 0.0 } else { (won as f64) * 100.0 / (closed as f64) };
    Ok(json!({
        "leads": lead_n,
        "converted": converted,
        "lead_to_deal_pct": lead_to_deal_pct,
        "deals": deals.len(),
        "won": won,
        "lost": lost,
        "win_pct": win_pct,
    }))
}

fn do_days_to_convert(input: CompanyFilter) -> Result<Value> {
    require_user()?;
    let mut leads = db::find::<Record>("crm_lead.lead").limit(2000).all()?;
    if let Some(company) = input.company.filter(|c| !c.is_empty()) {
        leads.retain(|r| text(r, "company") == Some(company.as_str()));
    }
    let mut days = Vec::new();
    for lead in leads {
        let Some(deal_id) = text(&lead, "converted_deal") else { continue };
        let Ok(deal) = require("crm_deal.deal", deal_id, "deal") else { continue };
        // Approximate: lead created_at vs deal created_at if present on records.
        let Some(lead_at) = text(&lead, "created_at").or_else(|| text(&lead, "updated_at")) else { continue };
        let Some(deal_at) = text(&deal, "created_at").or_else(|| text(&deal, "updated_at")) else { continue };
        let Ok(a) = parse_datetime(lead_at) else { continue };
        let Ok(b) = parse_datetime(deal_at) else { continue };
        let d = (b - a).num_days().max(0);
        days.push(d);
    }
    let n = days.len() as i64;
    let avg = if n == 0 { 0.0 } else { days.iter().sum::<i64>() as f64 / n as f64 };
    Ok(json!({ "samples": n, "avg_days": avg, "max_days": days.iter().copied().max().unwrap_or(0) }))
}

fn do_deadline_slip(input: CompanyFilter) -> Result<Value> {
    require_user()?;
    let mut deals = db::find::<Record>("crm_deal.deal").limit(2000).all()?;
    if let Some(company) = input.company.filter(|c| !c.is_empty()) {
        deals.retain(|r| text(r, "company") == Some(company.as_str()));
    }
    let ctx = context::current()?;
    let Ok(now) = parse_datetime(&ctx.now) else {
        return Ok(json!({ "open_past_close": 0, "avg_slip_days": 0.0 }));
    };
    let mut slip_days = Vec::new();
    for deal in deals {
        if text(&deal, "closed_at").is_some() {
            continue;
        }
        let Some(expected) = text(&deal, "expected_close") else { continue };
        // expected_close is a date; compare to today.
        let Ok(day) = aether_sdk::dates::parse_date(expected) else { continue };
        if day < now.date() {
            slip_days.push((now.date() - day).num_days());
        }
    }
    let n = slip_days.len() as i64;
    let avg = if n == 0 { 0.0 } else { slip_days.iter().sum::<i64>() as f64 / n as f64 };
    let _ = Duration::days(1);
    Ok(json!({ "open_past_close": n, "avg_slip_days": avg }))
}

fn do_save_funnel(input: SaveFunnel) -> Result<Record> {
    require_user()?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err(Error::msg("name is required"));
    }
    let ctx = context::current()?;
    let owner = ctx.actor.id.clone().unwrap_or_default();
    let mut data = json!({
        "name": name,
        "filters": input.filters.unwrap_or_else(|| "{}".into()),
        "owner": owner,
    });
    if let Some(pipeline) = input.pipeline.filter(|p| !p.is_empty()) {
        data["pipeline"] = json!(id_of(&require("crm.crm_pipeline", &pipeline, "pipeline")?)?);
    }
    db::create("saved_funnel", &data).map_err(|e| e.or("could not save the funnel"))
}

fn do_list_funnels(_input: Empty) -> Result<Vec<Record>> {
    require_user()?;
    db::find::<Record>("saved_funnel").limit(200).all()
}

fn do_get_funnel(input: Id) -> Result<Record> {
    require_user()?;
    require("saved_funnel", &input.id, "funnel")
}

handler! {
    fn funnel(input: PipelineInput) -> Value { do_funnel(input) }
    fn conversion_rates(input: CompanyFilter) -> Value { do_conversion_rates(input) }
    fn days_to_convert(input: CompanyFilter) -> Value { do_days_to_convert(input) }
    fn deadline_slip(input: CompanyFilter) -> Value { do_deadline_slip(input) }
    fn save_funnel(input: SaveFunnel) -> Record { do_save_funnel(input) }
    fn list_funnels(input: Empty) -> Vec<Record> { do_list_funnels(input) }
    fn get_funnel(input: Id) -> Record { do_get_funnel(input) }
}

//! Pipeline funnels, conversion rates and deadline slip, computed by `report::query`.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, Record};

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

#[derive(Deserialize)]
struct ExportInput {
    format: String,
    #[serde(default)]
    pipeline: Option<String>,
}

fn company_filter(company: Option<String>) -> Value {
    match company.filter(|c| !c.is_empty()) {
        Some(company) => json!({ "company": company }),
        None => json!({}),
    }
}

fn number(row: &Value, field: &str) -> f64 {
    row.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

fn do_funnel(input: PipelineInput) -> Result<Value> {
    require_user()?;
    let pipeline = require("crm.crm_pipeline", &input.pipeline, "pipeline")?;
    let pipeline_id = id_of(&pipeline)?.to_string();
    let report = funnel_report(&pipeline_id, false)?;
    let stages = report
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            json!({
                "stage": row.get("stage").cloned().unwrap_or(Value::Null),
                "name": row.get("name").cloned().unwrap_or(Value::Null),
                "count": number(&row, "n") as i64,
            })
        })
        .collect::<Vec<_>>();
    let total = report.get("total").map(|row| number(row, "n") as i64).unwrap_or(0);
    Ok(json!({ "pipeline": pipeline_id, "stages": stages, "total": total }))
}

fn funnel_report(pipeline_id: &str, preview: bool) -> Result<Value> {
    report::query(&json!({
        "model": "crm_deal.deal",
        "filter": { "pipeline": pipeline_id },
        "dimensions": ["stage"],
        "measures": { "n": "count" },
        "fill": {
            "model": "crm.crm_stage",
            "key": "id",
            "label": "name",
            "filter": { "pipeline": pipeline_id }
        },
        "preview": preview
    }))
}

fn do_conversion_rates(input: CompanyFilter) -> Result<Value> {
    require_user()?;
    let filter = company_filter(input.company);
    let leads = report::query(&json!({
        "model": "crm_lead.lead",
        "filter": filter.clone(),
        "measures": {
            "leads": "count",
            "converted": { "count_if": { "converted_deal": { "null": false } } }
        }
    }))?;
    let deals = report::query(&json!({
        "model": "crm_deal.deal",
        "filter": filter,
        "measures": {
            "deals": "count",
            "won": { "count_if": { "closed_at": { "null": false }, "lost_reason": { "null": true } } },
            "lost": { "count_if": { "lost_reason": { "null": false } } }
        }
    }))?;
    let lead_total = leads.get("total").cloned().unwrap_or(json!({}));
    let deal_total = deals.get("total").cloned().unwrap_or(json!({}));
    let lead_n = number(&lead_total, "leads");
    let converted = number(&lead_total, "converted");
    let won = number(&deal_total, "won");
    let lost = number(&deal_total, "lost");
    let lead_to_deal_pct = if lead_n == 0.0 { 0.0 } else { converted * 100.0 / lead_n };
    let closed = won + lost;
    let win_pct = if closed == 0.0 { 0.0 } else { won * 100.0 / closed };
    Ok(json!({
        "leads": lead_n as i64,
        "converted": converted as i64,
        "lead_to_deal_pct": lead_to_deal_pct,
        "deals": number(&deal_total, "deals") as i64,
        "won": won as i64,
        "lost": lost as i64,
        "win_pct": win_pct,
    }))
}

/// Both timestamps are not on the lead, so this is not an `avg_days` and it does not scan rows.
fn do_days_to_convert(_input: CompanyFilter) -> Result<Value> {
    require_user()?;
    Ok(json!({
        "samples": 0,
        "avg_days": Value::Null,
        "max_days": Value::Null,
        "pending": "Convert duration needs a timestamp on the lead. It is not computed from a row scan."
    }))
}

fn do_deadline_slip(input: CompanyFilter) -> Result<Value> {
    require_user()?;
    let today = context::current()?.today().to_string();
    let mut filter = company_filter(input.company);
    if let Value::Object(object) = &mut filter {
        object.insert("closed_at".into(), json!({ "null": true }));
        object.insert("expected_close".into(), json!({ "lt": today }));
    }
    let report = report::query(&json!({
        "model": "crm_deal.deal",
        "filter": filter,
        "measures": {
            "open_past_close": "count",
            "avg_slip_days": { "avg_days": { "from": "expected_close", "to": "now" } }
        }
    }))?;
    let total = report.get("total").cloned().unwrap_or(json!({}));
    Ok(json!({
        "open_past_close": number(&total, "open_past_close") as i64,
        "avg_slip_days": total.get("avg_slip_days").cloned().unwrap_or(Value::Null),
    }))
}

fn first_pipeline() -> Result<Option<String>> {
    let rows = db::find::<Record>("crm.crm_pipeline").limit(1).all()?;
    Ok(rows.first().and_then(|row| id_of(row).ok()).map(str::to_string))
}

fn do_reports_dashboard(input: CompanyFilter) -> Result<Value> {
    require_user()?;
    let conversion = do_conversion_rates(CompanyFilter { company: input.company.clone() })?;
    let slip = do_deadline_slip(CompanyFilter { company: input.company.clone() })?;
    let Some(pipeline_id) = first_pipeline()? else {
        return Ok(json!({
            "funnel": [],
            "preview_note": "No pipeline",
            "leads": conversion["leads"],
            "lead_to_deal_pct": conversion["lead_to_deal_pct"],
            "win_pct": conversion["win_pct"],
            "open_past_close": slip["open_past_close"],
            "avg_slip_days": slip["avg_slip_days"],
        }));
    };
    let report = funnel_report(&pipeline_id, true)?;
    let shown = number(&report, "shown") as i64;
    let total_groups = number(&report, "total_groups") as i64;
    let funnel = report
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            json!({
                "name": row.get("name").cloned().unwrap_or_else(|| row.get("stage").cloned().unwrap_or(Value::Null)),
                "value": number(&row, "n"),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "funnel": funnel,
        "preview_note": format!("Showing {shown} of {total_groups}"),
        "leads": conversion["leads"],
        "converted": conversion["converted"],
        "lead_to_deal_pct": conversion["lead_to_deal_pct"],
        "won": conversion["won"],
        "lost": conversion["lost"],
        "win_pct": conversion["win_pct"],
        "open_past_close": slip["open_past_close"],
        "avg_slip_days": slip["avg_slip_days"],
    }))
}

fn do_export_report(input: ExportInput) -> Result<Value> {
    require_user()?;
    let format = match input.format.as_str() {
        "pdf" => "pdf",
        _ => "xlsx",
    };
    let pipeline_id = match input.pipeline.filter(|id| !id.is_empty()) {
        Some(id) => Some(id),
        None => first_pipeline()?,
    };
    let Some(pipeline_id) = pipeline_id else {
        return report::export(&json!({
            "format": format,
            "title": "Funnel",
            "columns": [{ "field": "name", "label": "Stage" }, { "field": "n", "label": "Deals" }],
            "rows": []
        }));
    };
    let report = funnel_report(&pipeline_id, false)?;
    let mut rows = report.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    if let Some(total) = report.get("total") {
        rows.push(json!({
            "name": "Total",
            "n": total.get("n").cloned().unwrap_or(Value::from(0)),
        }));
    }
    report::export(&json!({
        "format": format,
        "title": "Funnel",
        "columns": [{ "field": "name", "label": "Stage" }, { "field": "n", "label": "Deals" }],
        "rows": rows
    }))
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
    fn reports_dashboard(input: CompanyFilter) -> Value { do_reports_dashboard(input) }
    fn export_report(input: ExportInput) -> Value { do_export_report(input) }
    fn save_funnel(input: SaveFunnel) -> Record { do_save_funnel(input) }
    fn list_funnels(input: Empty) -> Vec<Record> { do_list_funnels(input) }
    fn get_funnel(input: Id) -> Record { do_get_funnel(input) }
}

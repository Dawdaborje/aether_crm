//! Inbound forms → lead or deal.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_admin, require_user, text, Record};

#[derive(Deserialize)]
struct CreateForm {
    key: String,
    name: String,
    company: String,
    #[serde(default)]
    create_mode: Option<String>,
    #[serde(default)]
    default_stage: Option<String>,
    #[serde(default)]
    default_pipeline: Option<String>,
    #[serde(default)]
    default_source: Option<String>,
    #[serde(default)]
    field_map: Option<String>,
}

#[derive(Deserialize)]
struct SetForm {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    create_mode: Option<String>,
    #[serde(default)]
    default_stage: Option<String>,
    #[serde(default)]
    default_pipeline: Option<String>,
    #[serde(default)]
    default_source: Option<String>,
    #[serde(default)]
    field_map: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct Submit {
    form: String,
    #[serde(default)]
    fields: Option<Value>,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

fn mode_of(value: Option<&str>) -> Result<String> {
    match value.unwrap_or("lead") {
        "lead" | "deal" => Ok(value.unwrap_or("lead").to_string()),
        _ => Err(Error::msg("create_mode is lead or deal")),
    }
}

fn parse_map(raw: Option<&str>) -> Result<serde_json::Map<String, Value>> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(serde_json::Map::new());
    };
    let value: Value = serde_json::from_str(raw).map_err(|_| Error::msg("field_map must be JSON"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| Error::msg("field_map must be a JSON object"))
}

fn mapped_payload(form: &Record, inbound: &Value) -> Result<serde_json::Map<String, Value>> {
    let map = parse_map(text(form, "field_map"))?;
    let inbound_obj = inbound.as_object().cloned().unwrap_or_default();
    let mut out = serde_json::Map::new();
    if map.is_empty() {
        // Pass through known keys.
        for (k, v) in inbound_obj {
            out.insert(k, v);
        }
        return Ok(out);
    }
    for (from, to) in map {
        let Some(target) = to.as_str() else { continue };
        if let Some(value) = inbound_obj.get(&from) {
            out.insert(target.to_string(), value.clone());
        }
    }
    Ok(out)
}

fn do_create_form(input: CreateForm) -> Result<Record> {
    require_admin()?;
    let key = input.key.trim();
    let name = input.name.trim();
    if key.is_empty() || name.is_empty() {
        return Err(Error::msg("key and name are required"));
    }
    let company = require("company.company", &input.company, "company")?;
    let mut data = json!({
        "key": key,
        "name": name,
        "company": id_of(&company)?,
        "create_mode": mode_of(input.create_mode.as_deref())?,
        "field_map": input.field_map.unwrap_or_else(|| "{}".into()),
        "active": true,
    });
    if let Some(stage) = input.default_stage.filter(|s| !s.is_empty()) {
        data["default_stage"] = json!(id_of(&require("crm.crm_stage", &stage, "stage")?)?);
    }
    if let Some(pipeline) = input.default_pipeline.filter(|s| !s.is_empty()) {
        data["default_pipeline"] = json!(id_of(&require("crm.crm_pipeline", &pipeline, "pipeline")?)?);
    }
    if let Some(source) = input.default_source.filter(|s| !s.is_empty()) {
        data["default_source"] = json!(id_of(&require("crm.crm_source", &source, "source")?)?);
    }
    db::create("intake_form", &data).map_err(|e| e.or("could not create the form"))
}

fn do_set_form(input: SetForm) -> Result<Record> {
    require_admin()?;
    let _ = require("intake_form", &input.id, "form")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::msg("name is required"));
        }
        changes["name"] = json!(name);
    }
    if let Some(mode) = input.create_mode {
        changes["create_mode"] = json!(mode_of(Some(&mode))?);
    }
    if let Some(field_map) = input.field_map {
        let _ = parse_map(Some(&field_map))?;
        changes["field_map"] = json!(field_map);
    }
    if let Some(stage) = input.default_stage {
        changes["default_stage"] = if stage.is_empty() {
            Value::Null
        } else {
            json!(id_of(&require("crm.crm_stage", &stage, "stage")?)?)
        };
    }
    if let Some(pipeline) = input.default_pipeline {
        changes["default_pipeline"] = if pipeline.is_empty() {
            Value::Null
        } else {
            json!(id_of(&require("crm.crm_pipeline", &pipeline, "pipeline")?)?)
        };
    }
    if let Some(source) = input.default_source {
        changes["default_source"] = if source.is_empty() {
            Value::Null
        } else {
            json!(id_of(&require("crm.crm_source", &source, "source")?)?)
        };
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("intake_form", &input.id, &changes)?.ok_or_else(|| Error::msg("the form is gone"))
}

fn resolve_form(key_or_id: &str) -> Result<Record> {
    if let Ok(row) = require("intake_form", key_or_id, "form") {
        return Ok(row);
    }
    db::find::<Record>("intake_form")
        .filter("key", key_or_id)
        .first()?
        .ok_or_else(|| Error::msg("form not found"))
}

fn do_submit(input: Submit) -> Result<Record> {
    // Public-ish: allow crm_user or admin (website bridge will call as a service user).
    require_user()?;
    let form = resolve_form(input.form.trim())?;
    if form.get("active") == Some(&json!(false)) {
        return Err(Error::msg("this form is inactive"));
    }
    let mode = text(&form, "create_mode").unwrap_or("lead");
    let mut payload = mapped_payload(&form, input.fields.as_ref().unwrap_or(&json!({})))?;
    payload.insert("company".into(), json!(text(&form, "company").unwrap_or_default()));
    if let Some(source) = text(&form, "default_source") {
        payload.entry("source").or_insert(json!(source));
    }
    if mode == "lead" {
        let stage = text(&form, "default_stage").ok_or_else(|| Error::msg("form needs default_stage for lead mode"))?;
        payload.entry("stage").or_insert(json!(stage));
        if !payload.contains_key("title") {
            let title = payload
                .get("person_name")
                .or_else(|| payload.get("org_name"))
                .and_then(Value::as_str)
                .unwrap_or("Website lead");
            payload.insert("title".into(), json!(title));
        }
        let lead: Record = plugins::call("crm_lead", "create_lead", &Value::Object(payload))?;
        let _ = events::emit("intake_submitted", &json!({ "form": id_of(&form)?, "lead": id_of(&lead)?, "mode": "lead" }));
        Ok(lead)
    } else {
        if let Some(pipeline) = text(&form, "default_pipeline") {
            payload.entry("pipeline").or_insert(json!(pipeline));
        }
        if let Some(stage) = text(&form, "default_stage") {
            payload.entry("stage").or_insert(json!(stage));
        }
        if !payload.contains_key("title") {
            payload.insert("title".into(), json!("Website deal"));
        }
        let deal: Record = plugins::call("crm_deal", "create_deal", &Value::Object(payload))?;
        let _ = events::emit("intake_submitted", &json!({ "form": id_of(&form)?, "deal": id_of(&deal)?, "mode": "deal" }));
        Ok(deal)
    }
}

fn do_get_form(input: Id) -> Result<Record> {
    require_user()?;
    resolve_form(&input.id)
}

fn do_list_forms(_input: Empty) -> Result<Vec<Record>> {
    require_user()?;
    db::find::<Record>("intake_form").limit(200).all()
}

handler! {
    fn create_form(input: CreateForm) -> Record { do_create_form(input) }
    fn set_form(input: SetForm) -> Record { do_set_form(input) }
    fn submit(input: Submit) -> Record { do_submit(input) }
    fn get_form(input: Id) -> Record { do_get_form(input) }
    fn list_forms(input: Empty) -> Vec<Record> { do_list_forms(input) }
}

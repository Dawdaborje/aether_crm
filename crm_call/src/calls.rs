//! Phone call records linked to parties, leads and deals.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record, DIRECTIONS, STATUSES};

#[derive(Deserialize)]
struct LogCall {
    direction: String,
    #[serde(default)]
    from_number: Option<String>,
    #[serde(default)]
    to_number: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    party: Option<String>,
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    duration_sec: Option<i64>,
    #[serde(default)]
    external_id: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Deserialize)]
struct SetCall {
    id: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    duration_sec: Option<i64>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    party: Option<String>,
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct ListCalls {
    #[serde(default)]
    party: Option<String>,
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    external_id: Option<String>,
}

fn one_of(value: Option<&str>, allowed: &[&str], fallback: &str, label: &str) -> Result<String> {
    let value = value.unwrap_or(fallback);
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(Error::msg(format!("{label} is {}", allowed.join(", "))))
    }
}

fn assert_subject(kind: Option<&str>, id: Option<&str>) -> Result<(Option<String>, Option<String>)> {
    match (kind.filter(|k| !k.is_empty()), id.filter(|i| !i.is_empty())) {
        (None, None) => Ok((None, None)),
        (Some(kind), Some(id)) => {
            match kind {
                "lead" => {
                    require("crm_lead.lead", id, "lead")?;
                }
                "deal" => {
                    require("crm_deal.deal", id, "deal")?;
                }
                _ => return Err(Error::msg("subject_kind is lead or deal")),
            }
            Ok((Some(kind.to_string()), Some(id.to_string())))
        }
        _ => Err(Error::msg("subject_kind and subject_id go together")),
    }
}

fn do_log_call(input: LogCall) -> Result<Record> {
    require_user()?;
    let direction = one_of(Some(input.direction.as_str()), DIRECTIONS, "inbound", "direction")?;
    let status = one_of(input.status.as_deref(), STATUSES, "completed", "status")?;
    let (subject_kind, subject_id) = assert_subject(input.subject_kind.as_deref(), input.subject_id.as_deref())?;
    let ctx = context::current()?;
    let mut data = json!({
        "direction": direction,
        "status": status,
        "started_at": input.started_at.filter(|s| !s.is_empty()).unwrap_or(ctx.now),
        "duration_sec": input.duration_sec.unwrap_or(0).max(0),
        "from_number": input.from_number.unwrap_or_default().trim(),
        "to_number": input.to_number.unwrap_or_default().trim(),
        "notes": input.notes.unwrap_or_default(),
    });
    if let Some(party) = input.party.filter(|p| !p.is_empty()) {
        let row = require("party.party", &party, "party")?;
        data["party"] = json!(id_of(&row)?);
    }
    if let Some(kind) = subject_kind {
        data["subject_kind"] = json!(kind);
        data["subject_id"] = json!(subject_id.unwrap());
    }
    if let Some(external_id) = input.external_id.filter(|e| !e.trim().is_empty()) {
        let external_id = external_id.trim();
        if let Some(existing) = db::find::<Record>("call_log").filter("external_id", external_id).first()? {
            return Ok(existing);
        }
        data["external_id"] = json!(external_id);
    }
    let call = db::create("call_log", &data).map_err(|e| e.or("could not log the call"))?;
    let _ = events::emit("call_logged", &json!({ "id": id_of(&call)? }));
    Ok(call)
}

fn do_set_call(input: SetCall) -> Result<Record> {
    require_user()?;
    let _ = require("call_log", &input.id, "call")?;
    let mut changes = json!({});
    if let Some(status) = input.status {
        changes["status"] = json!(one_of(Some(&status), STATUSES, "completed", "status")?);
    }
    if let Some(duration) = input.duration_sec {
        changes["duration_sec"] = json!(duration.max(0));
    }
    if let Some(notes) = input.notes {
        changes["notes"] = json!(notes);
    }
    if let Some(party) = input.party {
        if party.is_empty() {
            changes["party"] = Value::Null;
        } else {
            let row = require("party.party", &party, "party")?;
            changes["party"] = json!(id_of(&row)?);
        }
    }
    if input.subject_kind.is_some() || input.subject_id.is_some() {
        let (kind, id) = assert_subject(input.subject_kind.as_deref(), input.subject_id.as_deref())?;
        match (kind, id) {
            (None, None) => {
                changes["subject_kind"] = Value::Null;
                changes["subject_id"] = Value::Null;
            }
            (Some(kind), Some(id)) => {
                changes["subject_kind"] = json!(kind);
                changes["subject_id"] = json!(id);
            }
            _ => {}
        }
    }
    db::update("call_log", &input.id, &changes)?.ok_or_else(|| Error::msg("the call is gone"))
}

fn do_get_call(input: Id) -> Result<Record> {
    require_user()?;
    require("call_log", &input.id, "call")
}

fn do_list_calls(input: ListCalls) -> Result<Vec<Record>> {
    require_user()?;
    let mut rows = db::find::<Record>("call_log").limit(500).all()?;
    if let Some(party) = input.party.filter(|p| !p.is_empty()) {
        rows.retain(|r| text(r, "party") == Some(party.as_str()));
    }
    if let Some(kind) = input.subject_kind.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "subject_kind") == Some(kind.as_str()));
    }
    if let Some(id) = input.subject_id.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "subject_id") == Some(id.as_str()));
    }
    if let Some(external_id) = input.external_id.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "external_id") == Some(external_id.as_str()));
    }
    Ok(rows)
}

handler! {
    fn log_call(input: LogCall) -> Record { do_log_call(input) }
    fn set_call(input: SetCall) -> Record { do_set_call(input) }
    fn get_call(input: Id) -> Record { do_get_call(input) }
    fn list_calls(input: ListCalls) -> Vec<Record> { do_list_calls(input) }
}

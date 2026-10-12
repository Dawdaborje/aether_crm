//! Communication threads on leads and deals (kernel message ids as refs).

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record, CHANNELS};

#[derive(Deserialize)]
struct OpenThread {
    subject_kind: String,
    subject_id: String,
    channel: String,
    #[serde(default)]
    external_thread: Option<String>,
}

#[derive(Deserialize)]
struct AppendMessage {
    thread: String,
    message_id: String,
    #[serde(default)]
    at: Option<String>,
    /// When true, ask crm_sla to mark first response on the subject.
    #[serde(default)]
    counts_as_response: Option<bool>,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct ListThreads {
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    active_only: Option<bool>,
}

fn one_of(value: &str, allowed: &[&str], label: &str) -> Result<String> {
    if allowed.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(Error::msg(format!("{label} is {}", allowed.join(", "))))
    }
}

fn assert_subject(kind: &str, id: &str) -> Result<()> {
    match kind {
        "lead" => {
            require("crm_lead.lead", id, "lead")?;
        }
        "deal" => {
            require("crm_deal.deal", id, "deal")?;
        }
        _ => return Err(Error::msg("subject_kind is lead or deal")),
    }
    Ok(())
}

fn parse_ids(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(ids) => ids,
        Err(_) => Vec::new(),
    }
}

fn do_open_thread(input: OpenThread) -> Result<Record> {
    require_user()?;
    let kind = one_of(input.subject_kind.as_str(), &["lead", "deal"], "subject_kind")?;
    let subject_id = input.subject_id.trim();
    if subject_id.is_empty() {
        return Err(Error::msg("subject_id is required"));
    }
    assert_subject(&kind, subject_id)?;
    let channel = one_of(input.channel.as_str(), CHANNELS, "channel")?;
    let external = input.external_thread.as_deref().unwrap_or("").trim();

    if !external.is_empty() {
        if let Some(existing) = db::find::<Record>("comms_thread")
            .filter("external_thread", external)
            .first()?
        {
            return Ok(existing);
        }
    } else {
        let existing = db::find::<Record>("comms_thread")
            .filter("subject_kind", &kind)
            .filter("subject_id", subject_id)
            .limit(50)
            .all()?;
        if let Some(found) = existing.into_iter().find(|row| {
            text(row, "channel") == Some(channel.as_str()) && row.get("active") != Some(&json!(false))
        }) {
            return Ok(found);
        }
    }

    let data = json!({
        "subject_kind": kind,
        "subject_id": subject_id,
        "channel": channel,
        "external_thread": external,
        "message_ids": "[]",
        "active": true,
    });
    let thread = db::create("comms_thread", &data).map_err(|e| e.or("could not open the thread"))?;
    let _ = events::emit("thread_opened", &json!({ "id": id_of(&thread)? }));
    Ok(thread)
}

fn do_append_message(input: AppendMessage) -> Result<Record> {
    require_user()?;
    let thread = require("comms_thread", &input.thread, "thread")?;
    let message_id = input.message_id.trim();
    if message_id.is_empty() {
        return Err(Error::msg("message_id is required"));
    }
    let mut ids = parse_ids(text(&thread, "message_ids"));
    if !ids.iter().any(|id| id == message_id) {
        ids.push(message_id.to_string());
    }
    let ctx = context::current()?;
    let at = input.at.filter(|a| !a.is_empty()).unwrap_or(ctx.now);
    let updated = db::update(
        "comms_thread",
        &input.thread,
        &json!({
            "message_ids": serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into()),
            "last_message_at": at,
        }),
    )?
    .ok_or_else(|| Error::msg("the thread is gone"))?;

    if input.counts_as_response.unwrap_or(false) {
        if let (Some(kind), Some(subject_id)) = (text(&updated, "subject_kind"), text(&updated, "subject_id")) {
            let _: Result<Value> = plugins::call(
                "crm_sla",
                "mark_responded",
                &json!({ "subject_kind": kind, "subject_id": subject_id }),
            );
        }
    }
    let _ = events::emit(
        "message_appended",
        &json!({ "thread": input.thread, "message_id": message_id }),
    );
    Ok(updated)
}

fn do_close_thread(input: Id) -> Result<Record> {
    require_user()?;
    let _ = require("comms_thread", &input.id, "thread")?;
    db::update("comms_thread", &input.id, &json!({ "active": false }))?.ok_or_else(|| Error::msg("the thread is gone"))
}

fn do_get_thread(input: Id) -> Result<Record> {
    require_user()?;
    require("comms_thread", &input.id, "thread")
}

fn do_list_threads(input: ListThreads) -> Result<Vec<Record>> {
    require_user()?;
    let mut rows = db::find::<Record>("comms_thread").limit(500).all()?;
    if let Some(kind) = input.subject_kind.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "subject_kind") == Some(kind.as_str()));
    }
    if let Some(id) = input.subject_id.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "subject_id") == Some(id.as_str()));
    }
    if let Some(channel) = input.channel.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "channel") == Some(channel.as_str()));
    }
    if input.active_only.unwrap_or(true) {
        rows.retain(|r| r.get("active") != Some(&json!(false)));
    }
    Ok(rows)
}

handler! {
    fn open_thread(input: OpenThread) -> Record { do_open_thread(input) }
    fn append_message(input: AppendMessage) -> Record { do_append_message(input) }
    fn close_thread(input: Id) -> Record { do_close_thread(input) }
    fn get_thread(input: Id) -> Record { do_get_thread(input) }
    fn list_threads(input: ListThreads) -> Vec<Record> { do_list_threads(input) }
}

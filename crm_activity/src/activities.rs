//! Activities on leads and deals.

use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_user, text, Record, KINDS, PRIORITIES, STATES};

#[derive(Deserialize)]
struct CreateActivity {
    title: String,
    subject_kind: String,
    subject_id: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    due_at: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct SetActivity {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    due_at: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct ListInput {
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    open_only: Option<bool>,
}

fn one_of(value: Option<&str>, allowed: &[&str], fallback: &str, label: &str) -> Result<String> {
    let value = value.unwrap_or(fallback);
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

fn do_create_activity(input: CreateActivity) -> Result<Record> {
    require_user()?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err(Error::msg("title is required"));
    }
    let subject_kind = one_of(Some(input.subject_kind.as_str()), &["lead", "deal"], "lead", "subject_kind")?;
    assert_subject(&subject_kind, input.subject_id.trim())?;
    let mut data = json!({
        "title": title,
        "kind": one_of(input.kind.as_deref(), KINDS, "task", "kind")?,
        "subject_kind": subject_kind,
        "subject_id": input.subject_id.trim(),
        "assignee": input.assignee.unwrap_or_default().trim(),
        "state": "todo",
        "priority": one_of(input.priority.as_deref(), PRIORITIES, "medium", "priority")?,
        "description": input.description.unwrap_or_default(),
    });
    if let Some(due_at) = input.due_at.filter(|d| !d.is_empty()) {
        data["due_at"] = json!(due_at);
    }
    db::create("activity", &data).map_err(|e| e.or("could not create the activity"))
}

fn do_set_activity(input: SetActivity) -> Result<Record> {
    require_user()?;
    let _ = require("activity", &input.id, "activity")?;
    let mut changes = json!({});
    if let Some(title) = input.title {
        let title = title.trim();
        if title.is_empty() {
            return Err(Error::msg("title is required"));
        }
        changes["title"] = json!(title);
    }
    if let Some(assignee) = input.assignee {
        changes["assignee"] = json!(assignee.trim());
    }
    if let Some(due_at) = input.due_at {
        changes["due_at"] = if due_at.is_empty() { Value::Null } else { json!(due_at) };
    }
    if let Some(priority) = input.priority {
        changes["priority"] = json!(one_of(Some(&priority), PRIORITIES, "medium", "priority")?);
    }
    if let Some(description) = input.description {
        changes["description"] = json!(description);
    }
    if let Some(state) = input.state {
        changes["state"] = json!(one_of(Some(&state), STATES, "todo", "state")?);
    }
    db::update("activity", &input.id, &changes)?.ok_or_else(|| Error::msg("the activity is gone"))
}

fn do_complete(input: Id) -> Result<Record> {
    require_user()?;
    let _ = require("activity", &input.id, "activity")?;
    db::update("activity", &input.id, &json!({ "state": "done" }))?.ok_or_else(|| Error::msg("the activity is gone"))
}

fn do_cancel(input: Id) -> Result<Record> {
    require_user()?;
    let _ = require("activity", &input.id, "activity")?;
    db::update("activity", &input.id, &json!({ "state": "cancelled" }))?.ok_or_else(|| Error::msg("the activity is gone"))
}

fn do_list_activities(input: ListInput) -> Result<Vec<Record>> {
    require_user()?;
    let mut rows = db::find::<Record>("activity").limit(500).all()?;
    if let Some(kind) = input.subject_kind.filter(|k| !k.is_empty()) {
        rows.retain(|row| text(row, "subject_kind") == Some(kind.as_str()));
    }
    if let Some(id) = input.subject_id.filter(|k| !k.is_empty()) {
        rows.retain(|row| text(row, "subject_id") == Some(id.as_str()));
    }
    if let Some(assignee) = input.assignee.filter(|k| !k.is_empty()) {
        rows.retain(|row| text(row, "assignee") == Some(assignee.as_str()));
    }
    if input.open_only.unwrap_or(false) {
        rows.retain(|row| !matches!(text(row, "state"), Some("done" | "cancelled")));
    }
    Ok(rows)
}

fn do_activity_nightly(_input: Empty) -> Result<Value> {
    let ctx = context::current()?;
    let now = ctx.now.clone();
    let rows = db::find::<Record>("activity").limit(1000).all()?;
    let mut n = 0;
    for row in rows {
        let state = text(&row, "state").unwrap_or("todo");
        if matches!(state, "done" | "cancelled" | "backlog") {
            continue;
        }
        let Some(due) = text(&row, "due_at") else { continue };
        if due < now.as_str() {
            let _ = events::emit("activity_overdue", &json!({ "id": id_of(&row)? }));
            n += 1;
        }
    }
    Ok(json!({ "overdue": n }))
}

handler! {
    fn create_activity(input: CreateActivity) -> Record { do_create_activity(input) }
    fn set_activity(input: SetActivity) -> Record { do_set_activity(input) }
    fn complete(input: Id) -> Record { do_complete(input) }
    fn cancel(input: Id) -> Record { do_cancel(input) }
    fn list_activities(input: ListInput) -> Vec<Record> { do_list_activities(input) }
    fn activity_nightly(input: Empty) -> Value { do_activity_nightly(input) }
}

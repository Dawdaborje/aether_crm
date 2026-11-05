//! SLA policies and first-response clocks.

use aether_sdk::decimal::Decimal;
use aether_sdk::events::Received;
use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_admin, require_user, text, Record, APPLY_ON, PRIORITIES};
use crate::deadline::response_deadline;

#[derive(Deserialize)]
struct CreatePolicy {
    code: String,
    name: String,
    #[serde(default)]
    apply_on: Option<String>,
    first_response_hours: Decimal,
    #[serde(default)]
    calendar: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    priority: Option<String>,
}

#[derive(Deserialize)]
struct SetPolicy {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    apply_on: Option<String>,
    #[serde(default)]
    first_response_hours: Option<Decimal>,
    #[serde(default)]
    calendar: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct StartClock {
    subject_kind: String,
    subject_id: String,
    #[serde(default)]
    company: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    policy: Option<String>,
}

#[derive(Deserialize)]
struct Subject {
    subject_kind: String,
    subject_id: String,
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct ListClocks {
    #[serde(default)]
    subject_kind: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
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

fn assert_subject(kind: &str, id: &str) -> Result<(Option<String>, Option<String>, Option<String>)> {
    match kind {
        "lead" => {
            let lead = require("crm_lead.lead", id, "lead")?;
            Ok((
                text(&lead, "company").map(str::to_string),
                text(&lead, "source").map(str::to_string),
                text(&lead, "team").map(str::to_string),
            ))
        }
        "deal" => {
            let deal = require("crm_deal.deal", id, "deal")?;
            Ok((
                text(&deal, "company").map(str::to_string),
                text(&deal, "source").map(str::to_string),
                text(&deal, "team").map(str::to_string),
            ))
        }
        _ => Err(Error::msg("subject_kind is lead or deal")),
    }
}

fn hours_of(policy: &Record) -> Result<Decimal> {
    match policy.get("first_response_hours") {
        Some(Value::String(s)) => Decimal::parse(s),
        Some(other) => Decimal::parse(&other.to_string().trim_matches('"')),
        None => Err(Error::msg("policy has no first_response_hours")),
    }
}

fn pick_policy(kind: &str, source: Option<&str>, team: Option<&str>, priority: Option<&str>, explicit: Option<&str>) -> Result<Record> {
    if let Some(id) = explicit.filter(|p| !p.is_empty()) {
        return require("sla_policy", id, "policy");
    }
    let policies = db::find::<Record>("sla_policy").limit(200).all()?;
    let mut best: Option<(i32, Record)> = None;
    for policy in policies {
        if policy.get("active") == Some(&json!(false)) {
            continue;
        }
        let apply = text(&policy, "apply_on").unwrap_or("both");
        if apply != "both" && apply != kind {
            continue;
        }
        let mut score = 0;
        if let Some(want) = text(&policy, "source") {
            if source != Some(want) {
                continue;
            }
            score += 4;
        }
        if let Some(want) = text(&policy, "team") {
            if team != Some(want) {
                continue;
            }
            score += 2;
        }
        if let Some(want) = text(&policy, "priority") {
            if priority != Some(want) {
                continue;
            }
            score += 1;
        }
        match &best {
            None => best = Some((score, policy)),
            Some((best_score, _)) if score > *best_score => best = Some((score, policy)),
            _ => {}
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| Error::msg("no matching SLA policy"))
}

fn open_clock_for(input: StartClock) -> Result<Record> {
    let kind = one_of(Some(input.subject_kind.as_str()), &["lead", "deal"], "lead", "subject_kind")?;
    let subject_id = input.subject_id.trim();
    if subject_id.is_empty() {
        return Err(Error::msg("subject_id is required"));
    }
    let (company_from_subject, source_from_subject, team_from_subject) = assert_subject(&kind, subject_id)?;

    if let Some(existing) = db::find::<Record>("sla_clock")
        .filter("subject_kind", &kind)
        .filter("subject_id", subject_id)
        .limit(20)
        .all()?
        .into_iter()
        .find(|row| text(row, "status") == Some("open"))
    {
        return Ok(existing);
    }

    let source = input.source.as_deref().or(source_from_subject.as_deref());
    let team = input.team.as_deref().or(team_from_subject.as_deref());
    let priority = input.priority.as_deref();
    let policy = pick_policy(&kind, source, team, priority, input.policy.as_deref())?;
    let hours = hours_of(&policy)?;
    let ctx = context::current()?;
    let response_by = response_deadline(ctx.now.as_str(), &hours, text(&policy, "calendar"))?;
    let data = json!({
        "subject_kind": kind,
        "subject_id": subject_id,
        "policy": id_of(&policy)?,
        "status": "open",
        "opened_at": ctx.now,
        "response_by": response_by,
        "company": input.company.or(company_from_subject).unwrap_or_default(),
    });
    db::create("sla_clock", &data).map_err(|e| e.or("could not open the SLA clock"))
}

fn do_create_policy(input: CreatePolicy) -> Result<Record> {
    require_admin()?;
    let code = input.code.trim();
    let name = input.name.trim();
    if code.is_empty() || name.is_empty() {
        return Err(Error::msg("code and name are required"));
    }
    if input.first_response_hours.is_negative() {
        return Err(Error::msg("first_response_hours must be zero or more"));
    }
    let mut data = json!({
        "code": code,
        "name": name,
        "apply_on": one_of(input.apply_on.as_deref(), APPLY_ON, "lead", "apply_on")?,
        "first_response_hours": input.first_response_hours,
        "active": true,
    });
    if let Some(calendar) = input.calendar.filter(|c| !c.is_empty()) {
        let row = require("calendar.holiday_calendar", &calendar, "calendar")?;
        data["calendar"] = json!(id_of(&row)?);
    }
    if let Some(source) = input.source.filter(|s| !s.is_empty()) {
        let row = require("crm.crm_source", &source, "source")?;
        data["source"] = json!(id_of(&row)?);
    }
    if let Some(team) = input.team.filter(|t| !t.is_empty()) {
        let row = require("crm.crm_team", &team, "team")?;
        data["team"] = json!(id_of(&row)?);
    }
    if let Some(priority) = input.priority.filter(|p| !p.is_empty()) {
        data["priority"] = json!(one_of(Some(&priority), PRIORITIES, "medium", "priority")?);
    }
    db::create("sla_policy", &data).map_err(|e| e.or("could not create the policy"))
}

fn do_set_policy(input: SetPolicy) -> Result<Record> {
    require_admin()?;
    let _ = require("sla_policy", &input.id, "policy")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::msg("name is required"));
        }
        changes["name"] = json!(name);
    }
    if let Some(apply_on) = input.apply_on {
        changes["apply_on"] = json!(one_of(Some(&apply_on), APPLY_ON, "lead", "apply_on")?);
    }
    if let Some(hours) = input.first_response_hours {
        if hours.is_negative() {
            return Err(Error::msg("first_response_hours must be zero or more"));
        }
        changes["first_response_hours"] = json!(hours);
    }
    if let Some(calendar) = input.calendar {
        if calendar.is_empty() {
            changes["calendar"] = Value::Null;
        } else {
            let row = require("calendar.holiday_calendar", &calendar, "calendar")?;
            changes["calendar"] = json!(id_of(&row)?);
        }
    }
    if let Some(source) = input.source {
        if source.is_empty() {
            changes["source"] = Value::Null;
        } else {
            let row = require("crm.crm_source", &source, "source")?;
            changes["source"] = json!(id_of(&row)?);
        }
    }
    if let Some(team) = input.team {
        if team.is_empty() {
            changes["team"] = Value::Null;
        } else {
            let row = require("crm.crm_team", &team, "team")?;
            changes["team"] = json!(id_of(&row)?);
        }
    }
    if let Some(priority) = input.priority {
        if priority.is_empty() {
            changes["priority"] = Value::Null;
        } else {
            changes["priority"] = json!(one_of(Some(&priority), PRIORITIES, "medium", "priority")?);
        }
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("sla_policy", &input.id, &changes)?.ok_or_else(|| Error::msg("the policy is gone"))
}

fn do_start_clock(input: StartClock) -> Result<Record> {
    require_user()?;
    open_clock_for(input)
}

fn do_mark_responded(input: Subject) -> Result<Record> {
    require_user()?;
    let kind = one_of(Some(input.subject_kind.as_str()), &["lead", "deal"], "lead", "subject_kind")?;
    let clocks = db::find::<Record>("sla_clock")
        .filter("subject_kind", &kind)
        .filter("subject_id", input.subject_id.trim())
        .limit(50)
        .all()?;
    let open = clocks
        .into_iter()
        .find(|row| text(row, "status") == Some("open"))
        .ok_or_else(|| Error::msg("no open SLA clock for this subject"))?;
    let ctx = context::current()?;
    db::update(
        "sla_clock",
        id_of(&open)?,
        &json!({ "status": "met", "first_responded_at": ctx.now }),
    )?
    .ok_or_else(|| Error::msg("the clock is gone"))
}

fn cancel_open_clocks(kind: &str, subject_id: &str) -> Result<i64> {
    let clocks = db::find::<Record>("sla_clock")
        .filter("subject_kind", kind)
        .filter("subject_id", subject_id)
        .limit(50)
        .all()?;
    let mut n = 0i64;
    for row in clocks {
        if text(&row, "status") == Some("open") {
            let _: Option<Record> = db::update("sla_clock", id_of(&row)?, &json!({ "status": "cancelled" }))?;
            n += 1;
        }
    }
    Ok(n)
}

fn do_cancel_clock(input: Subject) -> Result<Value> {
    require_user()?;
    let kind = one_of(Some(input.subject_kind.as_str()), &["lead", "deal"], "lead", "subject_kind")?;
    Ok(json!({ "cancelled": cancel_open_clocks(&kind, input.subject_id.trim())? }))
}

fn do_list_clocks(input: ListClocks) -> Result<Vec<Record>> {
    require_user()?;
    let mut rows = db::find::<Record>("sla_clock").limit(500).all()?;
    if let Some(kind) = input.subject_kind.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "subject_kind") == Some(kind.as_str()));
    }
    if let Some(id) = input.subject_id.filter(|k| !k.is_empty()) {
        rows.retain(|r| text(r, "subject_id") == Some(id.as_str()));
    }
    if input.open_only.unwrap_or(false) {
        rows.retain(|r| text(r, "status") == Some("open"));
    }
    Ok(rows)
}

fn do_get_policy(input: Id) -> Result<Record> {
    require_user()?;
    require("sla_policy", &input.id, "policy")
}

fn do_search_policies(_input: Empty) -> Result<Vec<Record>> {
    require_user()?;
    db::find::<Record>("sla_policy").limit(200).all()
}

fn do_sla_tick(_input: Empty) -> Result<Value> {
    let ctx = context::current()?;
    let now = ctx.now.clone();
    let rows = db::find::<Record>("sla_clock").limit(1000).all()?;
    let mut n = 0;
    for row in rows {
        if text(&row, "status") != Some("open") {
            continue;
        }
        let Some(by) = text(&row, "response_by") else { continue };
        if by < now.as_str() {
            let id = id_of(&row)?.to_string();
            let _: Option<Record> = db::update("sla_clock", &id, &json!({ "status": "breached" }))?;
            let _ = events::emit(
                "sla_breached",
                &json!({
                    "id": id,
                    "subject_kind": text(&row, "subject_kind"),
                    "subject_id": text(&row, "subject_id"),
                }),
            );
            n += 1;
        }
    }
    Ok(json!({ "breached": n }))
}

fn do_on_lead_created(received: Received) -> Result<Value> {
    let Some(id) = received.payload.get("id").and_then(Value::as_str) else {
        return Ok(json!({ "started": false }));
    };
    match open_clock_for(StartClock {
        subject_kind: "lead".into(),
        subject_id: id.into(),
        company: None,
        source: None,
        team: None,
        priority: None,
        policy: None,
    }) {
        Ok(clock) => Ok(json!({ "started": true, "id": id_of(&clock)? })),
        Err(err) if err.to_string().contains("no matching SLA policy") => Ok(json!({ "started": false })),
        Err(err) => Err(err),
    }
}

fn do_on_lead_converted(received: Received) -> Result<Value> {
    let lead = received.payload.get("lead").and_then(Value::as_str);
    let deal = received.payload.get("deal").and_then(Value::as_str);
    let mut cancelled = 0i64;
    if let Some(lead) = lead {
        cancelled = cancel_open_clocks("lead", lead)?;
    }
    let mut started = false;
    let mut clock_id = Value::Null;
    if let Some(deal) = deal {
        match open_clock_for(StartClock {
            subject_kind: "deal".into(),
            subject_id: deal.into(),
            company: None,
            source: None,
            team: None,
            priority: None,
            policy: None,
        }) {
            Ok(clock) => {
                started = true;
                clock_id = json!(id_of(&clock)?);
            }
            Err(err) if err.to_string().contains("no matching SLA policy") => {}
            Err(err) => return Err(err),
        }
    }
    Ok(json!({ "cancelled": cancelled, "started": started, "clock": clock_id }))
}

handler! {
    fn create_policy(input: CreatePolicy) -> Record { do_create_policy(input) }
    fn set_policy(input: SetPolicy) -> Record { do_set_policy(input) }
    fn get_policy(input: Id) -> Record { do_get_policy(input) }
    fn search_policies(input: Empty) -> Vec<Record> { do_search_policies(input) }
    fn start_clock(input: StartClock) -> Record { do_start_clock(input) }
    fn mark_responded(input: Subject) -> Record { do_mark_responded(input) }
    fn cancel_clock(input: Subject) -> Value { do_cancel_clock(input) }
    fn list_clocks(input: ListClocks) -> Vec<Record> { do_list_clocks(input) }
    fn sla_tick(input: Empty) -> Value { do_sla_tick(input) }
    fn on_lead_created(received: Received) -> Value { do_on_lead_created(received) }
    fn on_lead_converted(received: Received) -> Value { do_on_lead_converted(received) }
}

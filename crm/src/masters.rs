//! CRM masters: sources, industries, lost reasons, tags, territories, teams, pipelines, stages.

use aether_sdk::decimal::Decimal;
use aether_sdk::prelude::*;

use crate::common::{id_of, require, require_admin, require_reader, text, Record, STAGE_KINDS};

#[derive(Deserialize)]
struct CodeName {
    code: String,
    name: String,
}

#[derive(Deserialize)]
struct SetCodeName {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    requires_notes: Option<bool>,
    #[serde(default)]
    color: Option<String>,
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
    active_only: Option<bool>,
}

#[derive(Deserialize)]
struct TerritoryInput {
    code: String,
    name: String,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    manager: Option<String>,
}

#[derive(Deserialize)]
struct SetTerritory {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    manager: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct TeamInput {
    code: String,
    name: String,
    company: String,
    #[serde(default)]
    leader: Option<String>,
}

#[derive(Deserialize)]
struct SetTeam {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    leader: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct TeamMemberInput {
    team: String,
    user: String,
}

#[derive(Deserialize)]
struct PipelineInput {
    code: String,
    name: String,
    company: String,
}

#[derive(Deserialize)]
struct SetPipeline {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct StageInput {
    pipeline: String,
    name: String,
    #[serde(default)]
    sequence: Option<i64>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    default_probability: Option<Decimal>,
    #[serde(default)]
    fold: Option<bool>,
}

#[derive(Deserialize)]
struct SetStage {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    sequence: Option<i64>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    default_probability: Option<Decimal>,
    #[serde(default)]
    fold: Option<bool>,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Deserialize)]
struct CompanySettingsInput {
    company: String,
    #[serde(default)]
    use_leads: Option<bool>,
    #[serde(default)]
    default_pipeline: Option<String>,
    #[serde(default)]
    require_forecast_fields: Option<bool>,
}

#[derive(Deserialize)]
struct TagInput {
    name: String,
    #[serde(default)]
    color: Option<String>,
}

fn trim_code(code: &str) -> Result<String> {
    let code = code.trim();
    if code.is_empty() {
        return Err(Error::msg("code is required"));
    }
    Ok(code.to_string())
}

fn trim_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::msg("name is required"));
    }
    Ok(name.to_string())
}

fn kind_or(value: Option<&str>, fallback: &str) -> Result<String> {
    let kind = value.unwrap_or(fallback);
    if STAGE_KINDS.contains(&kind) {
        Ok(kind.to_string())
    } else {
        Err(Error::msg("kind is open, hold, won, lost or converted"))
    }
}

fn filter_search(mut rows: Vec<Record>, input: &Search) -> Vec<Record> {
    if input.active_only.unwrap_or(true) {
        rows.retain(|row| row.get("active") != Some(&json!(false)));
    }
    if let Some(q) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let q = q.to_lowercase();
        rows.retain(|row| {
            text(row, "code").map(|v| v.to_lowercase().contains(&q)).unwrap_or(false)
                || text(row, "name").map(|v| v.to_lowercase().contains(&q)).unwrap_or(false)
        });
    }
    rows.sort_by(|a, b| text(a, "name").cmp(&text(b, "name")));
    rows
}

fn create_code_name(model: &str, what: &str, input: CodeName) -> Result<Record> {
    require_admin()?;
    let data = json!({
        "code": trim_code(&input.code)?,
        "name": trim_name(&input.name)?,
        "active": true,
    });
    db::create(model, &data).map_err(|e| e.or(format!("could not create the {what} (the code may be taken)")))
}

fn set_code_name(model: &str, what: &str, input: SetCodeName) -> Result<Record> {
    require_admin()?;
    let _ = require(model, &input.id, what)?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        changes["name"] = json!(trim_name(&name)?);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    if let Some(requires_notes) = input.requires_notes {
        changes["requires_notes"] = json!(requires_notes);
    }
    if let Some(color) = input.color {
        changes["color"] = json!(color.trim());
    }
    db::update(model, &input.id, &changes)?.ok_or_else(|| Error::msg(format!("the {what} is gone")))
}

fn get_one(model: &str, what: &str, input: Id) -> Result<Record> {
    require_reader()?;
    require(model, &input.id, what)
}

fn search_model(model: &str, input: Search) -> Result<Vec<Record>> {
    require_reader()?;
    let rows = db::find::<Record>(model).limit(500).all()?;
    Ok(filter_search(rows, &input))
}

// --- source / industry / lost reason / tag ---

fn do_create_source(input: CodeName) -> Result<Record> {
    create_code_name("crm_source", "source", input)
}
fn do_set_source(input: SetCodeName) -> Result<Record> {
    set_code_name("crm_source", "source", input)
}
fn do_get_source(input: Id) -> Result<Record> {
    get_one("crm_source", "source", input)
}
fn do_search_sources(input: Search) -> Result<Vec<Record>> {
    search_model("crm_source", input)
}

fn do_create_industry(input: CodeName) -> Result<Record> {
    create_code_name("crm_industry", "industry", input)
}
fn do_set_industry(input: SetCodeName) -> Result<Record> {
    set_code_name("crm_industry", "industry", input)
}
fn do_get_industry(input: Id) -> Result<Record> {
    get_one("crm_industry", "industry", input)
}
fn do_search_industries(input: Search) -> Result<Vec<Record>> {
    search_model("crm_industry", input)
}

fn do_create_lost_reason(input: CodeName) -> Result<Record> {
    require_admin()?;
    let data = json!({
        "code": trim_code(&input.code)?,
        "name": trim_name(&input.name)?,
        "requires_notes": false,
        "active": true,
    });
    db::create("crm_lost_reason", &data).map_err(|e| e.or("could not create the lost reason (the code may be taken)"))
}
fn do_set_lost_reason(input: SetCodeName) -> Result<Record> {
    set_code_name("crm_lost_reason", "lost reason", input)
}
fn do_get_lost_reason(input: Id) -> Result<Record> {
    get_one("crm_lost_reason", "lost reason", input)
}
fn do_search_lost_reasons(input: Search) -> Result<Vec<Record>> {
    search_model("crm_lost_reason", input)
}

fn do_create_tag(input: TagInput) -> Result<Record> {
    require_admin()?;
    let mut data = json!({ "name": trim_name(&input.name)? });
    if let Some(color) = input.color {
        data["color"] = json!(color.trim());
    }
    db::create("crm_tag", &data).map_err(|e| e.or("could not create the tag (the name may be taken)"))
}
fn do_set_tag(input: SetCodeName) -> Result<Record> {
    set_code_name("crm_tag", "tag", input)
}
fn do_get_tag(input: Id) -> Result<Record> {
    get_one("crm_tag", "tag", input)
}
fn do_search_tags(input: Search) -> Result<Vec<Record>> {
    require_reader()?;
    let mut rows = db::find::<Record>("crm_tag").limit(500).all()?;
    if let Some(q) = input.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let q = q.to_lowercase();
        rows.retain(|row| text(row, "name").map(|v| v.to_lowercase().contains(&q)).unwrap_or(false));
    }
    rows.sort_by(|a, b| text(a, "name").cmp(&text(b, "name")));
    Ok(rows)
}

// --- territory ---

fn do_create_territory(input: TerritoryInput) -> Result<Record> {
    require_admin()?;
    let mut data = json!({
        "code": trim_code(&input.code)?,
        "name": trim_name(&input.name)?,
        "active": true,
    });
    if let Some(parent_id) = input.parent.filter(|p| !p.is_empty()) {
        let parent = require("crm_territory", &parent_id, "territory")?;
        data["parent"] = json!(id_of(&parent)?);
    }
    if let Some(manager) = input.manager.filter(|m| !m.is_empty()) {
        data["manager"] = json!(manager.trim());
    }
    db::create("crm_territory", &data).map_err(|e| e.or("could not create the territory (the code may be taken)"))
}

fn do_set_territory(input: SetTerritory) -> Result<Record> {
    require_admin()?;
    let _ = require("crm_territory", &input.id, "territory")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        changes["name"] = json!(trim_name(&name)?);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    if let Some(manager) = input.manager {
        changes["manager"] = if manager.is_empty() { Value::Null } else { json!(manager.trim()) };
    }
    if let Some(parent_id) = input.parent {
        if parent_id.is_empty() {
            changes["parent"] = Value::Null;
        } else {
            if parent_id == input.id {
                return Err(Error::msg("a territory cannot be its own parent"));
            }
            let parent = require("crm_territory", &parent_id, "territory")?;
            changes["parent"] = json!(id_of(&parent)?);
        }
    }
    db::update("crm_territory", &input.id, &changes)?.ok_or_else(|| Error::msg("the territory is gone"))
}

fn do_get_territory(input: Id) -> Result<Record> {
    get_one("crm_territory", "territory", input)
}
fn do_search_territories(input: Search) -> Result<Vec<Record>> {
    search_model("crm_territory", input)
}

// --- team / member ---

fn do_create_team(input: TeamInput) -> Result<Record> {
    require_admin()?;
    let company = require("company.company", &input.company, "company")?;
    let mut data = json!({
        "code": trim_code(&input.code)?,
        "name": trim_name(&input.name)?,
        "company": id_of(&company)?,
        "active": true,
    });
    if let Some(leader) = input.leader.filter(|l| !l.is_empty()) {
        data["leader"] = json!(leader.trim());
    }
    db::create("crm_team", &data).map_err(|e| e.or("could not create the team (the code may be taken)"))
}

fn do_set_team(input: SetTeam) -> Result<Record> {
    require_admin()?;
    let _ = require("crm_team", &input.id, "team")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        changes["name"] = json!(trim_name(&name)?);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    if let Some(leader) = input.leader {
        changes["leader"] = if leader.is_empty() { Value::Null } else { json!(leader.trim()) };
    }
    db::update("crm_team", &input.id, &changes)?.ok_or_else(|| Error::msg("the team is gone"))
}

fn do_get_team(input: Id) -> Result<Record> {
    get_one("crm_team", "team", input)
}
fn do_search_teams(input: Search) -> Result<Vec<Record>> {
    search_model("crm_team", input)
}

fn do_add_team_member(input: TeamMemberInput) -> Result<Record> {
    require_admin()?;
    let team = require("crm_team", &input.team, "team")?;
    let user = input.user.trim();
    if user.is_empty() {
        return Err(Error::msg("user is required"));
    }
    let data = json!({ "team": id_of(&team)?, "user": user });
    db::create("crm_team_member", &data).map_err(|e| e.or("could not add the team member (they may already be on the team)"))
}

fn do_remove_team_member(input: Id) -> Result<Record> {
    require_admin()?;
    let row = require("crm_team_member", &input.id, "team member")?;
    db::delete::<Record>("crm_team_member", &input.id)?;
    Ok(row)
}

fn do_list_team_members(input: Id) -> Result<Vec<Record>> {
    require_reader()?;
    let _ = require("crm_team", &input.id, "team")?;
    db::find::<Record>("crm_team_member").filter("team", &input.id).limit(500).all()
}

// --- pipeline / stage ---

fn do_create_pipeline(input: PipelineInput) -> Result<Record> {
    require_admin()?;
    let company = require("company.company", &input.company, "company")?;
    let data = json!({
        "code": trim_code(&input.code)?,
        "name": trim_name(&input.name)?,
        "company": id_of(&company)?,
        "active": true,
    });
    db::create("crm_pipeline", &data).map_err(|e| e.or("could not create the pipeline (the code may be taken)"))
}

fn do_set_pipeline(input: SetPipeline) -> Result<Record> {
    require_admin()?;
    let _ = require("crm_pipeline", &input.id, "pipeline")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        changes["name"] = json!(trim_name(&name)?);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("crm_pipeline", &input.id, &changes)?.ok_or_else(|| Error::msg("the pipeline is gone"))
}

fn do_get_pipeline(input: Id) -> Result<Record> {
    get_one("crm_pipeline", "pipeline", input)
}
fn do_search_pipelines(input: Search) -> Result<Vec<Record>> {
    search_model("crm_pipeline", input)
}

fn do_create_stage(input: StageInput) -> Result<Record> {
    require_admin()?;
    let pipeline = require("crm_pipeline", &input.pipeline, "pipeline")?;
    let kind = kind_or(input.kind.as_deref(), "open")?;
    let probability = input.default_probability.unwrap_or_else(|| Decimal::zero(2));
    let hundred = Decimal::whole(100, 2)?;
    if probability.is_negative() || probability > hundred {
        return Err(Error::msg("default_probability must be between 0 and 100"));
    }
    let data = json!({
        "pipeline": id_of(&pipeline)?,
        "name": trim_name(&input.name)?,
        "sequence": input.sequence.unwrap_or(10),
        "kind": kind,
        "default_probability": probability,
        "fold": input.fold.unwrap_or(false),
        "active": true,
    });
    db::create("crm_stage", &data).map_err(|e| e.or("could not create the stage"))
}

fn do_set_stage(input: SetStage) -> Result<Record> {
    require_admin()?;
    let _ = require("crm_stage", &input.id, "stage")?;
    let mut changes = json!({});
    if let Some(name) = input.name {
        changes["name"] = json!(trim_name(&name)?);
    }
    if let Some(sequence) = input.sequence {
        changes["sequence"] = json!(sequence);
    }
    if let Some(kind) = input.kind {
        changes["kind"] = json!(kind_or(Some(&kind), "open")?);
    }
    if let Some(probability) = input.default_probability {
        let hundred = Decimal::whole(100, 2)?;
        if probability.is_negative() || probability > hundred {
            return Err(Error::msg("default_probability must be between 0 and 100"));
        }
        changes["default_probability"] = json!(probability);
    }
    if let Some(fold) = input.fold {
        changes["fold"] = json!(fold);
    }
    if let Some(active) = input.active {
        changes["active"] = json!(active);
    }
    db::update("crm_stage", &input.id, &changes)?.ok_or_else(|| Error::msg("the stage is gone"))
}

fn do_get_stage(input: Id) -> Result<Record> {
    get_one("crm_stage", "stage", input)
}

fn do_list_stages(input: Id) -> Result<Vec<Record>> {
    require_reader()?;
    let _ = require("crm_pipeline", &input.id, "pipeline")?;
    let mut rows = db::find::<Record>("crm_stage").filter("pipeline", &input.id).limit(200).all()?;
    rows.retain(|row| row.get("active") != Some(&json!(false)));
    rows.sort_by(|a, b| {
        a.get("sequence")
            .and_then(Value::as_i64)
            .cmp(&b.get("sequence").and_then(Value::as_i64))
    });
    Ok(rows)
}

// --- company settings ---

fn do_set_company_settings(input: CompanySettingsInput) -> Result<Record> {
    require_admin()?;
    let company = require("company.company", &input.company, "company")?;
    let company_id = id_of(&company)?.to_string();
    let mut data = json!({
        "company": company_id,
        "use_leads": input.use_leads.unwrap_or(true),
        "require_forecast_fields": input.require_forecast_fields.unwrap_or(false),
    });
    if let Some(pipeline_id) = input.default_pipeline.filter(|p| !p.is_empty()) {
        let pipeline = require("crm_pipeline", &pipeline_id, "pipeline")?;
        data["default_pipeline"] = json!(id_of(&pipeline)?);
    }
    if let Some(existing) = db::find::<Record>("crm_company_settings").filter("company", &company_id).first()? {
        let id = id_of(&existing)?.to_string();
        return db::update("crm_company_settings", &id, &data)?
            .ok_or_else(|| Error::msg("the company settings are gone"));
    }
    db::create("crm_company_settings", &data).map_err(|e| e.or("could not save company settings"))
}

fn do_get_company_settings(input: Id) -> Result<Record> {
    require_reader()?;
    let company = require("company.company", &input.id, "company")?;
    let company_id = id_of(&company)?;
    db::find::<Record>("crm_company_settings")
        .filter("company", company_id)
        .first()?
        .ok_or_else(|| Error::msg("there are no CRM settings for this company yet"))
}

#[derive(Deserialize)]
struct PipelineEventInput {
    subject_kind: String,
    subject_id: String,
    #[serde(default)]
    from_stage: Option<String>,
    #[serde(default)]
    to_stage: Option<String>,
    kind: String,
}

fn do_record_pipeline_event(input: PipelineEventInput) -> Result<Record> {
    // Callable by CRM sales plugins (via:*) and admins.
    let kind = input.kind.as_str();
    if !["enter", "win", "lose", "reopen", "convert"].contains(&kind) {
        return Err(Error::msg("kind is enter, win, lose, reopen or convert"));
    }
    let subject_kind = input.subject_kind.as_str();
    if subject_kind != "lead" && subject_kind != "deal" {
        return Err(Error::msg("subject_kind is lead or deal"));
    }
    if input.subject_id.trim().is_empty() {
        return Err(Error::msg("subject_id is required"));
    }
    let seq = db::find::<Record>("pipeline_event")
        .filter("subject_kind", subject_kind)
        .filter("subject_id", input.subject_id.trim())
        .limit(500)
        .all()?
        .len() as i64
        + 1;
    let ctx = context::current()?;
    let mut data = json!({
        "subject_kind": subject_kind,
        "subject_id": input.subject_id.trim(),
        "kind": kind,
        "at": ctx.now,
        "by": ctx.actor.id,
        "seq": seq,
    });
    if let Some(from) = input.from_stage.filter(|s| !s.is_empty()) {
        let stage = require("crm_stage", &from, "stage")?;
        data["from_stage"] = json!(id_of(&stage)?);
    }
    if let Some(to) = input.to_stage.filter(|s| !s.is_empty()) {
        let stage = require("crm_stage", &to, "stage")?;
        data["to_stage"] = json!(id_of(&stage)?);
    }
    db::create("pipeline_event", &data).map_err(|e| e.or("could not record the pipeline event"))
}

#[derive(Deserialize)]
struct SubjectRef {
    subject_kind: String,
    subject_id: String,
}

fn do_stage_history(input: SubjectRef) -> Result<Vec<Record>> {
    require_reader()?;
    let mut rows = db::find::<Record>("pipeline_event")
        .filter("subject_kind", input.subject_kind.trim())
        .filter("subject_id", input.subject_id.trim())
        .limit(500)
        .all()?;
    rows.sort_by(|a, b| a.get("seq").and_then(Value::as_i64).cmp(&b.get("seq").and_then(Value::as_i64)));
    Ok(rows)
}

handler! {
    fn create_source(input: CodeName) -> Record { do_create_source(input) }
    fn set_source(input: SetCodeName) -> Record { do_set_source(input) }
    fn get_source(input: Id) -> Record { do_get_source(input) }
    fn search_sources(input: Search) -> Vec<Record> { do_search_sources(input) }

    fn create_industry(input: CodeName) -> Record { do_create_industry(input) }
    fn set_industry(input: SetCodeName) -> Record { do_set_industry(input) }
    fn get_industry(input: Id) -> Record { do_get_industry(input) }
    fn search_industries(input: Search) -> Vec<Record> { do_search_industries(input) }

    fn create_lost_reason(input: CodeName) -> Record { do_create_lost_reason(input) }
    fn set_lost_reason(input: SetCodeName) -> Record { do_set_lost_reason(input) }
    fn get_lost_reason(input: Id) -> Record { do_get_lost_reason(input) }
    fn search_lost_reasons(input: Search) -> Vec<Record> { do_search_lost_reasons(input) }

    fn create_tag(input: TagInput) -> Record { do_create_tag(input) }
    fn set_tag(input: SetCodeName) -> Record { do_set_tag(input) }
    fn get_tag(input: Id) -> Record { do_get_tag(input) }
    fn search_tags(input: Search) -> Vec<Record> { do_search_tags(input) }

    fn create_territory(input: TerritoryInput) -> Record { do_create_territory(input) }
    fn set_territory(input: SetTerritory) -> Record { do_set_territory(input) }
    fn get_territory(input: Id) -> Record { do_get_territory(input) }
    fn search_territories(input: Search) -> Vec<Record> { do_search_territories(input) }

    fn create_team(input: TeamInput) -> Record { do_create_team(input) }
    fn set_team(input: SetTeam) -> Record { do_set_team(input) }
    fn get_team(input: Id) -> Record { do_get_team(input) }
    fn search_teams(input: Search) -> Vec<Record> { do_search_teams(input) }
    fn add_team_member(input: TeamMemberInput) -> Record { do_add_team_member(input) }
    fn remove_team_member(input: Id) -> Record { do_remove_team_member(input) }
    fn list_team_members(input: Id) -> Vec<Record> { do_list_team_members(input) }

    fn create_pipeline(input: PipelineInput) -> Record { do_create_pipeline(input) }
    fn set_pipeline(input: SetPipeline) -> Record { do_set_pipeline(input) }
    fn get_pipeline(input: Id) -> Record { do_get_pipeline(input) }
    fn search_pipelines(input: Search) -> Vec<Record> { do_search_pipelines(input) }

    fn create_stage(input: StageInput) -> Record { do_create_stage(input) }
    fn set_stage(input: SetStage) -> Record { do_set_stage(input) }
    fn get_stage(input: Id) -> Record { do_get_stage(input) }
    fn list_stages(input: Id) -> Vec<Record> { do_list_stages(input) }

    fn set_company_settings(input: CompanySettingsInput) -> Record { do_set_company_settings(input) }
    fn get_company_settings(input: Id) -> Record { do_get_company_settings(input) }

    fn record_pipeline_event(input: PipelineEventInput) -> Record { do_record_pipeline_event(input) }
    fn stage_history(input: SubjectRef) -> Vec<Record> { do_stage_history(input) }
}

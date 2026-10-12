# CRM setup (`crm`)

## Purpose

Company-scoped CRM masters: sources, industries, lost reasons, tags, territories, teams, pipelines, stages, and `crm_company_settings`. Owns the append-only `pipeline_event` table used by leads and deals.

## Dependencies

`party`, `company`, `currency`, `number_sequence`

## Models

| Model | Role |
|---|---|
| `crm_source` | Lead/deal acquisition source (`code`, `name`) |
| `crm_industry` | Industry taxonomy |
| `crm_lost_reason` | Closed-lost reasons; optional `requires_notes` |
| `crm_tag` | Lightweight labels (`name`, `color`) |
| `crm_territory` | Hierarchy via optional `parent`; `manager` |
| `crm_team` / `crm_team_member` | Sales team + user membership |
| `crm_pipeline` | Named pipeline per company |
| `crm_stage` | Ordered stage on a pipeline; `kind` = `open` \| `hold` \| `won` \| `lost` \| `converted` |
| `crm_company_settings` | Per-company defaults (`use_leads`, `default_pipeline`, `require_forecast_fields`) |
| `pipeline_event` | Append-only stage history (`subject_kind`, `subject_id`, `from_stage`, `to_stage`, `kind`, `at`, `by`, `seq`) |

## Design

- Identity lives in `party`; this plugin only defines CRM vocabulary and stage machinery.
- Stages carry semantic `kind`. Closing paths (`won` / `lost` / `converted`) are enforced by lead/deal plugins, not by free stage edits.
- `record_pipeline_event` assigns a monotonic `seq` per subject and never updates prior rows. `stage_history` returns events sorted by `seq`.
- Default pipeline for a company comes from `crm_company_settings.default_pipeline` when present.

## Functions, commands, events

**Handlers:** CRUD/search for source, industry, lost_reason, tag, territory, team; `add_team_member` / `remove_team_member` / `list_team_members`; pipeline/stage CRUD; `list_stages`; `set_company_settings` / `get_company_settings`; `record_pipeline_event`; `stage_history`.

**Events:** none emitted. Downstream plugins call `record_pipeline_event`.

## Roles

| Role | Use |
|---|---|
| `crm_admin` | Create/change masters and company settings |
| `crm_reader` | Read masters, pipelines, stages, history |

## Boundaries

- No leads, deals, customers, or quotations.
- Does not invent contact/org records; those stay in `party`.
- Win/lose/convert transitions are owned by `crm_deal` / `crm_lead`.

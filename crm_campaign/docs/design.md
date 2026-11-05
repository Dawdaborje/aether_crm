# Campaigns (`crm_campaign`)

## Purpose

Named campaigns with UTM attribution and membership of leads/deals/parties.

## Dependencies

`crm_lead`, `crm_deal`, `party`

## Models

| Model | Role |
|---|---|
| `campaign` | `code`, `name`, `channel`, date range, `utm_campaign`, `active` |
| `campaign_member` | Link: `campaign`, `subject_kind` + `subject_id`, `joined_at` |

## Design

- Campaigns are attribution buckets; lead/deal rows also carry UTM fields copied at create/convert.
- `add_member` validates `subject_kind` ∈ `party` \| `lead` \| `deal` and records join time; duplicate membership is avoided by lookup before create.
- No send/orchestration of emails — membership and metadata only.
- Does not alter pipeline stages.

## Functions, commands, events

**Handlers:** `create_campaign`, `set_campaign`, `add_member`, `get_campaign`, `search_campaigns`, `list_members`.

**Events:** none declared in `plugin.toml`.

## Roles

| Role | Use |
|---|---|
| `crm_user` | Add campaign members |
| `crm_admin` | Define campaigns |

## Boundaries

- Inbound form → lead creation is `crm_intake`.
- Reporting funnels are `crm_report`.

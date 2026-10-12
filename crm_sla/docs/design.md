# CRM SLA (`crm_sla`)

## Purpose

First-response policies and clocks on leads and deals, with optional business-calendar deadlines.

## Dependencies

`crm`, `crm_lead`, `crm_deal`, `calendar`

## Models

| Model | Role |
|---|---|
| `sla_policy` | `code`, `name`; `apply_on` (lead/deal); `first_response_hours`; optional `calendar`, `source`, `team`, `priority`; `active` |
| `sla_clock` | Open/responded/breached/cancelled clock: subject, policy, `opened_at`, `response_by`, `first_responded_at` |

## Design

- Policy matching prefers explicit policy id, else filters by `apply_on` plus optional source/team/priority.
- Deadlines use wall-clock hours, or business minutes via `calendar` (`response_deadline` snaps into working windows).
- On `crm_lead.lead_created`, auto-starts a lead clock when a policy matches (silent skip if none).
- On `crm_lead.lead_converted`, cancels open lead clocks and may open a deal clock.
- `mark_responded` closes the open clock with `first_responded_at`. `sla_tick` (every 15 minutes) marks open clocks past `response_by` as `breached` and emits `sla_breached`.
- Does not own party identity or pipeline stages.

## Functions, commands, events

**Handlers:** `create_policy`, `set_policy`, `get_policy`, `search_policies`, `start_clock`, `mark_responded`, `cancel_clock`, `list_clocks`, `sla_tick`, `on_lead_created`, `on_lead_converted`.

**Emit:** `sla_breached`

**Listen:** `crm_lead.lead_created`, `crm_lead.lead_converted`

**Schedule:** `sla-tick` → `sla_tick` (`*/15 * * * *`)

## Roles

| Role | Use |
|---|---|
| `crm_user` | Start clocks / mark first response |
| `crm_admin` | Define and apply policies |

## Boundaries

- Helpdesk / ticket SLA is a separate suite.
- Comms may call `mark_responded` when a message counts as response; this plugin does not store message bodies.

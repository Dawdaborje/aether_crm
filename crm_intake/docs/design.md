# Lead intake (`crm_intake`)

## Purpose

Inbound form definitions and website-to-lead (or deal) creation via field maps.

## Dependencies

`crm`, `crm_lead`, `crm_deal`, `company`

## Models

| Model | Role |
|---|---|
| `intake_form` | `key`, `name`; `create_mode` (`lead` \| `deal`); `company`; optional defaults (`default_pipeline`, `default_stage`, `default_source`); `field_map` (JSON text); `active` |

## Design

- Admins define forms; submitters (service/bridge users) call `submit` with a form key/id and inbound payload.
- `field_map` remaps inbound keys into lead/deal create payloads; company and defaults are injected from the form.
- Mode `lead` → `crm_lead.create_lead`; mode `deal` → `crm_deal.create_deal`.
- Emits `intake_submitted` with form id and created subject. Party resolution still follows lead/deal create rules (not duplicated here).
- No pipeline_event ownership; stage history starts when the created lead/deal records it.

## Functions, commands, events

**Handlers:** `create_form`, `set_form`, `submit`, `get_form`, `list_forms`.

**Emit:** `intake_submitted`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Submit intake forms (service / bridge) |
| `crm_admin` | Define intake forms |

## Boundaries

- Not a public HTTP server — a bridge/API calls `submit`.
- Campaign attribution is separate (`crm_campaign` / UTM fields on the created row).

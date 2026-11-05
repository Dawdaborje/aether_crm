# Leads (`crm_lead`)

## Purpose

Unqualified prospects and idempotent convert → deal. Leads hold optional party links plus denormalized intake fields until convert resolves identity.

## Dependencies

`crm`, `crm_customer`, `crm_deal`, `party`

## Models

| Model | Role |
|---|---|
| `lead` | `number`, `company`, `title`, optional `contact_party` / `org_party`, intake fields (`email`, `phone`, `person_name`, `org_name`), masters (`source`, `industry`, `territory`, `team`, `stage`), UTM, `converted_deal`, `lost_reason` / `lost_notes`, `score`, `active` |

## Design

- **Party-owned identity:** convert resolves contact/org via existing links, party search by email/org name, or `party.create_person` / `create_organization`. Contact is parented under org when both exist. Customer is ensured on org (else contact).
- **Pipeline events:** `set_stage` and convert/lose call `crm.record_pipeline_event` (`enter` / `convert` / `lose`). Stage kind `lost` cannot be set via `set_stage` — use `lose_lead`.
- **Convert idempotency** (`if_converted` / `create_another`):
  - default / `return_existing` → return existing deal
  - `fail` → error if already converted
  - `create` or `create_another` → create another deal
  - Lost leads cannot convert.
- After convert: set `converted_deal`, move to a `converted` stage when the pipeline has one, emit `lead_converted`.
- `party.merged` repoints `contact_party` and `org_party`.

## Functions, commands, events

**Handlers:** `create_lead`, `set_lead`, `set_stage`, `lose_lead`, `convert`, `get_lead`, `search_leads`, `on_party_merged`.

**Emit:** `lead_created`, `lead_qualified` (stage kind `converted` or name “qualified”), `lead_converted`, `lead_lost`

**Listen:** `party.merged` → `on_party_merged`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Create and convert leads |
| `crm_admin` | Full lead administration |

## Boundaries

- Does not own `pipeline_event` storage (calls `crm`).
- Does not create quotations; won deals hand off later via `crm_deal` events.
- Depends on `crm_deal` for `create_deal` during convert (load after deal plugin or satisfy cycle via runtime).

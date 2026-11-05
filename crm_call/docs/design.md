# Call log (`crm_call`)

## Purpose

Phone call records linked to optional `party` and optional lead/deal subject.

## Dependencies

`crm_lead`, `crm_deal`, `party`

## Models

| Model | Role |
|---|---|
| `call_log` | `direction`, `from_number`, `to_number`, `status`, optional `party`, `subject_kind` / `subject_id`, `started_at`, `duration_sec`, `external_id`, `notes` |

## Design

- Logging is append/update of factual call rows — not pipeline stage changes.
- Subject (when set) must be a valid lead or deal; party is a soft link for 360 views.
- `external_id` supports telephony bridge idempotency when listing/filtering.
- Emits `call_logged` on create. No SLA auto-start (use activities/comms/`mark_responded` for response clocks).

## Functions, commands, events

**Handlers:** `log_call`, `set_call`, `get_call`, `list_calls` (filter by party / subject / external_id).

**Emit:** `call_logged`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Log and update calls |
| `crm_admin` | Full call-log administration |

## Boundaries

- Not a PBX; no dialing or recording storage.
- Distinct from `crm_activity` `kind=call` to-dos.
- Does not win/lose deals or convert leads.

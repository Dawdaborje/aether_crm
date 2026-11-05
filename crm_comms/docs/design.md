# CRM communications (`crm_comms`)

## Purpose

Email/messaging threads on leads and deals. Stores thread metadata and kernel message ids — not message bodies.

## Dependencies

`crm_lead`, `crm_deal`, `crm_sla`

## Models

| Model | Role |
|---|---|
| `comms_thread` | `subject_kind` + `subject_id` (lead/deal); `channel`; optional `external_thread`; `last_message_at`; `message_ids` (JSON array text); `active` |

## Design

- `open_thread` is idempotent: same `external_thread`, or same subject+channel when active.
- `append_message` dedupes message ids, updates `last_message_at`. If `counts_as_response`, calls `crm_sla.mark_responded` for the subject.
- Actual transport/content lives in kernel messaging; this plugin only keeps CRM-facing refs.
- Party identity stays on the lead/deal subject.

## Functions, commands, events

**Handlers:** `open_thread`, `append_message`, `close_thread`, `get_thread`, `list_threads`.

**Emit:** `thread_opened`, `message_appended`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Open threads and append message refs |
| `crm_admin` | Full communications administration |

## Boundaries

- No marketing blast engine (see `crm_campaign` for attribution, not send).
- Does not own SLA policies; only triggers first-response when asked.

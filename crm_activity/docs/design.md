# Activities (`crm_activity`)

## Purpose

Tasks, meetings, and call-activities attached to a lead or deal subject.

## Dependencies

`crm_lead`, `crm_deal`

## Models

| Model | Role |
|---|---|
| `activity` | `title`; `kind` = `task` \| `call` \| `meeting`; `subject_kind` + `subject_id` (lead/deal); `assignee`, `due_at`; `state` = `backlog` \| `todo` \| `in_progress` \| `done` \| `cancelled`; `priority`; `description` |

## Design

- Subject must resolve to an existing lead or deal (`assert_subject`).
- Completing/cancelling are explicit state transitions (`complete`, `cancel`), not free-form deletes.
- Nightly schedule `activity-nightly` (`15 1 * * *`) scans open activities with `due_at` in the past and emits `activity_overdue` (does not auto-change state).
- No party identity of its own; ownership stays on the parent lead/deal.

## Functions, commands, events

**Handlers:** `create_activity`, `set_activity`, `complete`, `cancel`, `list_activities`, `activity_nightly`.

**Emit:** `activity_overdue`

**Schedule:** `activity-nightly` → `activity_nightly`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Create and complete activities |
| `crm_admin` | Full activity administration |

## Boundaries

- Phone CDR / telephony detail belongs in `crm_call`; this `kind=call` is a CRM to-do, not a call log.
- Does not move pipeline stages or emit win/lose.

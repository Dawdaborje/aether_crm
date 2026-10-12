# Sales targets (`crm_target`)

## Purpose

Team and rep quotas versus won revenue for a period.

## Dependencies

`crm`, `crm_deal`, `company`, `currency`

## Models

| Model | Role |
|---|---|
| `sales_target` | `name`, `company`, optional `team` / `owner`, `period_start` / `period_end`, `amount`, `currency`, `active` |

## Design

- Targets are planning rows; attainment is computed, not stored.
- `attainment` sums `deal.expected_value` for deals in the company (and team/owner when set) whose close window falls in the period, excluding lost deals (`lost_reason` present). Prefer deals that look won (probability 100 or closed without lost reason).
- Returns `{ goal, won, deals, percent }`.
- Does not invent party identity or mutate pipeline stages.

## Functions, commands, events

**Handlers:** `create_target`, `set_target`, `attainment`, `get_target`, `list_targets`.

**Events:** none.

## Roles

| Role | Use |
|---|---|
| `crm_user` | Read targets and attainment |
| `crm_admin` | Define sales targets |

## Boundaries

- Not commission or payroll.
- Funnel/conversion analytics live in `crm_report`.
- Booked revenue from invoices is outside CRM (accounting / selling).

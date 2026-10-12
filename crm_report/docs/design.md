# CRM reports (`crm_report`)

## Purpose

Pipeline funnels, conversion rates, days-to-convert, and deadline-slip metrics; optional saved funnel presets.

## Dependencies

`crm`, `crm_lead`, `crm_deal`

## Models

| Model | Role |
|---|---|
| `saved_funnel` | Named funnel preset: `name`, optional `pipeline`, `filters` (JSON text), `owner` |

## Design

- Reports are read-side aggregations over `lead`, `deal`, and `crm.pipeline_event` — no second fact store.
- `funnel`: counts deals (optionally filtered by pipeline) by current stage.
- `conversion_rates`: lead→deal and deal win/loss style ratios for a company.
- `days_to_convert`: average duration from lead create to convert (via timestamps / events).
- `deadline_slip`: deals past `expected_close` still open.
- `save_funnel` / `list_funnels` / `get_funnel` persist UI presets only.
- Party identity and stage mutations stay in upstream plugins.

## Functions, commands, events

**Handlers:** `funnel`, `conversion_rates`, `days_to_convert`, `deadline_slip`, `save_funnel`, `list_funnels`, `get_funnel`.

**Events:** none.

## Roles

| Role | Use |
|---|---|
| `crm_user` | Run CRM reports |
| `crm_admin` | Full CRM reporting |

## Boundaries

- Not a general BI warehouse.
- Quotas/attainment are `crm_target`.
- Post-win revenue reporting belongs with selling/accounting once `erp_selling` lands.

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

- Reports are `report::query` calls over `lead` and `deal` — no second fact store and no row scan.
- `funnel`: counts deals in a pipeline by stage, and fills stages that have no deals from `crm.crm_stage`.
- `conversion_rates`: filtered counts (leads vs converted, won vs lost) for a company.
- `days_to_convert`: not computed. A lead has `converted_deal` and no convert timestamp, and the deal's create time is a different record, so `avg_days` cannot see both. This stays a follow-up until the lead stores that timestamp (or a pipeline event does). It does not scan rows.
- `deadline_slip`: open deals whose `expected_close` is before now, with `avg_days` from that date to now.
- The reports page is a dashboard preview. Group rows are capped by `ui.report.preview_rows`. The headline stats use the ungrouped total. Download Excel and Download PDF send the full funnel (still at most 1000 groups) through `report::export`.
- `save_funnel` / `list_funnels` / `get_funnel` persist UI presets only.
- Party identity and stage mutations stay in upstream plugins.

## Functions, commands, events

**Handlers:** `funnel`, `conversion_rates`, `days_to_convert`, `deadline_slip`, `reports_dashboard`, `export_report`, `save_funnel`, `list_funnels`, `get_funnel`.

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

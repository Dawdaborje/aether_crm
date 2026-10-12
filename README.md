# Aether CRM

Customer Relationship Management for Aether — twelve **Rust → WASM** plugins. Identity is **party-owned**; pipeline history is **append-only** (`pipeline_event`); a **won deal** emits `deal_won` for a later hand-off to `erp_selling`.

Per-plugin design: each `./<name>/docs/design.md`.

## Spine

1. **Party owns identity** — leads/deals/customers link to `party`; CRM does not invent a second contact/org store.
2. **Append-only stage history** — `crm.pipeline_event` records enter/convert/win/lose/reopen; current stage is a pointer.
3. **Explicit close paths** — `win_deal` / `lose_deal` / `lose_lead` / `convert` (not free stage edits into terminal kinds).
4. **Idempotent convert** — default returns the existing deal; `if_converted` / `create_another` control retries.
5. **Selling hand-off** — `deal_won` is the event boundary; quotations/orders are not in this workspace.

## Plugins

### Phase A — identity & pipeline

| Plugin | Role | Design |
|---|---|---|
| `crm` | Masters: source, industry, lost reason, tag, territory, team, pipeline, stage, company settings, `pipeline_event` | [docs](./crm/docs/design.md) |
| `crm_customer` | Customer 360 overlay on `party` | [docs](./crm_customer/docs/design.md) |
| `crm_lead` | Leads + idempotent convert → deal | [docs](./crm_lead/docs/design.md) |
| `crm_deal` | Deals, lines, win/lose/reopen | [docs](./crm_deal/docs/design.md) |
| `crm_activity` | Tasks / meetings / call-activities | [docs](./crm_activity/docs/design.md) |

### Phase B — response & channels

| Plugin | Role | Design |
|---|---|---|
| `crm_sla` | SLA policies and first-response clocks | [docs](./crm_sla/docs/design.md) |
| `crm_call` | Call log | [docs](./crm_call/docs/design.md) |
| `crm_comms` | Email/messaging threads on lead/deal | [docs](./crm_comms/docs/design.md) |

### Phase C — growth & targets

| Plugin | Role | Design |
|---|---|---|
| `crm_campaign` | Campaigns + UTM membership | [docs](./crm_campaign/docs/design.md) |
| `crm_intake` | Inbound forms → lead/deal | [docs](./crm_intake/docs/design.md) |
| `crm_target` | Quotas / attainment | [docs](./crm_target/docs/design.md) |
| `crm_report` | Funnels and conversion metrics | [docs](./crm_report/docs/design.md) |

## Depends on base

Load from the **base** workspace first:

`currency` → `country` → `party` → `company` → `number_sequence`

Also used where declared: `calendar` (`crm_sla`).

## Outside this workspace

| Concern | Where |
|---|---|
| Quotations / sales orders | Later `erp_selling` (listen for `crm_deal.deal_won`) |
| Invoicing / GL | Accounting |
| Helpdesk / appointments | Separate suites |

## Load order

```text
base: currency → country → party → company → number_sequence
(+ calendar before crm_sla)

Phase A:
  crm → crm_customer → crm_deal → crm_lead → crm_activity

Phase B:
  crm_sla → crm_call → crm_comms

Phase C:
  crm_campaign → crm_intake → crm_target → crm_report
```

`crm_lead` depends on `crm_deal` for convert (`create_deal`). Prefer loading `crm_deal` before `crm_lead`.

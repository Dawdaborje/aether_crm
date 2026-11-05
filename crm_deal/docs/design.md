# Deals (`crm_deal`)

## Purpose

Opportunities with lines, append-only stage history (via `crm.pipeline_event`), and explicit win / lose / reopen.

## Dependencies

`crm`, `crm_customer`, `party`

## Models

| Model | Role |
|---|---|
| `deal` | `number`, `company`, `title`; optional `lead`, `contact_party`, `org_party`, `customer`; `pipeline` / `stage`; ownership (`owner`, `team`, `territory`); value (`currency`, `expected_value`, `probability`, `probability_manual`); close fields; UTM; `active` |
| `deal_line` | Lines: `deal`, `line_no`, `description`, `qty`, `unit_price`, `amount` |

## Design

- **Party-owned identity:** contact/org/customer are links; no second identity store.
- **Append-only pipeline:** every stage change records a `pipeline_event` through `crm.record_pipeline_event` with kinds `enter`, `win`, `lose`, `reopen`.
- **Create:** defaults pipeline/stage from company settings or first company pipeline + first open stage; numbers via `number_sequence`.
- **`set_stage`:** refuses `won` / `lost` kinds — must use `win_deal` / `lose_deal`. Applies stage `default_probability` unless `probability_manual`.
- **`win_deal`:** move to pipeline `won` stage, probability 100, set `closed_at`, emit `deal_won` (hand-off signal for later `erp_selling`).
- **`lose_deal`:** require lost reason (+ notes when `requires_notes`), move to `lost` stage, probability 0, `active=false`, emit `deal_lost`.
- **`reopen_deal`:** first open stage, clear close fields, event kind `reopen`.
- Closed deals refuse edits until reopen. `set_lines` replaces all lines and rolls `expected_value`.
- `party.merged` repoints `contact_party` / `org_party`.

## Functions, commands, events

**Handlers:** `create_deal`, `set_deal`, `set_stage`, `set_lines`, `win_deal`, `lose_deal`, `reopen_deal`, `get_deal`, `search_deals`, `list_lines`, `stage_history`, `on_party_merged`.

**Emit:** `deal_stage_changed`, `deal_won`, `deal_lost`

**Listen:** `party.merged` → `on_party_merged`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Work deals through the pipeline |
| `crm_admin` | Full deal administration |

## Boundaries

- No quotations or sales orders (later `erp_selling` consumes `deal_won`).
- Does not own masters; stages/pipelines live in `crm`.
- Activity, SLA, call, and comms attach by subject id — not embedded here.

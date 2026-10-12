# Customers (`crm_customer`)

## Purpose

Customer 360 overlay on `party`: one `customer` row per `(party, company)` with owner, territory, team, segment, and lifecycle.

## Dependencies

`crm`, `party`

## Models

| Model | Role |
|---|---|
| `customer` | Overlay: `party`, `company` (required); `owner`, `territory`, `team`, `segment`; `lifecycle` = `prospect` \| `active` \| `churned`; `active` |

## Design

- Party owns legal/contact identity. CRM never duplicates person/org fields onto `customer`.
- `ensure_customer` is idempotent: returns the existing row for `(party, company)`, else creates one, sets `party.is_customer`, emits `customer_created`.
- Listens to `party.merged`: repoints `customer.party` from → into; if both parties already have a customer for the same company, deactivates the loser overlay.

## Functions, commands, events

**Handlers:** `ensure_customer`, `set_customer`, `get_customer`, `search_customers`, `on_party_merged`.

**Emit:** `customer_created`

**Listen:** `party.merged` → `on_party_merged`

## Roles

| Role | Use |
|---|---|
| `crm_user` | Read/update customers |
| `crm_admin` | Full customer administration |

## Boundaries

- No pipeline stages, deals, or invoices.
- Selling documents (quotations / sales orders) belong in later `erp_selling`.
- Merge semantics only touch this overlay; lead/deal plugins repoint their own party links.

# Quant OS Frontend — Implementation Note (F1)

## Audit summary

Existing Rust workspace (`crates/*`, `strategies/python`, `intent-bridge`) already defines:

- Account ≠ Venue
- TradeIntent → RiskDecision → Order → Fill → Position
- Money as Decimal / NUMERIC (frontend: `Money` string)
- Reconciliation alert-only
- Paper PropVenue / SimulatedVenue

## Frontend placement

`frontend/` — isolated Next.js App Router app. No Cargo workspace changes.

## Domain alignment (vs draft UI copy)

| UI must use | Not |
|-------------|-----|
| Buy / Sell | BUY / SELL |
| Created, PendingRisk, Approved, Submitted, Accepted, PartiallyFilled, Filled, … | NEW / ACKNOWLEDGED inventing |
| TradeIntentCreated | TradeIntentGenerated as primary name |
| AccountType: Simulated, Prop, Personal, Cex, Vault | Broker |

## Data mode

F1–F6: `dataMode: mock`, `eventMode: mock`, `environment: paper`.
API repositories stubbed for later Rust REST/SSE.

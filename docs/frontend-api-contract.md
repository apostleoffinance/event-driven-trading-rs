# Quant OS — Frontend API Contract (proposed)

These endpoints are **integration contracts** for a future Rust HTTP/SSE layer.
They are **not** implemented in the Rust workspace yet. The Next.js app uses mock repositories today.

Base URL: `NEXT_PUBLIC_API_URL`

## REST

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/accounts` | List accounts |
| GET | `/api/accounts/:id` | Account detail + state |
| GET | `/api/portfolio` | Portfolio snapshot |
| GET | `/api/portfolio/equity` | Equity time series |
| GET | `/api/strategies` | Strategy deployments |
| GET | `/api/strategies/:id` | Strategy detail |
| GET | `/api/positions` | Open positions |
| GET | `/api/orders` | Orders (OMS) |
| GET | `/api/orders/:id` | Order + fills + audit |
| GET | `/api/risk` | Risk overview meters |
| GET | `/api/risk/decisions` | Recent RiskDecision records |
| GET | `/api/events` | Recent TradingEvent envelopes |
| GET | `/api/reconciliation` | Latest recon report |
| GET | `/api/system/health` | Component health |

## Events

`NEXT_PUBLIC_EVENT_STREAM_URL` — WebSocket or SSE stream of `TradingEvent` JSON matching `crates/events`.

## Money

All monetary fields are **decimal strings** (e.g. `"100482.31"`), matching Rust `Decimal` / Postgres `NUMERIC`.

## Domain notes

- Side: `Buy` \| `Sell`
- Order status: domain state machine (`Created` … `Filled`)
- Intent events: prefer `TradeIntentCreated`
- Never expose credentials or signing keys to the client

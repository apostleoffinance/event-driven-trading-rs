# EdgeNest — Shared Platform API Contract (proposed)

These endpoints are **proposed contracts**, not proof of an implemented Rust HTTP service. The Next.js app uses mock repositories today. The same API is intended to serve both the EdgeNest Trader Dashboard and external developer applications; neither client may bypass Rust risk authorization, OMS, adapter capability checks or reconciliation.

Base URL: `NEXT_PUBLIC_API_URL`

## Product boundary
- First-party dashboard and third-party developers use the same versioned service contracts.
- Tenant identity comes from authenticated server-side claims, never from a trusted client-supplied tenant ID.
- Public account identifiers are opaque and tenant-scoped.
- Demo, paper and live environments are explicit and cannot be confused.
- Account provisioning/connectivity is a separate lifecycle from execution eligibility.
- Do not expose broker passwords, terminal credentials, private keys, certificates or internal host details.
- Live write endpoints remain unavailable until the corresponding venue and platform release gates are approved.

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

## Proposed developer API additions

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/v1/accounts/connections` | Start a tenant-scoped account connection/provisioning request |
| GET | `/api/v1/accounts/connections/:id` | Read provisioning and connectivity state |
| POST | `/api/v1/accounts/connections/:id/disconnect` | Request account disconnection |
| GET | `/api/v1/developer/usage` | Read current usage/quota summary |
| GET | `/api/v1/developer/api-keys` | List API-key metadata (never key secrets) |
| POST | `/api/v1/developer/api-keys` | Create a scoped key and return its secret once |
| POST | `/api/v1/developer/api-keys/:id/revoke` | Revoke a key |

These are proposals only. They must not be described as implemented endpoints until the HTTP service, authorization and tests exist. Account connection must use a secure provider-specific credential flow; never place credentials in URLs, logs or frontend telemetry.

## Events

`NEXT_PUBLIC_EVENT_STREAM_URL` — WebSocket or SSE stream of `TradingEvent` JSON matching `crates/events`. The production contract must define tenant-scoped subscriptions, sequence/cursor behavior, reconnect recovery, backpressure and authorization renewal.

## Money

All monetary fields are **decimal strings** (e.g. `"100482.31"`), matching Rust `Decimal` / Postgres `NUMERIC`.

## Domain notes

- Side: `Buy` \| `Sell`
- Order status: domain state machine (`Created` … `Filled`)
- Intent events: prefer `TradeIntentCreated`
- Never expose credentials or signing keys to the client

## Shared contract rule across EdgeNest products

This contract is the common integration boundary for EdgeNest products; it is not a product-definition change. The dashboard and Developer Platform are clients of shared services, and other EdgeNest products should reuse the same account, event, risk and execution contracts where applicable rather than creating separate broker connectivity or financial-state implementations.

Read-only products may consume authorized account/trade events. Any product that can create or modify exposure must submit through the shared risk authorization and OMS path; the frontend, strategy runtime, Risk Router or Agent must not call a broker gateway directly. Account and event data must remain tenant-scoped, and all multi-account outcomes are independent and reconciled. These endpoints remain proposed until server implementation and tests exist.


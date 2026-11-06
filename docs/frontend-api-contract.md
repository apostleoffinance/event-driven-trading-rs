# EdgeNest — Shared Platform API Contract

The first-party Identity and Journal REST endpoints below are implemented in the Rust workspace. Trading platform/OMS endpoints remain proposed contracts and are not represented as live APIs by the Journal module. The same shared service contracts are intended to serve EdgeNest dashboard and external developer applications.

Base URLs:
- `NEXT_PUBLIC_IDENTITY_API_URL` — EdgeNest Identity API
- `NEXT_PUBLIC_JOURNAL_API_URL` — EdgeNest Journal API
- `NEXT_PUBLIC_API_URL` — shared platform API / compatibility fallback

## Product boundary
- First-party dashboard and third-party developers use the same versioned service contracts.
- Tenant identity comes from authenticated server-side claims, never from a trusted client-supplied tenant ID.
- Public account identifiers are opaque and tenant-scoped.
- Demo, paper and live environments are explicit and cannot be confused.
- Account provisioning/connectivity is a separate lifecycle from execution eligibility.
- Do not expose broker passwords, terminal credentials, private keys, certificates or internal host details.
- Live write endpoints remain unavailable until the corresponding venue and platform release gates are approved.

## Identity (implemented in `identity-api`)

| Method | Path | Purpose |
|---|---|---|
| POST | `/api/v1/auth/register` | Create EdgeNest user and email verification |
| POST | `/api/v1/auth/verify-email` | Verify email token |
| POST | `/api/v1/auth/login` | Password login (returns MFA challenge if enabled) |
| POST | `/api/v1/auth/mfa/verify-login` | Complete MFA login |
| POST | `/api/v1/auth/refresh` | Refresh access JWT using HttpOnly refresh cookie |
| POST | `/api/v1/auth/logout` | Revoke refresh session |
| GET | `/api/v1/auth/me` | Authenticated user |
| POST | `/api/v1/auth/password-reset/request` | Request password reset |
| POST | `/api/v1/auth/password-reset/confirm` | Complete password reset |
| POST | `/api/v1/auth/mfa/setup` | Begin authenticator enrollment |
| POST | `/api/v1/auth/mfa/confirm` | Enable TOTP and issue recovery codes |
| POST | `/api/v1/auth/mfa/disable` | Disable TOTP |
| GET | `/.well-known/jwks.json` | RSA public key set |

Refresh token is a Secure, HttpOnly, SameSite=None `__Host-edgenest_refresh` cookie. The frontend holds short-lived access JWTs in memory, never localStorage. Configure exact CORS origins and credentials. Identity requires Resend API configuration for verification/recovery mail.

## Journal (implemented in `journal-api`)

Every route requires an EdgeNest RS256 bearer access JWT; the owner is derived from its verified `sub`.

| Method | Path | Purpose |
|---|---|---|
| GET / POST | `/api/v1/journal/accounts` | List/create personal journal accounts |
| GET / PUT / PATCH / DELETE | `/api/v1/journal/accounts/:id` | Account details, replace, archive |
| GET / POST | `/api/v1/journal/setups` | List/create journal setups |
| GET / PUT / PATCH / DELETE | `/api/v1/journal/setups/:id` | Setup details, replace, archive |
| GET / POST | `/api/v1/journal/trades` | List/create journal trades |
| GET / PUT / PATCH / DELETE | `/api/v1/journal/trades/:id` | Trade detail, replace, delete |

Journal records are a distinct user journal data domain; they do not create or alter OMS orders, fills, positions, or execution state. No TraderOS auth/runtime data is migrated.

## REST (trading platform endpoints — proposed)

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


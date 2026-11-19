# MT5-MVP-01B — Tenant-scoped read API and dashboard implementation plan

**Status:** implementation plan only. Do not expose broker snapshots to customers until 01A host acceptance and tenant authorization gates are satisfied.

## 1. Preconditions

- 01A single TenTrade DEMO host acceptance: all 21 checks accepted with independent review.
- QOS-020 exact identity guard and cleanup green on the target commit.
- I1 session remediation and R2 membership/entitlement authorization tests accepted.
- Verified broker binding and evidence authority; no account-number, terminal ID, organization header or path parameter is treated as authorization.
- Slot registry admission enforcement completed before more than one terminal is used.
- Customer credential intake remains disabled until 01C is reviewed.

## 2. API contract

All routes require a valid authenticated session and resolve scope server-side:

`authenticated user → active organization membership → active account entitlement → verified broker binding → active gateway/READY terminal slot → current account snapshot`.

Proposed routes:

- `GET /api/v1/mt5/broker-catalog` — only ACTIVE, supported broker/server entries; expose connector family, environment and capabilities, not internal host details.
- `GET /api/v1/mt5/connections` — current user's authorized connections, with opaque connection ID, broker/server labels, masked account label, lifecycle/readiness and freshness.
- `GET /api/v1/mt5/connections/{connection_id}` — one authorized connection and readiness/reconnect timestamps.
- `GET /api/v1/mt5/connections/{connection_id}/account` — controlled balance/equity/margin/currency fields and captured/source timestamps.
- `GET /api/v1/mt5/connections/{connection_id}/market` — latest supported market snapshot with source timestamp and freshness.
- `GET /api/v1/mt5/connections/{connection_id}/positions` and `/orders` — current read-only state only.

Never expose raw snapshot JSON, HMAC lookup tokens, certificate fingerprints, worker IDs, host IDs, terminal profile paths, secret references, raw login/account number or database connection data. Do not accept client-supplied organization, account, broker, terminal or gateway IDs as authority. A connection ID is only a lookup key after the full authorization chain.

## 3. Response envelope and freshness

Every snapshot response includes `data`, `captured_at`, `source_at` where available, `received_at`, `freshness` (`FRESH|STALE|UNAVAILABLE|UNKNOWN`), `last_connected_at`, `last_reconciled_at` and a stable non-secret status code. Broker tick/source time determines market freshness; database write time does not.

Account/position/order snapshots must be internally consistent with the same broker binding and terminal slot. Return an explicit unavailable/partial state rather than substituting PAPER account data or presenting stale data as current.

## 4. Dashboard

A new MT5 Connections page should include:

- supported broker/server selection (DEMO clearly labeled; LIVE hidden for MVP);
- operator-assisted enrollment state and verification status;
- connection lifecycle: pending, connecting, connected, degraded, disconnected, revoked;
- account overview: balance, equity, margin/free margin, currency and source freshness;
- current positions and pending/open orders, read-only;
- last successful connection, last tick and last reconciliation;
- clear stale/unavailable/unsupported-history states.

No trade buttons, execution controls, credential form or customer-side VPS/terminal setup instructions.

## 5. Authorization and test matrix

For every endpoint, test:
- unauthenticated session;
- inactive/suspended organization membership;
- missing/revoked entitlement;
- missing/pending/revoked/suspended broker binding;
- unverified, expired, unauthenticated or conflicting evidence;
- guessed connection/account/terminal/gateway IDs;
- cross-organization ID substitution;
- slot not READY, worker mismatch, broker/server mismatch and revoked assignment;
- stale/missing market/account snapshot and database unavailable;
- raw snapshot, HMAC, certificate and secret fields never serialized.

All failures must return no data and must not disclose whether another organization owns the supplied identifier. Use guarded disposable PostgreSQL tests for DB-backed authorization.

## 6. Delivery sequence

1. Freeze versioned DTOs and API error/freshness semantics.
2. Add repository queries that start from authenticated user and active organization, not client-supplied tenant IDs.
3. Add endpoint tests for every negative authorization case before UI wiring.
4. Add frontend API client types and loading/empty/degraded/stale/revoked states.
5. Add the read-only connections dashboard.
6. Run guarded DB tests and frontend lint/typecheck/build.
7. Keep customer release blocked until 01A/01B evidence and security review are complete.

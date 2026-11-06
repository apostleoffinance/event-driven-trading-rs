# Cloudflare Frontend ↔ Rust Backend Integration Audit

Date: 2026-11-06
Repository: apostleoffinance/event-driven-trading-rs
Base: main at 97c731ca7bc09ac350544f075bdf7b171048cc33
Scope: Source audit after frontend deployment. This is not runtime, security, or end-to-end verification.

## Executive summary

The Next.js frontend is deployable on Cloudflare Workers, but it is not connected to the Rust trading backend. The repository's repository factory always returns mock repositories. Setting NEXT_PUBLIC_DATA_MODE=api only logs a warning and still returns mock data.

Existing Rust services found:
- Identity API: register/login/refresh/logout/me.
- Journal API: owner-scoped journal account/setup/trade CRUD.
- Market Data API: candles/providers/instruments/MT5 latest.
- The source audit did not find a unified API for dashboard portfolio, trading accounts, strategies, positions, OMS orders, risk, reconciliation, system telemetry, or execution.

The frontend's simulated pipeline/events are not proof of Rust backend processing, risk authorization, broker connectivity, or venue execution.

## Frontend audit

### Mock source
frontend/src/lib/repositories/index.ts returns mockRepositories unconditionally. API mode is a stub; no API repository implementation is selected. This is the highest priority gap.

### Dashboard / simulation
- frontend/src/app/page.tsx loads portfolio, positions, orders, risk, pipeline from repository abstraction.
- SimulationProvider and SimulationControls are browser simulation.
- frontend/src/hooks/useEventStream.ts generates synthetic mock ticks on an interval.
- In mock mode, connected state may represent the local mock loop rather than an actual remote stream.
- Topbar SYSTEM HEALTHY was documented as unconditional in FRONTEND_AUDIT.md.

### Journal
frontend/src/app/journal/page.tsx is a static pending-integration screen. It explicitly says no journal data is loaded and TraderOS history has not been imported.

### Calculator
frontend/src/app/calculator/page.tsx is a client-side estimate. It is not wired to instrument metadata, server-side risk, or an order ticket.

### Analytics Lab
frontend/src/app/analytics/page.tsx computes metrics from manually pasted P&L; it does not read Journal or trading account data.

### Accounts / portfolio / strategies / positions / orders / risk / reconciliation / system
These pages use the repository abstraction or mock fixtures. Until the repository factory is wired to API implementations, these remain mock-backed.

### Environment / security
frontend/src/config/frontend.ts reads NEXT_PUBLIC_API_URL and NEXT_PUBLIC_EVENT_STREAM_URL. NEXT_PUBLIC variables are browser-visible; never place JWT private keys, database URLs, broker credentials, MT5 credentials, or privileged tokens there.

## Rust services observed

### Identity API (crates/identity-api/src/main.rs)
- GET /healthz
- GET /readyz
- POST /api/v1/auth/register
- POST /api/v1/auth/login
- POST /api/v1/auth/refresh
- POST /api/v1/auth/logout
- GET /api/v1/auth/me

Requires DATABASE_URL, EDGENEST_JWT_ISSUER, EDGENEST_JWT_AUDIENCE, EDGENEST_JWT_PRIVATE_KEY_PEM, EDGENEST_JWT_PUBLIC_KEY_PEM; optional TTL and bind settings. CORS is configured from IDENTITY_CORS_ORIGINS.

### Journal API (crates/journal-api/src/main.rs, http.rs, resources.rs)
- GET /healthz, GET /readyz
- GET/POST /api/v1/journal/accounts
- GET/PUT/PATCH/DELETE /api/v1/journal/accounts/:id
- GET/POST /api/v1/journal/setups
- GET/PUT/PATCH/DELETE /api/v1/journal/setups/:id
- GET/POST /api/v1/journal/trades
- GET/PUT/PATCH/DELETE /api/v1/journal/trades/:trade_id

JWT authenticated, owner-scoped persistence. Frontend Journal route does not use it yet.

### Market Data API (crates/market-data/src/http.rs)
- GET /healthz
- GET /api/v1/market-data/candles
- GET /api/v1/market-data/providers
- GET /api/v1/market-data/instruments
- GET /api/v1/market-data/mt5/latest

Read-only. Actual provider configuration, backend runtime availability, and data freshness were not tested.

## Proposed contracts vs actual implementation

docs/frontend-api-contract.md explicitly marks portfolio/accounts/strategies/positions/orders/risk/events/reconciliation/system endpoints as proposed. Do not treat those routes as implemented until Rust handlers, auth checks, and tests exist.

## Deployment boundary

Cloudflare Workers hosts the Next.js frontend. It does not host Rust services, Postgres, MT5 terminal, broker adapters, or venue execution.

Expected architecture:
Browser → Cloudflare Worker (Next.js) → HTTPS API gateway/backend → Identity / Journal / Market Data / trading services → Postgres / Windows VPS / venue adapters.

No browser-direct broker/MT5 credentials. All exposure-changing requests must flow through backend risk authorization and OMS. Keep live execution disabled until release gates are satisfied.

## Integration sequence / acceptance criteria

### API foundation
- [ ] Decide gateway/BFF and backend origin.
- [ ] Establish HTTPS reachability and exact CORS allowlist.
- [ ] Define versioned contracts/OpenAPI for existing services.
- [ ] Establish session/auth/token-renewal model.
- [ ] Confirm health/readiness and environment separation.

### Frontend API integration
- [ ] Replace mock-only repository factory with real API repositories.
- [ ] API mode must fail visibly, not silently fall back to mocks.
- [ ] Make mock/demo status explicit and unmistakable.
- [ ] Integrate Identity API login/session/me/refresh/logout.
- [ ] Integrate Journal account/setup/trade CRUD.
- [ ] Integrate Market Data read-only endpoints.
- [ ] Add loading, empty, stale, disconnected, and error states.

### Trading read-path API
- [ ] Implement actual server handlers for account snapshots, portfolio, positions, orders, strategies, risk, reconciliation, system health, and pipeline.
- [ ] Enforce tenant/account authorization for each endpoint.
- [ ] Add authenticated event streaming with replay/cursor/reconnect semantics.
- [ ] Ensure financial values serialize safely (decimal strings).

### Safety / execution
- [ ] Frontend never calls broker or MT5 directly.
- [ ] All execution passes backend risk authorization + OMS.
- [ ] Add idempotency, audit records, and independent multi-account reconciliation.
- [ ] Keep live execution disabled pending security and venue gates.

### Runtime validation
- [ ] Frontend lint/typecheck/build/OpenNext build/Wrangler preview.
- [ ] Rust fmt/check/test/clippy and API integration tests.
- [ ] Cloudflare browser smoke tests against backend.
- [ ] Test auth, CORS preflight, 401/403, expired session, 429, 5xx, stale feed, and outage.
- [ ] Check logs for secrets/account data leakage.
- [ ] Verify paper/demo/live state cannot be confused.

Commands (not executed during this source-only audit):
```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd frontend && npm ci && npm run typecheck && npm run build && npm run cloudflare:build && npm run cloudflare:preview
```

## Audit conclusion

Cloudflare frontend: deployment/rendering confirmed by owner-provided screenshot.
Backend connectivity: not established.
The immediate blocker is the mock-only frontend data layer plus missing dashboard REST contracts. Complete API foundation before claiming end-to-end functional status. This audit did not execute tests or inspect deployed runtime/API logs.

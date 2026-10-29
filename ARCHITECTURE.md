# Quant OS — Architecture
Last updated: 2026-10-27. `RULES.md` overrides conflicts. Target design is not a claim of current implementation.

## Existing baseline (source must be re-verified before code changes)
Repository documentation describes a Rust/Tokio workspace, Python strategies emitting `TradeIntent`, `rust_decimal`, PostgreSQL/SQLx, and a Next.js/TypeScript frontend documented as mock-first. It documents `VenueAdapter`, `SimulatedVenue`, and a prop-paper path; prop live mode is documented as `NotImplemented`; vault accounts are reserved. These are baseline notes, not proof of live capability.

## EdgeNest platform boundary
EdgeNest is both a trader-facing application and a developer-facing trading infrastructure API. Both clients must call the same backend services; the frontend is not the execution engine.

Target platform layers:
- **Public API layer:** versioned REST endpoints, authenticated WebSocket/SSE events, tenant-scoped authorization, quotas/rate limits, idempotency keys, and stable error contracts.
- **Account/connectivity control plane:** account registry, platform/broker metadata, explicit account-to-terminal binding, provisioning lifecycle, compatibility/policy status, connection health, and terminal fleet scheduling.
- **Execution data plane:** per-account risk authorization, OMS, adapter dispatch, venue acknowledgements/events, durable order state and reconciliation.
- **MT5 gateway fleet:** managed Windows hosts with separately isolated terminal sessions for concurrently active accounts unless a supported broker-side API removes that requirement. A gateway worker uses a versioned authenticated protocol; host-side supervision handles process health, restart, reconnect and provisioning.
- **Developer experience:** API docs, SDKs, demo sandbox, webhooks/event streams, usage metering and future billing.
- **First-party dashboard:** a client of the same API and event contracts available to developers; it must not have privileged execution shortcuts.

The target is to build EdgeNest-owned core orchestration and gateway control while allowing replaceable platform adapters. Third-party hosted MT5 APIs may be used for comparison or temporary experiments, but must not become an unexamined permanent dependency.

## Component boundaries
- `crates/domain`: canonical types, IDs, policies, instrument specs, state machines.
- `crates/events`: immutable typed events and correlation metadata.
- `crates/strategy-runtime`, `strategies/python`: strategy lifecycle; strategies emit intents only.
- `crates/intent-bridge`: validate external intent payloads.
- `crates/account-engine`: account registry, snapshots, statuses, account controls.
- `crates/risk-engine`: deterministic authorization and risk-aware sizing.
- `crates/execution-engine`: OMS, idempotency, order state, fills.
- `crates/venue-connectors`: capability-aware venue adapters and transport.
- `crates/persistence`: durable financial/audit records.
- `crates/trading-runtime`: pipeline orchestration.
- `crates/reconciliation`: compare internal/external state; alert-only.
- `frontend`: operator control plane, never financial truth.

## Target data flow
Market data + health → strategy/bridge → `TradeIntent` → validate intent/instrument → select eligible accounts → independently evaluate risk per account → OMS/idempotent order → `VenueAdapter` → external venue → acknowledgements/fills/snapshots → persistence and audit ↔ reconciliation.

Multi-account fan-out is not atomic. Partial success is an explicit state.

## VenueAdapter responsibilities
Model capabilities explicitly: submit/cancel, stable client IDs, typed acknowledgement/errors, ambiguous outcomes, async order/fill events, account snapshots, open orders, positions, instrument specs, health/rate limits, and reconciliation. Do not assume every venue supports the same order types or controls. Vendor/network code stays out of account and risk policy crates.

## Multi-account sizing flow
Check account eligibility and fresh state; compute remaining risk budget after existing exposure; derive risk/unit from stop distance and instrument contract/tick/FX conversion rules; bound by account, strategy, portfolio, leverage/margin, concentration, and venue constraints; round down to valid volume increments; reject below-minimum or missing/stale inputs; persist policy version, inputs, decision, and reason. Use exact decimal arithmetic.

## Venue bridges
**MT5:** EdgeNest-managed connectivity is the first infrastructure milestone. Evaluate two implementation paths behind the same adapter contract: (A) official MetaTrader5 Python integration against a supervised running terminal; (B) a managed MQL5 EA bridge using an authenticated, versioned protocol. Choose after real demo-terminal conformance, failure testing, security review and operating-cost measurement. A customer must not have to attach an EA or keep a personal computer running; if the EA path is chosen, EdgeNest provisions and manages the EA inside its own terminal fleet. ZeroMQ is a candidate, not a settled decision.
**cTrader:** later adapter; isolate OAuth/token lifecycle and Protobuf/API details, with least privilege and rate-limit handling.
**Hosted gateway operations:** account identity must be bound to an immutable tenant/account/terminal/server mapping. Provisioning and reconnect must not silently switch account identity. A successful bridge write is not a venue acknowledgement or fill. After a timeout, mark the outcome unknown and reconcile before retrying.
**CEX:** integrate one provider first; handle exchange filters, precision, fees/funding, rate limits, sequence gaps, and ambiguous submits.
**On-chain later:** model signing, nonce, pending, revert, and finality; reconcile wallet/protocol state; signing keys never enter strategy/frontend.

## Security and frontend
No secrets in source, logs, events, frontend, or strategy payloads. Tenant isolation and least privilege gate SaaS. Live mode must be explicit. Frontend labels mock/paper/live and shows unknown/stale/partial states; money wire values are strings. Never show success before venue confirmation.

## Public API contract principles
- REST and streaming contracts are versioned and shared by the dashboard and third-party developers.
- Money and quantity values that require exactness use decimal strings on the wire.
- Every write request carries tenant-scoped authorization, an idempotency key, correlation ID and explicit account identity.
- WebSocket/SSE streams must expose sequence/cursor semantics, reconnect behavior and replay or snapshot recovery rules.
- External developers never supply trusted tenant IDs as authorization; tenant identity comes from authenticated claims.
- Rate limits, quotas, error codes, API version support and deprecation policy are explicit.
- Public API operations cannot bypass risk authorization, OMS lifecycle rules, adapter capability gates or reconciliation latches.

## Verification
Unit/property/invariant tests; adapter contract tests; simulator/failure injection; restart/replay/reconciliation tests; paper forward run; controlled live pilot. Green CI alone does not prove financial correctness. Platform release additionally requires tenant-isolation/security tests, API contract tests, stream recovery tests, provisioning/revocation tests and documented operational runbooks.

## Shared EdgeNest infrastructure rule

**Every EdgeNest product must reuse the same shared infrastructure; this is an architecture clarification, not a change to product definitions or scope.** Trader-facing, developer-facing, strategy, Risk Router / copier, risk co-pilot, journal and future agent experiences are clients/workflows over shared platform services wherever applicable. They must not implement separate MT5 connectivity, duplicate financial truth, or bypass shared authorization and execution controls.

The shared foundation comprises:

1. **Connectivity control plane:** tenant-scoped account registry, broker/platform/server metadata, immutable account-to-terminal identity binding, connection lifecycle, compatibility status, provisioning and health.
2. **Managed gateway fleet:** EdgeNest-operated MT5 terminal sessions and authenticated bridge/worker processes. The target customer experience does not require users to attach an EA to their charts or maintain their own terminal host. If the MQL5 bridge path is selected, EdgeNest manages that EA inside its own terminal fleet.
3. **Shared execution data plane:** one request boundary for account eligibility, fresh-data validation, independent per-account risk authorization, OMS/idempotency, venue-adapter dispatch, typed outcomes, persistence and reconciliation.
4. **Shared account/event services:** normalized account snapshots, market and instrument data, orders, deals, positions, health events, audit records and tenant-scoped streams.
5. **Shared security and policy:** tenant isolation, product-scoped permissions, account and portfolio limits, secrets lifecycle, environment separation and consistent audit.

### Product integration contract

Products request capabilities through versioned internal/public contracts. The Journal can consume read-only account and trade events; products that can create exposure (including strategies, copier/Risk Router and an Agent with trading permissions) must route all actions through the shared risk engine and OMS. No product-specific privileged execution bypass is allowed. Multi-account fan-out is non-atomic, each account is authorized independently, and every result must be reconciled.

### Architecture release gate

The shared platform is a target design, not proof of production readiness. Before dependent products are enabled for broker execution, prove gateway-to-broker lifecycle on real MT5 demo environments: identity binding, fresh account/market/instrument data, protected credential/certificate lifecycle, disconnect/restart recovery and authoritative reconciliation. Demo order execution is permitted only after existing QOS-013/014/015 gates pass. Do not mark APIs, automated provisioning or product integrations complete without implementation and test evidence.


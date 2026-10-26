# Quant OS — Architecture
Last updated: 2026-10-27. `RULES.md` overrides conflicts. Target design is not a claim of current implementation.

## Existing baseline (source must be re-verified before code changes)
Repository documentation describes a Rust/Tokio workspace, Python strategies emitting `TradeIntent`, `rust_decimal`, PostgreSQL/SQLx, and a Next.js/TypeScript frontend documented as mock-first. It documents `VenueAdapter`, `SimulatedVenue`, and a prop-paper path; prop live mode is documented as `NotImplemented`; vault accounts are reserved. These are baseline notes, not proof of live capability.

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
**MT5:** versioned EA/terminal protocol; request/correlation/client-order IDs, account identity, timestamps/units, acknowledgement, heartbeat, reconnect, and reconciliation. ZeroMQ is a candidate, not a settled decision.
**cTrader:** isolate OAuth/token lifecycle and Protobuf/API details; least privilege and rate-limit handling.
**CEX:** integrate one provider first; handle exchange filters, precision, fees/funding, rate limits, sequence gaps, and ambiguous submits.
**On-chain later:** model signing, nonce, pending, revert, and finality; reconcile wallet/protocol state; signing keys never enter strategy/frontend.

## Security and frontend
No secrets in source, logs, events, frontend, or strategy payloads. Tenant isolation and least privilege gate SaaS. Live mode must be explicit. Frontend labels mock/paper/live and shows unknown/stale/partial states; money wire values are strings. Never show success before venue confirmation.

## Verification
Unit/property/invariant tests; adapter contract tests; simulator/failure injection; restart/replay/reconciliation tests; paper forward run; controlled live pilot. Green CI alone does not prove financial correctness.

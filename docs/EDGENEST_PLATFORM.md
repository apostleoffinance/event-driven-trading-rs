# EdgeNest Platform Strategy and Build Guide

**Status:** Target architecture and implementation guide; not evidence that the public API, managed terminal provisioning, or live execution is implemented.  
**Authority:** `RULES.md` remains authoritative for safety and engineering workflow. This document complements `PRD.md`, `ARCHITECTURE.md`, `ROADMAP.md`, and `docs/frontend-api-contract.md`.

## 1. Product definition

EdgeNest is a trading infrastructure platform with two first-party products built on shared services:

1. **Trader Dashboard:** users connect eligible accounts, configure risk, submit a single trade request, and monitor independent account outcomes.
2. **Developer Platform:** external developers use EdgeNest's versioned API and event streams to build trading applications, bots, analytics, execution systems, and copy-trading products without using the EdgeNest frontend or managing MT5 terminals themselves.

The dashboard is one API client, not the execution engine. Both first-party and third-party clients must pass through the same authentication, authorization, account eligibility, risk, OMS, adapter, persistence, and reconciliation paths.

## 2. Shared platform capabilities

- Tenant-scoped identity, authentication and authorization.
- Account registry and connection/provisioning lifecycle.
- Platform/broker/server metadata and immutable account-to-terminal identity binding.
- Managed MT5 terminal fleet and authenticated gateway protocol.
- Canonical, versioned REST and WebSocket/SSE API contracts.
- Per-account risk authorization and sizing through the Rust risk engine.
- OMS with account-scoped client-order IDs and durable idempotency.
- Typed order acknowledgements, fills, rejections and unknown outcomes.
- Append-only audit and financial records; exact decimal financial values.
- Account snapshots, market/instrument metadata and reconciliation.
- Developer keys, scopes, rate limits, quotas, usage metering, documentation and SDKs.
- Operational monitoring, alerting, deployment/rollback procedures and incident response.

## 3. MT5 connectivity strategy

EdgeNest should own its connectivity orchestration and public API. Hosted MT5 API vendors may be used for market comparison, temporary experiments or a fallback, but should not become a permanent core dependency by default.

Evaluate one implementation path at a time:

### Path A — Official MetaTrader5 Python integration

- Run MT5 terminal instances on EdgeNest-managed Windows hosts.
- Use the official Python integration to communicate with a running terminal.
- Build a supervised worker that binds each configured terminal to an immutable account/server identity.
- Expose only a versioned, authenticated protocol to the Rust backend.
- Supervise terminal/worker process health, restart and reconnect; reconcile before restoring readiness.

### Path B — EdgeNest-managed MQL5 bridge

- EdgeNest provisions an EA into its own managed terminal; customers do not attach EAs or maintain computers.
- The EA communicates with an authenticated gateway using a versioned protocol.
- Protocol messages include schema version, request/correlation IDs, account/terminal/server identity, timestamps, sequence and explicit units.
- The EA reports heartbeats, account state, orders, positions, instrument metadata, quotes, and typed execution outcomes as capabilities are implemented and verified.

Do not fully build both paths in parallel. Select based on real Windows demo conformance, supported broker builds, account identity guarantees, restart recovery, security review, operational cost and testability. ZeroMQ or any other transport is a candidate until selected and reviewed.

A managed terminal is the MT5 application running on EdgeNest-controlled infrastructure. A bridge is the software/protocol that carries authenticated commands and observations between EdgeNest and that terminal. They are separate components.

## 4. Initial connectivity alpha

First validate two distinct broker demo environments, for example FundedNext MT5 demo and TenTrade MT5 demo, after verifying the exact programs' rules and account permissions.

Initial scope:
- Read-only account snapshots, open orders, positions, instrument specifications and fresh quotes.
- Exact account, terminal, broker-server and currency identity binding.
- Authenticated transport, protected certificates/secrets and least-privilege host configuration.
- Persisted sequence checkpoints and reconciliation snapshots.
- Terminal/worker restart, disconnect and reconnect recovery.
- Redacted logs and an independently reviewed evidence record.

Do not add order execution until existing QOS-013, QOS-014 and QOS-015 gates have passed, including actual Windows host conformance and protected credential/certificate lifecycle review. Hosted CI and local mocks do not prove real terminal compatibility.

## 5. Execution semantics

When a user or API client submits a parent trade request:

1. Authenticate and authorize the tenant and caller.
2. Validate request schema, instrument, account selection and idempotency key.
3. Select only accounts whose connection, policy, platform and instrument capabilities are eligible.
4. Evaluate risk independently for each account using fresh account/market/instrument data.
5. Persist the decision and stable account-scoped child order identity before dispatch.
6. Submit each approved child through the OMS and adapter.
7. Record each account's independent accepted/rejected/unknown/partial-fill result.
8. Reconcile broker orders, deals, positions and account state before clearing any uncertainty.

Multi-account fan-out is non-atomic. A successful socket write is not proof of broker acceptance; acceptance is not proof of a fill. An ambiguous submit remains `Unknown` until reconciled and must never be blindly retried. Do not promise simultaneous fills or guaranteed loss prevention.

## 6. Public developer API

The initial API should cover:
- Account connection lifecycle and connection status.
- Account snapshots and supported instruments.
- Order submission and order status/history.
- Positions, deals and reconciliation.
- Risk decisions and account eligibility reasons where safe to expose.
- Tenant-scoped event streams/webhooks.
- API key creation/revocation, usage and quota metadata.

All routes in `docs/frontend-api-contract.md` remain proposed until the server implementation and tests exist. Tenant identity comes from authenticated claims, not a client-supplied tenant ID. API keys are scoped, revocable and stored as hashes where feasible; secret material is returned only at creation and never logged. Money values use decimal strings. Streams require explicit sequence/cursor and reconnect/recovery semantics.

The dashboard must use the same contracts available to external developers and must not have a privileged execution bypass.

## 7. Tenant and account isolation

- A tenant can access only its own users, API keys, accounts, orders, positions, events, logs and usage.
- Account IDs supplied by clients are selectors, not credentials.
- A configured terminal principal may operate only the explicitly mapped terminal/server/account set.
- Never expose broker passwords, private keys, TLS private-key material or internal host credentials to frontend code, strategies, API responses, logs or events.
- Disconnection, credential revocation, account identity mismatch, stale data or unknown compatibility must fail closed for new exposure.
- Demo, paper and live environments are explicitly separate. Live mode is unavailable until a separately approved release gate passes.

## 8. Provider and platform adapter strategy

The public API and Rust domain should not expose provider-specific types as canonical business objects. Use a capability-aware adapter boundary. Keep MT5-specific protocol and broker mapping inside the MT5 adapter. Add cTrader or other platforms only when access, permissions, authentication and conformance can be tested. A connected account is not automatically permitted to use automation or copying; compatibility and applicable broker/prop-program rules must be checked.

## 9. Release gates

1. Unit/property/invariant tests and adapter contract tests.
2. Simulator and deterministic failure injection.
3. Real Windows MT5 demo conformance on two broker environments.
4. Restart/reconnect and authoritative reconciliation evidence.
5. Protected credential/certificate provisioning, ACL, rotation, revocation and rollback review.
6. QOS-013 risk input/sizing evidence and QOS-014 transport/dispatch evidence.
7. Controlled demo order tests only after the existing release gates permit them.
8. Tenant-isolation, API authentication, rate-limit, stream recovery and audit tests before external developer beta.
9. Explicit approval, monitoring, rollback and legal/contractual review before any live rollout.

A green CI run alone does not establish terminal, broker, risk, security or production correctness.

## 10. Non-goals for the first alpha

- Public live-trading access.
- Customer-managed EA installation.
- Supporting every broker/server/build immediately.
- Building MT5 and cTrader execution simultaneously.
- Replacing the Rust risk engine with provider-side copier sizing.
- Launching vaults or pooled capital as part of connectivity validation.
- Claiming public REST/WebSocket APIs, automated provisioning or live execution are implemented before evidence exists.

## 11. Success criteria

The alpha succeeds when two distinct demo broker environments can be independently identified, monitored and reconciled on EdgeNest-managed infrastructure; reconnect and restart do not lose account identity or financial state; all failures are explicit; and the same internal service boundary can later serve the dashboard and public developer API without a redesign.

## 12. Shared infrastructure across all EdgeNest products

**Architecture rule: all EdgeNest products use one shared platform foundation. Product definitions and product scope remain unchanged by this rule.** The Trader Dashboard, Developer Platform, Risk Router / multi-account copier, Risk Co-Pilot, strategy products and any future EdgeNest products must reuse the same account registry, connectivity control plane, tenant and permission model, risk authorization, OMS/execution boundary, venue adapters, event stream, persistence, audit and reconciliation services wherever those capabilities apply.

Products are separate user experiences and workflows, not separate trading backends. A product must not create its own MT5 connection mechanism, keep a parallel source of financial truth, bypass the Rust risk engine, or dispatch broker orders outside the shared OMS/adapter path. The dashboard and developer API are clients of this platform; other products integrate as first-party clients or controlled internal services through versioned contracts.

### Shared service responsibilities

- **Connectivity control plane:** connect/disconnect lifecycle, tenant/account ownership, broker/server metadata, immutable account-to-terminal identity binding, provisioning, health and capacity.
- **Managed gateway data plane:** EdgeNest-operated terminal sessions and authenticated bridges. Customer-facing flows must not require customers to attach an EA to a chart or manage a personal VPS; if an EA bridge is selected, EdgeNest installs and operates it within its own managed terminal fleet.
- **Execution boundary:** one authenticated, idempotent order path through eligibility checks, per-account risk authorization, OMS, adapter dispatch, typed outcomes and reconciliation.
- **Event and account data services:** normalized account snapshots, orders, deals, positions, market/instrument data, connection health and auditable event delivery.
- **Shared policy and tenancy:** scoped product permissions, per-account and portfolio controls, tenant isolation, environment separation and consistent audit records.

### Product integration rule

Each product declares the shared capabilities it needs and its permissions; it does not own a second broker connection or execution implementation. The Journal may operate read-only; strategy, copier, Risk Router and Agent workflows that create exposure must use the shared risk and execution boundary. Agents and strategies cannot bypass policy checks. Account fan-out remains non-atomic, with independent authorization and explicit per-account outcomes.

### Implementation priority

Prove the gateway-to-broker lifecycle end to end before treating this platform as production-ready: real target-host MT5 demo identity binding, fresh data, secure credentials/certificates, reconnect/restart recovery, authoritative reconciliation, then controlled demo execution only after existing QOS-013/014/015 gates pass. Public APIs, automated gateway provisioning and product integrations remain planned until implemented and evidenced.


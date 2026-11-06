# Phase 0 — Architecture and Readiness Audit

**Audit date:** 2026-11-06  
**Repository:** `apostleoffinance/event-driven-trading-rs`  
**Audited source ref:** `main` at `2e2973c86889de5e7ffdf15ba2350724ce27ecca`  
**Method:** GitHub source/documentation inspection, existing architecture and QOS records, and operator-reported local validation. No production host, broker account, or external deployment was accessed as part of this audit.

> **Decision: NOT READY for live broker execution or broad external SaaS launch.**  
> **Ready for continued development and controlled read-only MT5 demo conformance work.**  
> The Rust modular core has a strong tested foundation. The biggest gaps are real-terminal proof, managed gateway operations, product authentication/tenant isolation, public API implementation, and production operations evidence.

## 1. Evidence and scope

This audit covers:
- Rust domain, strategy intent, risk, account, OMS, adapter, runtime, persistence, reconciliation, and MT5 bridge boundaries.
- Frontend data mode and API contract status.
- Implementation vs. target architecture and operational readiness.

Evidence considered:
- Main branch source and docs at the audited ref.
- `README.md`, `docs/architecture/CONSTITUTION.md`, architecture docs, `PRD.md`, `ROADMAP.md`, `docs/EDGENEST_PLATFORM.md`, `docs/frontend-api-contract.md`, `BASELINE_AUDIT.md`, `TASKS.md`, `PROGRESS.md`, and MT5 bridge/security docs.
- Operator-reported local run on 2026-11-06: `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` passed with PostgreSQL configured. The two MT5 reconciliation recovery tests also passed against local PostgreSQL.

**Evidence boundary:** local tests and CI prove behavior in their test fixtures. They do not prove real broker terminal behavior, deployment configuration, credential lifecycle, tenant isolation in a running service, backup restoration, or live execution.

## 2. Executive summary

| Area | Status | What is supported by evidence | Remaining readiness gap |
|---|---|---|---|
| Rust domain + event contracts | Implemented / tested | Strong typed domain, decimal money, event envelopes, intent contract | Broader production contract compatibility and operational audit |
| Strategy runtime | Implemented / tested | Strategy emits intent; no direct broker submission | Production strategy isolation, deployment lifecycle |
| Risk engine | Implemented / tested | Deterministic checks, sizing, halt/reject/resizing, fail-closed tests | Broker-specific calibration/conformance; complete validated input pipeline before exposure |
| Account engine | Implemented / tested as core | Account/venue separation, account state and controls | Canonical auth/tenant ownership service and account onboarding are not proven |
| OMS | Implemented / tested | State machine, idempotency, fills/cancel lifecycle, audit/risk decision linkage | Production adapter outcome semantics and end-to-end venue proof |
| Simulated / prop-paper venues | Implemented / tested | Paper flows and end-to-end pipeline tests | Not broker execution; prop live mode remains gated/not implemented |
| PostgreSQL persistence | Implemented / tested | SQLx migrations, durable order/trade state, snapshots, restart recovery | Production backup/restore, runtime/migration role split, HA and operational recovery evidence |
| Reconciliation | Implemented / tested | Internal-vs-venue comparison; alert-only behavior; durable MT5 snapshots and recovery | Reconciliation against real terminal/broker truth and operational response process |
| MT5 bridge | Partial implementation | TLS 1.3 mTLS transport, identity binding, read-only daemon, protocol, heartbeat/watchdog, persisted state and test certificates | Real terminal and broker conformance; deployment/credential lifecycle review; stable restart/reconnect proof |
| MT5 execution | Not available through current read-only daemon | Explicitly documented read-only bridge boundary | Do not add/enable order execution until existing QOS release gates and separate review pass |
| Frontend | Mock-first / PAPER | Next.js control console, mock repositories and mock stream | Real Rust backend/API provider integration; UI must not imply live service state |
| REST / WebSocket / SSE developer API | Proposed / not implemented as a complete service | API contract documentation | HTTP service, auth, tenant authorization, streams, rate limits, API keys, contract tests |
| Tenant auth / developer credentials | Not established | Target architecture describes desired identity and auth | Implementation and isolation/security evidence required |
| Managed MT5 fleet / provisioning | Target architecture | Strategy and runbooks describe desired lifecycle | Provisioning, supervised fleet, credential rotation, capacity and monitoring |
| Multi-account fan-out | Partial/core implementation | Runtime has per-account fan-out tests and independent results | End-to-end connected-account eligibility, durable per-account outcomes and product/API integration |
| Production operations | Not ready / not evidenced | Some deployment and rollback plans exist | Environment inventory, alerting, restore drill, incident runbooks, security review, sign-off |

## 3. Architecture verification

### Intended authoritative flow

```text
Market data + health
  → Strategy (TradeIntent only)
  → Instrument compatibility / broker inputs
  → Rust risk authorization and sizing
  → OMS (idempotent order lifecycle)
  → Venue adapter / bridge
  → Venue acknowledgement and fills
  → Persistence / positions
  ↔ Reconciliation (never silently overwrite)
```

### Verified architectural strengths

- Strategy and execution responsibilities are separated.
- Rust risk and OMS are intended to remain authoritative; frontend and strategy code are not execution authorities.
- Account and venue are separate concepts.
- Financial state uses Decimal / PostgreSQL NUMERIC / decimal-string wire formats.
- Ambiguous submit outcomes preserve `Unknown` rather than assuming failure.
- Idempotency and payload-conflict behavior have tests.
- Stale/gapped market data and missing safety inputs fail closed.
- Reconciliation reports mismatches rather than silently rewriting financial state.
- MT5 transport identity, heartbeat, sequence and durable reconciliation checks have meaningful test coverage.
- The latest operator run passed format, workspace check, strict Clippy and full workspace tests with PostgreSQL configured.

### Architecture deviations / gaps to close

1. **Platform identity is still a target-level gap.** Account IDs and terminal/account bindings in core code are not equivalent to an implemented authentication, tenant, authorization, account onboarding and ownership service.
2. **The API is still a contract, not a production service.** `docs/frontend-api-contract.md` explicitly calls endpoints proposed. The frontend README says mock repositories and mock event stream are current.
3. **MT5 bridge is read-only.** The daemon and provider path must not be represented as broker order execution. Live/real terminal conformance is not proven.
4. **MT5 sizing needs target-environment validation.** Existing instrument-aware sizing and persisted quote consumers are useful implementation; final acceptance depends on real broker instrument metadata, conversion/cost/slippage, margin/exposure, volume-step/rounding, freshness, modeled loss, and audit inputs.
5. **Venue abstraction and deployment capabilities are not yet a complete product connectivity layer.** Health, capabilities, instrument specs, asynchronous events, broker-specific semantics, provisioning, and recovery must be implemented/validated where not yet covered.
6. **Frontend operational truth is not connected to Rust service truth.** Mock state, simulated events, and any UI mode labels must be visibly distinguished from real account/venue state.
7. **Production persistence/operations are unproven.** SQLx migrations and local tests do not prove backup restoration, least-privilege runtime roles, HA, secrets handling, production monitoring, or incident response.

## 4. Implemented vs. incomplete vs. target-only

### Implemented and supported by code/tests

- Rust modular workspace and domain primitives.
- Strategy-to-TradeIntent separation.
- Risk evaluation and sizing core with fail-closed tests.
- Account engine core and account state snapshots.
- OMS order lifecycle, idempotency, audit and risk decision association.
- Simulated and prop-paper venue pipeline.
- PostgreSQL SQLx persistence and local integration tests.
- Reconciliation core and MT5 durable snapshot recovery.
- MT5 protocol/transport foundations: mTLS, authenticated identity binding, heartbeat/watchdog, durable state, read-only daemon.
- Frontend mock-first control-plane shell.
- Strict Clippy and workspace test validation on the operator’s local checkout (reported 2026-11-06).

### Partially implemented; integration evidence required

- MT5 QOS-013/014/015A/015B connectivity alpha.
- MT5 account/server/terminal identity binding and data freshness under actual terminal/broker behavior.
- Read-only data pipeline from a real terminal through persistence/reconciliation.
- Multi-account runtime orchestration (core exists; full product path and connected account lifecycle not proven).
- Risk sizing with actual broker-derived data and all required cost/margin/conversion inputs.
- Runtime health and reconnect/restart recovery across real deployment environments.
- Frontend provider abstraction intended to move from mock to API-backed state.

### Target architecture / not to be claimed as implemented

- Public versioned REST API and WebSocket/SSE developer service.
- Tenant-scoped production authentication/authorization and developer API key lifecycle.
- Managed MT5 fleet provisioning, per-account terminal orchestration, automated capacity management and rotation.
- Broad product integrations using real shared services.
- Production SaaS billing/usage metering/support/legal/privacy hardening.
- Live broker execution through the current read-only MT5 daemon.
- Production readiness / general availability.

## 5. Safety-critical blockers

| Priority | Blocker | Why it matters | Evidence required to close |
|---|---|---|---|
| P0 | Real MT5 terminal/demo conformance | Local mocks/certificates cannot prove terminal protocol, broker identity or quote semantics | Approved demo account test on target OS/host; redacted evidence; exact build/EA/worker/daemon hashes |
| P0 | Account/terminal identity binding | Wrong-account execution/data association is unacceptable | Independent proof of broker/server/login/terminal binding, account switch and reconnect rejection tests |
| P0 | Reconnect and restart recovery against external truth | Session reauth alone cannot establish reconciliation readiness | Disconnect/restart test; fresh authoritative account/order/position snapshot; stream sequence continuity; no readiness before reconciliation |
| P0 | Broker-specific risk inputs | Bad tick/contract/FX/margin/volume assumptions can mis-size trades | Broker symbol metadata and account currency; conversion path; margin/exposure and cost/slippage inputs; exact final modeled-loss verification |
| P0 | Credential/certificate lifecycle | Authentication can fail operationally or expose terminal access | Protected storage/ACL, least privilege, provisioning, rotation/revocation, recovery and independent review |
| P0 | Tenant/auth ownership | API users must not read/control another user’s accounts | Server-side auth claims, authorization checks on every account/object, isolation tests, key scopes and revocation |
| P0 | Production data protection | Trading/journal/account records require recoverability | Approved backups and successful restore drill; retention; migration role separation; recovery point/time objective |
| P1 | API and frontend truthfulness | UI mock state must not be mistaken for real trading state | Rust API implementation, contract tests, stream recovery, mock/live mode guardrails, tenant tests |
| P1 | Operational readiness | Failures must be detectable and recoverable | Health/alert dashboards, on-call runbooks, incident/rollback plans, log redaction and deployment evidence |

**No live broker execution, production migration writes, or external exposure-changing API should be enabled while any P0 remains open.**

## 6. Prioritized implementation plan

### Phase 0A — Evidence and control baseline (this audit)

- [x] Source/documentation audit.
- [x] Identify implemented, partial, and target-only capabilities.
- [x] Record local operator validation from 2026-11-06.
- [x] Identify safety blockers and evidence needed.
- [ ] Record exact deployed main/RC/environment commit(s) for every running component.
- [ ] Reconcile all status documents and QOS task records against latest exact-head evidence.
- [ ] Obtain owner/reviewer and environment/change-window sign-off where needed.

### Phase 1 — MT5 Connectivity Alpha (read-only; existing priority)

Do not begin with order submission.

1. Select and document one terminal integration path for the first conformance target; avoid building two complete paths in parallel.
2. Establish a dedicated demo environment and exact build manifest: host OS, terminal build, broker/server, EA/worker/daemon versions, config hash and certificate fingerprint (no secrets).
3. Verify account/server/login and instrument identity; include account switch and wrong-principal tests.
4. Verify fresh quotes, instrument metadata, account state, open orders and positions.
5. Persist snapshots and verify sequence continuity, timestamps and restart recovery.
6. Test worker/terminal/daemon disconnect and restart; readiness remains degraded until a fresh authoritative reconciliation completes.
7. Review secret/certificate storage, ACLs, rotation/revocation and logs.
8. Collect independent, redacted evidence and close QOS-013/014/015A/015B criteria.

**Exit:** read-only conformance evidence is accepted on the intended managed host. No broker order execution is part of this phase.

### Phase 2 — Risk and execution integration hardening

- Verify broker-specific sizing inputs and final modeled loss under real demo metadata.
- Add/test capability-aware adapter contracts and authoritative terminal outcome semantics as required.
- Preserve idempotency, unknown outcomes, cancel-confirmation semantics and reconciliation on every failure path.
- Exercise per-account fan-out, partial success, account eligibility, persistence and independently reconciled outcomes.
- Keep all live execution paths disabled until a separate execution release gate is approved.

### Phase 3 — Shared API and product data path

- Implement tenant-authenticated backend service before wiring the frontend to real data.
- Add account/object authorization, scoped API credentials, revocation, quotas, idempotency and stable error contract.
- Add REST and event stream with cursor/sequence recovery, tenant-scoped subscriptions and authorization renewal.
- Build frontend API provider integration; mock/demo/paper/live environments must be distinct and explicit.
- Add tenant isolation and end-to-end contract tests.

### Phase 4 — Production operations and SaaS hardening

- Separate migration and runtime DB roles; review privileges.
- Establish environment inventory, secret/certificate lifecycle, monitoring, alerting, incident response and rollback.
- Run backup/restore and disaster recovery drills.
- Add security/privacy/data-retention and external API abuse review.
- Conduct independent release review before any controlled execution launch.

## 7. Acceptance gates

### Gate A — Read-only MT5 demo readiness

- [ ] Exact target host/build and deployment manifest captured.
- [ ] Broker/server/account identity independently verified.
- [ ] Fresh quote and instrument metadata validated.
- [ ] Account/order/position snapshot validated and persisted.
- [ ] Restart/reconnect recovers only after authoritative reconciliation.
- [ ] No duplicate/incorrect account binding across switch/reconnect.
- [ ] Credential/certificate lifecycle review passed.
- [ ] QOS-013/014/015A/015B acceptance evidence approved.

### Gate B — Exposure-changing execution

- [ ] Gate A passed.
- [ ] Broker-specific risk sizing and margin/cost inputs validated.
- [ ] Order submit/ack/fill/cancel/unknown semantics tested on demo.
- [ ] Idempotency, ambiguous outcome, failure injection and reconciliation tests passed.
- [ ] Kill-switch and account eligibility behavior verified.
- [ ] Independent safety review and explicit execution release approval.

### Gate C — External developer API

- [ ] Tenant auth and account/object authorization implemented.
- [ ] Tenant isolation and credential scope/revocation tests passed.
- [ ] API and event-stream contract/recovery tests passed.
- [ ] Rate limits, audit, secrets handling and abuse controls reviewed.
- [ ] Frontend uses real shared API contracts; mock mode is clearly labeled.
- [ ] Production operations and rollback readiness approved.

## 8. Non-goals for this audit

- No live broker execution.
- No deployment or terminal restart.
- No production database mutation or migration.
- No source data deletion or TraderOS migration writes.
- No secrets/credentials collection.
- No enabling of public API write operations.
- No frontend change that presents mock state as real account state.

## 9. Final readiness verdict

| Readiness dimension | Verdict |
|---|---|
| Local Rust engineering baseline | **PASS** (operator-reported fmt/check/strict Clippy/workspace tests, 2026-11-06) |
| Core architecture foundations | **PASS WITH GAPS** |
| MT5 read-only integration in code | **PARTIAL** |
| Real broker/terminal conformance | **NOT PROVEN** |
| Live execution | **NOT READY / NOT IMPLEMENTED THROUGH CURRENT READ-ONLY DAEMON** |
| Production tenant API | **NOT IMPLEMENTED / NOT READY** |
| Frontend connected to live backend | **NOT IMPLEMENTED; MOCK-FIRST** |
| Production operations | **NOT PROVEN** |
| General SaaS launch | **NOT READY** |

**Recommended immediate next work:** continue the existing Phase 1/QOS MT5 read-only connectivity acceptance on the exact intended terminal host, while preserving the strict quote freshness and reconciliation gates. Do not start a public API, product integration, or live execution rollout ahead of identity, data, and connectivity proof.

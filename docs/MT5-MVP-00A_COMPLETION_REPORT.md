# MT5-MVP-00A Completion Report

**Date:** 2026-11-12  
**Final recommendation: BLOCKED for MT5-MVP-01A implementation.** The five requested architecture documents are complete, but critical implementation dependencies remain unresolved: Windows host/provider, credential and PKI services, platform verification authority, enforced registry-backed daemon admission, DEMO environment policy approval, and current QOS-020 live access. No 01A implementation began.

## 1. Scope and authorization

This was an architecture and documentation-only phase. Created:

1. `docs/MT5-MVP-00A_ADR_BROKER_ENVIRONMENT_MODEL.md`
2. `docs/MT5-MVP-00A_VERIFIED_BROKER_BINDING_ADMISSION.md`
3. `docs/MT5-MVP-00A_WINDOWS_CREDENTIAL_CERTIFICATE_LIFECYCLE.md`
4. `docs/MT5-MVP-00A_PROTOCOL_V2_COMPATIBILITY_AND_DEFERRED_FIELDS.md`
5. `docs/MT5-MVP-00A_WINDOWS_GATEWAY_DEPLOYMENT_DESIGN.md`
6. `docs/MT5-MVP-00A_COMPLETION_REPORT.md`

All six destinations were absent at preflight. No code, migration, database, Docker resource, credential, certificate, Windows host or deployment was modified. No tests, builds, package installation, Docker inspection, database connection, terminal access, commit or push occurred.

## 2. Worktree preservation

| Evidence | Result |
|---|---|
| Root / branch / HEAD | `/Users/olaoluwatunmise/event-driven-trading-rs`; `main`; `f6446dd33dc74a86ede9064f35aef2ec082a1c59` |
| Baseline Git status | 110 entries, saved at `/private/tmp/mt5-mvp-00a-status-before.txt`, SHA-256 `17e778f5e95913d869afb3bd386d9ac225b9490bb73f4bb9df957853d1d7eda5` |
| Ignored inventory | 222,699 `!!` entries, saved at `/private/tmp/mt5-mvp-00a-ignored-before.txt`; SHA-256 of normalized ignored status lines `1ff74a6ef369b2e388fd59168e704fcf57aca536896ef0e852d40089b049b889` |
| Final comparison | 116 entries. Zero baseline entries removed or changed; exactly the six requested documents added. Final status snapshot SHA-256 `dca50f53c721419b75ab0186841ca7d7913d3df1bb94b2f64f569ecea2ac99e9`. Ignored-only inventory unchanged. |

No destructive Git operation or broad formatting command was used. The complete before/after status comparison is performed after document creation and reflected in the final repository status.

## 3. Repository paths and symbols inspected

- Worker/provider: `adapters/mt5-python/mt5_adapter/worker.py` (`Config.from_env`, `MetaTraderTerminal.connect`, `SequenceStore.next`, `build_snapshot`, `build_market_data_snapshot`, `build_heartbeat`, `tls_context`, `run_worker`, `load_terminal_runtime`); `adapters/mt5-python/README.md`; `pyproject.toml`; macOS provider and `mql5/QuantOSReadOnlyProvider.mq5`.
- Protocol/bridge: `crates/intent-bridge/src/mt5_v1.rs` (`MT5_BRIDGE_SCHEMA_VERSION`, `BridgeFrame`, `NdjsonFrameDecoder`, `validate_at`); `mt5_transport.rs` (`Mt5TransportPrincipal`, `Mt5TransportConfig::from_pem_files`, `authenticate`); `mt5_session.rs` (`Mt5SessionGate`); `mt5_runtime.rs`; `mt5_reconciliation.rs`; `bin/mt5-bridge-daemon.rs::main`.
- Persistence/identity: `crates/persistence/src/repos/mt5_bridge.rs`, `repos/mt5_market_data.rs`, `repos/broker_registry.rs`; `crates/domain/src/broker_registry.rs`; `crates/journal-api/src/broker_registry.rs` (`BrokerIdentityHmac`, `register`, `transition`).
- Schema/architecture: migrations `20261027000002_mt5_bridge_idempotency.sql`, `20261027000003_mt5_reconciliation_snapshots.sql`, `20261027000004_mt5_market_data_snapshots.sql`, `20261107000002_dashboard_account_access.sql`, `20261111000002_account_environment_provenance.sql`, `20261112000001_organization_tenant_foundation.sql`, and `20261113000001_broker_account_registry.sql`; `docs/QOS-021A-E3A-R3A_BROKER_VERIFICATION_AUTHORITY_AND_EVIDENCE_ARCHITECTURE.md`; `docs/MT5-MVP-01A_READ_ONLY_DATA_CONTRACT.md`; `docs/MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md`; `docs/MT5_BRIDGE_V2.md`; Windows smoke, restart and certificate ACL scripts.

## 4. Five architecture decisions

### ADR-01 — Separate environment dimensions

Keep broker `DEMO/LIVE/UNKNOWN` separate from Quant `PAPER/SHADOW/LIVE`. Recommend explicit versioned eligibility policy: only verified DEMO/PAPER for initial read-only 01A; unknown is denied, DEMO/LIVE and DEMO/SHADOW are denied, and live cases require separate authorization. A server/account/environment revision change invalidates admission. Current `broker_servers.environment` classifies a server, while `external_broker_accounts` has no per-account environment/provenance field; add a future separately authorized per-account environment verification record/event rather than overloading `account_category` or Quant account environment. **Proposed for review; no environment migration or implementation.**

### ADR-02 — Registry-backed admission

Keep the current broker registry as sole ownership authority. Add a narrow bridge admission adapter that checks organization, entitlement, Quant environment, external account HMAC, evidence, verification and binding revisions, broker/server/environment, terminal assignment, and mTLS workload assignment before any data persistence. Revalidate live sessions every proposed five seconds, fail closed on registry outage or revision change, and terminate stream. `ACTIVE` is derived eligibility, not a new persisted status. Recommend two-person evidence approval; current platform verification authority is not implemented. **Proposed for review; no schema or daemon changes.**

### ADR-03 — Separate credential and certificate lifecycles

Reject ordinary long-lived `MT5_PASSWORD` environment variables for production. Prefer a cloud secret manager accessed through host workload identity; enterprise vault is an acceptable provider-neutral alternative. Windows Credential Manager/DPAPI alone does not provide lifecycle, central audit or revocation. Use distinct broker secret, worker private key, daemon private key, trust roots and ordinary config. Prefer non-exportable Windows CNG/HSM keys where supported, but current Rust expects PEM files, so that requires a future integration decision. **Provider/CA not selected; blocks 01A-2.**

### ADR-04 — Preserve strict v2 subset

Keep `schema_version: 2` for currently supported fields. Do not silently add optional provider values to strict `deny_unknown_fields` frames. Carry minimum account/binding admission through a separately versioned authenticated control-plane exchange, not repeated market data. Defer unsupported fields and only consider v3 if a required semantic cannot be safely added otherwise. Pin worker/daemon artifacts and protocol together. **Proposed compatibility decision; no protocol changes.**

### ADR-05 — One managed Windows gateway

Recommend one dedicated managed Windows VM/VPS class, one terminal/profile, one worker, one daemon and one DEMO account. Keep provider choice neutral until host/cloud is chosen. Use a supervisor compatible with the terminal's session needs; SCM may supervise daemon, while terminal/worker supervision must be validated for interactive-session behavior. Enforce a single-instance lock, bounded backoff, separate health predicates, registry revalidation and reconciliation after every restart. No automatic failover. **Provider/supervisor not selected; blocks 01A-2 and 01A-3.**

## 5. Security invariants

- Data collection only; no submit, modify, cancel or close operations. The inspected runtime `dispatch_submit` method returns a deny-all error; the daemon loop does not call it and the worker sends only read-only frames. Preserve this behavior.
- mTLS identifies workload, not broker account ownership.
- Unverified binding, inactive entitlement, unknown/mismatched environment, stale revision or registry outage denies admission and closes active stream.
- Broker DEMO/LIVE and Quant PAPER/SHADOW/LIVE remain separate.
- Credentials never appear in logs or committed configuration; broker password and certificate/key lifecycle remain independent.
- Sequence is scoped to account/terminal; one worker per stream; no duplicate worker.
- Transport liveness, market freshness and reconciliation readiness are separate states.
- No production/staging database use or automatic failover.

## 6. External decisions and 01A dependencies

| Dependency | Blocks |
|---|---|
| Approve ADR-01 matrix and DEMO-to-PAPER eligibility; decide LIVE observation policy | 01A-1 |
| Select platform broker-verification authority, two-person approval roles, evidence repository/retention and server revision process | 01A-1 |
| Design schema/workload assignment representation and authenticated admission exchange; implement registry-backed daemon lookup, revision invalidation, lease and active-stream closure | 01A-1 |
| Select Windows host/provider, supported OS, workload identity, secret manager, CA/PKI and Windows key storage; validate official MT5 terminal session model | 01A-2 |
| Resolve worker password retrieval away from ordinary environment; certificate revocation/rotation propagation and ACL procedures | 01A-2 |
| Freeze v2 compatibility/admission protocol/artifact manifest and field deferrals | 01A-3 |
| Restore current QOS-020 verification in a separately authorized phase; run guarded database acceptance on exact approved disposable lane | 01A-3 and 01A-4 |
| Complete Windows DEMO acceptance matrix, including negative binding/cert/protocol cases, restart, reconnect, freshness, sequence, persistence and execution-disabled evidence | 01A-3 and 01A-4 |

## 7. QOS-020 limitation

Repository configuration and manifest identity were inspected. Manifest SHA-256 is `f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e`; guard SHA-256 is `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b`. Historical QOS-020-R3 passed for run `20261110-230504-ffb59dfd0ab4`, endpoint `127.0.0.1:51462`, database `qos020_test_master`, context `desktop-linux`.

MT5-MVP-00's current guard attempt failed at Docker identity inspection with exit 2. Therefore current live resource verification is **BLOCKED**, and current database identity/SQLx cleanup are **NOT VERIFIED**. This phase did not retry Docker, connect to a database, change context, or use a workaround. Verify the exact QOS-020 lane only in a separately authorized phase before database-capable 01A tests.

## 8. Recommended 01A engineering sequence

1. **01A-1 — Account environment and verified-binding admission:** approve ADR-01/02, implement narrowly scoped registry lookup/admission, binding and revision enforcement, periodic revalidation, stream lease and fail-closed revocation.
2. **01A-2 — Windows security and gateway foundation:** select host/provider and workload identity; integrate secret retrieval and certificate lifecycle; implement service identities, supervision, single-worker ownership and health-state reporting.
3. **01A-3 — Managed DEMO data vertical slice:** pin compatible v2 artifacts; use one DEMO/PAPER verified binding; connect official Python provider through TLS/mTLS; persist supported v2 snapshots to approved isolated DB; verify identity, sequence, freshness and reconciliation.
4. **01A-4 — Recovery and conformance:** test worker/daemon restart, terminal/network loss, certificate/password rotation, revoked/mismatched identity, malformed protocol, stale data, persistence recovery and continued execution disablement.

Each increment requires separate implementation authorization. No part began here.

## 9. Final recommendation

**BLOCKED for MT5-MVP-01A implementation.** The architecture can proceed to decision review, but implementation readiness requires explicit environment-policy approval, an enforceable platform verification authority and registry-to-daemon admission design, a selected Windows secret/PKI/host path, and a frozen v2 compatibility decision. Current QOS-020 live verification must also pass before any database-capable acceptance test. No source, migration, database, Docker resource, credential, certificate, Windows host, service, deployment or execution path was modified.

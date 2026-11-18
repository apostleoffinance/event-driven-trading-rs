# MT5-MVP-00 Completion Report

**Date:** 2026-11-12  
**Final recommendation: BLOCKED for MT5-MVP-01A.** The documentation review and v2 read-only field inventory are complete. Actual QOS-020 verification is currently blocked by Docker socket access; account binding is not enforced by the inspected daemon; the DEMO environment mapping and credential-delivery mechanism are unresolved; and the requested account/order/position contract contains fields absent from wire schema v2. No 01A implementation began.

## 1. Scope and authorization

This phase performed read-only repository and QOS-020 inspection and created the four authorized documentation files:

- `docs/MT5-MVP-00_TEST_LANE_VERIFICATION.md`
- `docs/MT5-MVP-01A_READ_ONLY_DATA_CONTRACT.md`
- `docs/MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md`
- `docs/MT5-MVP-00_COMPLETION_REPORT.md`

No existing file was overwritten. No implementation, migration, database operation, Docker lifecycle operation, build, test command, deployment, terminal/EA installation, or trading operation was performed.

## 2. Worktree preservation

| Evidence | Result |
|---|---|
| Root / branch / HEAD | `/Users/olaoluwatunmise/event-driven-trading-rs`; `main`; `f6446dd33dc74a86ede9064f35aef2ec082a1c59` |
| Baseline status | 106 entries in `/private/tmp/mt5-mvp-00-status-before.txt`; SHA-256 `964b831e700ac81f19a34a9d8fcba58bd3ab3407268311ebb08ff00c0eed480f` |
| Documentation destination preflight | All four requested paths were absent before writing |
| After status | 110 entries. Comparing captured before/after status sets found zero removed or changed entries and exactly the four requested documentation paths added. Final status snapshot SHA-256 `17e778f5e95913d869afb3bd386d9ac225b9490bb73f4bb9df957853d1d7eda5`. |
| Ignored work | The `!!`-only inventory has 222,699 entries before and after, byte-identical normalized status lines, SHA-256 `1ff74a6ef369b2e388fd59168e704fcf57aca536896ef0e852d40089b049b889`. |

All staged, unstaged, deleted, untracked, and ignored work is to remain intact. No destructive Git command or repository-wide formatter was used.

## 3. QOS-020 test lane result

**PARTIALLY VERIFIED / current runtime BLOCKED.** The tracked QOS-020 Compose, guard, provisioner, negative guard tests, initialization SQL, and infrastructure documentation were inspected. The external manifest identity fields match the expected run, with pinned image digest `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777`, loopback endpoint `127.0.0.1:51462`, master database `qos020_test_master`, and SQLx role `qos020_sqlx`. Manifest SHA-256 is `f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e`; unchanged guard SHA-256 is `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b`.

Current command:

```text
docker context show
=> desktop-linux
python3 scripts/qos020/qos020_guard.py --manifest /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json --secrets-dir /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 --verify-sqlx-cleanup
=> QOS-020 guard rejected command: CalledProcessError: docker inspect <expected-container-ID> returned non-zero exit status 1
=> exit 2
```

The Docker daemon socket is inaccessible to this process. The guard stopped at Docker identity inspection before its PostgreSQL identity and cleanup queries. No context switch, permissions change, privileged access, replacement resource, DB test, or protected database access was attempted. `docs/QOS-020-R3_READ_ONLY_GUARD_VERIFICATION.md` remains historical evidence that this same run passed previously; it is not current verification.

## 4. Existing implementation inspected

### Python provider

Primary Windows implementation is `adapters/mt5-python/mt5_adapter/worker.py`:

- `MetaTraderTerminal.connect` calls official `MetaTrader5.initialize(path, login, password, server)` and checks terminal connectivity plus configured login/server.
- `account_info`, `orders`, `positions`, `symbol_info`, `symbol_info_tick` wrap provider APIs and report `last_error` code without logging passwords.
- `Config.from_env` reads broker password from `MT5_PASSWORD`, certificate paths from config, and provider/account/terminal/server/symbol settings. No approved secret-manager integration was found.
- `build_snapshot` produces complete open order/position collections but maps only the fields enumerated in the contract; `build_market_data_snapshot` builds symbol specs and explicit per-symbol quote freshness.
- `SequenceStore.next` persists a monotonically increasing integer before frames are sent; default path is host-wide `ProgramData/QuantOS/mt5-sequence.json`, which must be uniquely scoped and protected for the target stream.
- `tls_context` pins TLS 1.3, hostname validation, trusted server CA, and worker client cert/key.
- `run_worker` retries connection/provider failures with bounded exponential delay and calls provider shutdown in `finally`.
- There is no order submission, modification, cancellation, or close call in this worker. `symbol_select(..., True)` can modify Market Watch selection and should be acknowledged as a non-trading terminal side effect.

The alternate `adapters/mt5-python/mt5_adapter/macos_socket_provider.py` and `mql5/QuantOSReadOnlyProvider.mq5` implement a loopback EA/provider route for macOS conformance. README explicitly says this is experimental and real terminal conformance is pending. It is not the primary Windows provider and must not be run simultaneously for the same account stream.

### Rust bridge / protocol

- `crates/intent-bridge/src/mt5_v1.rs`: `MT5_BRIDGE_SCHEMA_VERSION = 2`, `BridgeFrame`, `NdjsonFrameDecoder`, `parse_frame_json`, `BridgeFrame::validate_at`, market-data status/wire types, reconciliation types. Frames are JSONL, max 64 KiB, reject unknown fields, enforce UTC/timestamp, exact decimal, identity, and quote-state constraints.
- `crates/intent-bridge/src/mt5_transport.rs`: `Mt5TransportConfig::from_pem_files` configures TLS 1.3 only and mandatory client certificate verification; `authenticate` also requires exact leaf certificate DER SHA-256 fingerprint allowlisting before exposing frames. `Mt5TransportPrincipal` pins terminal ID, server, and account allowlist.
- `crates/intent-bridge/src/mt5_session.rs`: `Mt5SessionGate` checks account/terminal/server against immutable session identity, durable sequence observations, heartbeat state and reconciliation readiness.
- `crates/intent-bridge/src/mt5_runtime.rs` and `mt5_reconciliation.rs`: authenticated frames are checked; full snapshots are persisted before readiness confirmation; reconnect requires a new authenticated stream and reconciliation. Stale market data degrades readiness independently of transport connection.
- `crates/intent-bridge/src/bin/mt5-bridge-daemon.rs`: loopback bind by default, a single configured principal from environment, SQL persistence connection, accept/reconcile loop. It does not query `trading_account_broker_bindings` or require a `VERIFIED` external broker binding in the inspected startup path. It logs account/server/terminal identifiers and safe errors; log policy still needs deployment review.

The worker and daemon exchange schema v2 and must be upgraded together. mTLS proves possession of the provisioned worker key and configured workload identity; it does not prove that the connected terminal login belongs to the organization.

### Persistence and registry

- `migrations/20261027000002_mt5_bridge_idempotency.sql`: durable `(account_id,terminal_id)` sequence/readiness stream table plus order-key scaffolding.
- `migrations/20261027000003_mt5_reconciliation_snapshots.sql`: append-only JSONB reconciliation snapshots.
- `migrations/20261027000004_mt5_market_data_snapshots.sql`: latest JSONB market snapshot per account/terminal, sequence, server, source, currency, and capture time.
- `crates/persistence/src/repos/mt5_bridge.rs`: validates snapshot identity and monotonic sequence; persists reconciliation and stream state transactionally.
- `crates/persistence/src/repos/mt5_market_data.rs`: reads exact account/terminal market snapshot and checks embedded identity consistency; freshness remains a caller responsibility.
- `migrations/20261112000001_organization_tenant_foundation.sql`: org memberships, account entitlement, environment verification provenance.
- `migrations/20261113000001_broker_account_registry.sql`: broker/server catalog, HMAC-only external account identity tokens, evidence, verification and binding/event history. Active entitlement is required for eligible bindings; a `VERIFIED` decision requires evidence and explicit approval authority/reference.

Registry is not yet connected to daemon principal admission. Quant account `environment` is constrained to `PAPER`, `SHADOW`, `LIVE`, while broker server environment supports `DEMO`, `LIVE`, `UNKNOWN`; the DEMO mapping is unresolved. Existing bridge snapshot rows are not foreign-keyed to the organization/account binding tables.

## 5. Contract decisions and gaps

The versioned contract document freezes the existing v2 subset, preserves decimal strings and UTC source/collection distinctions, scopes sequence by account/terminal, separates heartbeat from freshness/reconciliation, and marks every absent field as not carried. Material v2 gaps against the requested field inventory include observed broker login/environment/account type; margin level/leverage/trade permissions; symbol description; tick last/volume; position stop/take-profit/current price/swap/open time; order initial and remaining volume separately, trigger price, stop/take-profit, and setup time; durable last-failure/error/restart counter; and independent reconciliation revision.

`deny_unknown_fields` and schema v2 validation mean these cannot safely be added as undocumented ad hoc fields. 01A needs an explicit schema evolution decision and compatible provider/daemon/persistence changes before promising them. Current field absence is not a license to infer values.

Security and binding prerequisites:

1. Approve DEMO-to-Quant environment semantics; do not map DEMO to PAPER/SHADOW by assumption.
2. Require active organization entitlement, HMAC-matched external login, broker/server, approved evidence, verification decision/revision, terminal assignment, and worker certificate principal to agree before ingestion eligibility.
3. Add/enforce the registry binding check in the bridge admission path in a separately authorized phase; a terminal heartbeat or mTLS identity alone is not broker ownership proof.
4. Approve Windows secret delivery and certificate lifecycle. Current `MT5_PASSWORD` environment requirement is not an accepted long-lived secret mechanism, and repository docs leave certificate issuance/key lifecycle external. This remains a blocker.
5. Decide whether symbol selection's Market Watch side effect is acceptable and scope the worker sequence file per account/terminal.

## 6. Windows acceptance plan

`docs/MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md` contains 21 explicit tests with preconditions, actions, expected results, evidence, and pass criteria. It covers host/terminal/Python identity; DEMO login/server verification; account, symbol, tick, order and position snapshots; TLS/mTLS positive and negative cases; account-binding rejection; malformed protocol; sequence/duplicate behavior; persistence and reconciliation; worker restart and terminal disconnect recovery; stale data; log redaction; and disabled execution. No Windows test was performed in this phase.

## 7. Completion gate and recommendation

**BLOCKED for MT5-MVP-01A.** Documentation work is complete, but acceptance requires:

- Current QOS-020 guard and identity verification to pass on its exact run (do not recreate or substitute resources).
- Explicit decision for DEMO environment mapping and contract fields/schema evolution.
- An approved Windows secret-delivery and certificate issuance/rotation/revocation path.
- Enforced verified-account binding before daemon ingestion.
- Versioned, pinned compatible artifacts and successful 21-test Windows DEMO acceptance on the isolated lane.

Execution remains disabled. This phase did not implement 01A or modify source, migrations, runtime behavior, schemas, Docker resources, databases, services, credentials, or deployment configuration.

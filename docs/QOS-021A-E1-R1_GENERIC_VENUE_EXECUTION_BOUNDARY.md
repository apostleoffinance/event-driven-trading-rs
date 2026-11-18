# QOS-021A-E1-R1 — Generic Venue Execution Boundary

## Executive result

**PARTIALLY COMPLETE.** The generic venue API no longer exposes order submission or cancellation. OMS wrappers and `TradingRuntime` execution methods now accept a sealed simulation/paper capability implemented only by the in-process `SimulatedVenue` and `PropVenue`. External application crates cannot implement that capability because its sealing trait is `pub(crate)`. Broker execution remains unavailable, and the MT5 path remains deny-all.

The final full workspace suite and Clippy passed with database-capable tests run through the verified QOS-020 guard. One earlier targeted command included SQLx-backed `trading-runtime` tests before the guard was run. The SQLx harness immediately failed because `DATABASE_URL` was absent; it did not establish a connection or enter the test bodies. After identifying this invocation error, QOS-020 was verified and all subsequent database-capable tests were run through the guard. Because the phase explicitly requires every DB-backed test command to be guarded, this process deviation prevents a strict **COMPLETE** result even though the final guarded validation passed.

## Preflight repository identity and worktree

- Root: `/Users/olaoluwatunmise/event-driven-trading-rs`
- Branch: `main`
- HEAD: `f6446dd33dc74a86ede9064f35aef2ec082a1c59`
- The baseline matched the authorized QOS-021A-E1-R1 identity.
- Staged pre-existing changes: `Cargo.lock`, `docker-compose.yml`.
- Unstaged and untracked pre-existing work included CI changes, QOS-021A-E1 changes, QOS-020 files, QOS-021A-S and E1 reports, and both migration backup files.
- No ignored files were reported in the relevant venue, runtime, event-trading, execution-engine, MT5, or domain source directories.
- All protected hashes listed below match the previous QOS-021A-E1 report.

No pre-existing change was reset, cleaned, stashed, checked out, overwritten, staged, or discarded.

## Execution capability architecture

### Before R1

`VenueAdapter` combined account/venue information with `submit_order` and `cancel_order`. `submit_approved_order` accepted any `VenueAdapter`, so an implementation capable of broker I/O could be supplied through the same interface used by simulation. `TradingRuntime` execution methods also accepted `&dyn VenueAdapter`.

### After R1

- `VenueAdapter` exposes venue name, account state, observed open orders, positions, and order lookup. It has no submit or cancel operation.
- `SimulationExecutionAdapter` exposes explicitly simulated submit/cancel operations. It extends a crate-private sealed trait. Only `SimulatedVenue` and paper-simulated `PropVenue` implement it in production code.
- `execute_simulated_order` and `cancel_simulated_order` require `&dyn SimulationExecutionAdapter`.
- `TradingRuntime` execution entry points and multi-account execution targets require the same simulation capability. Read-only reconciliation continues to accept `VenueAdapter`.
- The Prop connector is documented and implemented as a deterministic paper simulator. Its live-mode behavior remains `NotImplemented`/fail-closed.
- No public broker execution capability or concrete broker adapter exists. The future coordinator must own any future private broker capability after completing the QOS-021A-E2 provenance and durable authorization work.

## Caller and capability matrix

| Capability | Existing callers | Final accessibility |
|---|---|---|
| Simulated order submission | `TradingRuntime`, Python intent-ingest session, OMS simulation wrapper, existing simulation tests | Preserved through sealed `SimulationExecutionAdapter` |
| Paper execution | `SimulatedVenue`; Prop paper fill model | Preserved and explicitly typed as simulation |
| Broker order submission | No qualified production adapter exists | No public capability; future coordinator-owned private capability only |
| Read-only venue/account information | Runtime reconciliation and venue observation | Public `VenueAdapter` |
| MT5 raw transport write | MT5 bridge internals | `pub(crate)` only |
| MT5 dispatch | MT5 runtime | `dispatch_submit` still returns an error before claim or write; permit module is private |

The legacy `crates/event-trading` `ExecutionEngine::submit_order` is an in-memory paper/demo engine that creates local simulated orders and fills. It is not a venue transport or broker adapter. The modern `execution-engine::ExecutionEngine` remains the OMS and does not itself expose a broker transport.

## Exact files changed for R1

Modified:

- `crates/venue-connectors/src/adapter.rs` — removed execution methods from `VenueAdapter`; added sealed `SimulationExecutionAdapter`; added compile-fail API examples.
- `crates/venue-connectors/src/bridge.rs` — renamed and constrained wrappers to simulation capability.
- `crates/venue-connectors/src/lib.rs` — exports only the read-only and sealed simulation interfaces and simulation wrappers; registers moved unit tests.
- `crates/venue-connectors/src/simulated.rs` — split paper submit/cancel into the simulation implementation.
- `crates/venue-connectors/src/prop.rs` — split paper submit/cancel into the simulation implementation and clarify that Prop is a simulator.
- `crates/trading-runtime/src/pipeline.rs`, `runtime.rs`, `multi_account.rs` — constrain execution entry points and targets to the simulation capability; keep reconciliation read-only.
- `crates/reconciliation/tests/venue_vs_internal.rs`, `crates/venue-connectors/tests/e2e_pipeline.rs`, `crates/venue-connectors/tests/prop_pipeline.rs` — update supported simulation calls.
- `crates/execution-engine/src/oms.rs`, `crates/event-trading/src/execution/fill.rs` — update API descriptions to distinguish read-only venue access from simulation execution.

The existing `ambiguous_unknown`, `cancel_confirmation`, and `failure_injection` tests were moved from integration-test files into `crates/venue-connectors/src/tests/` so their fake simulator implementations can exercise the crate-private sealed capability. Their scenarios and assertions were retained; no test was removed or weakened.

QOS-021A-E1 files already dirty at R1 preflight (`crates/domain/*`, `crates/intent-bridge/*`, and their existing report) were not changed by R1. QOS-020/CI/Compose work and migration backups were also preserved.

## API visibility and bypass analysis

- **PASS:** There is no `submit_order` or `cancel_order` method on `VenueAdapter`.
- **PASS:** The `SimulationExecutionAdapter` sealing trait is `pub(crate)`; downstream crates cannot implement the capability.
- **PASS:** OMS wrappers are typed to the sealed simulation capability. Compile-fail doctests prove that a general `VenueAdapter` cannot call submission directly or be passed to `execute_simulated_order`.
- **PASS:** `TradingRuntime::execute_intent`, snapshot/sizing variants, event-loop execution paths, and multi-account targets all require the simulation capability.
- **PASS:** No broker adapter or broker transport capability was added.
- **PASS:** `AuthenticatedMt5Connection::write_frame` remains `pub(crate)`; `mt5_dispatch` remains a private module with no public permit re-exports.
- **PASS:** MT5 `dispatch_submit` remains callable only as a deny-all attempt: it errors before touching the order-claim store or writing to the connection.

The public `SimulationExecutionAdapter` trait’s private supertrait prevents downstream implementations. The only production implementors are the two in-process simulators. This blocks an application-provided broker adapter from entering through the simulation wrapper.

## Simulation and paper regression evidence

- Sealed simulation implementations retain the same deterministic fills, idempotency, OMS transitions, position accounting, and fees.
- Prop paper execution remains a one-fill simulator; Prop live mode remains gated.
- Existing OMS ambiguous-response, failure-injection, cancellation-confirmation, risk, and runtime scenarios passed.
- The previously integration-scoped fake-adapter tests now run as connector unit tests to permit private sealing without exposing a test or broker capability to downstream crates.

## MT5 deny-all evidence

The prior E1 changes remain intact. The mTLS integration test `dispatch_fails_closed_without_sending_or_claiming_order` passed. It verifies the denied dispatch returns an error, the peer receives no application frame, and the account/client-order key has zero rows. Other mTLS, terminal/server identity, freshness, heartbeat, reconciliation recovery, and session-gating tests passed. No MT5 frame was sent to a terminal or broker.

## QOS-020 identity and test evidence

Read-only guard verification confirmed:

- Run: `20261110-230504-ffb59dfd0ab4`
- Project: `qos020-integration-20261110-230504-ffb59dfd0ab4`
- Container ID: `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82`
- Network: `qos020-integration-net-20261110-230504-ffb59dfd0ab4`
- Volume: `qos020_it_20261110_230504_ffb59dfd0ab4`
- Image: `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777`
- Endpoint: `127.0.0.1:51462`, database `qos020_test_master`, SQLx role `qos020_sqlx`

The guard verified container, project, network, volume, image, endpoint, and run identity. `scripts/qos020/test_guard.py` reported **19 passed**, including `test_protected_port_15543_rejected`.

Final validation command:

```text
cargo fmt --all -- --check
python3 scripts/qos020/qos020_guard.py --manifest '/private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json' --secrets-dir '/private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets' --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 -- bash -euc 'cargo test --workspace -- --test-threads=1 && cargo clippy --workspace --all-targets -- -D warnings'
```

Results: formatting passed; the complete Rust workspace suite passed, including SQLx tests under QOS-020; all three public-boundary compile-fail doctests passed; MT5 mTLS reported **7 passed**; venue connector tests reported **11 unit tests, 2 e2e tests, and 2 Prop tests passed**; workspace Clippy passed with `-D warnings`.

### Test-procedure deviation

An earlier targeted command was run as `cargo test -p venue-connectors -p trading-runtime -p reconciliation` before running the QOS-020 guard. It unexpectedly selected the SQLx-backed `trading-runtime` integration tests. The SQLx test harness stopped immediately with `DATABASE_URL must be set`; it did not connect or enter those test bodies. I then verified QOS-020 and its protected-port rejection suite, and all later DB-capable test runs were guarded. This is the reason the result is **PARTIALLY COMPLETE**, despite the final guarded test run passing.

## Protected file hashes

The following hashes were captured at R1 preflight and again after implementation; each is unchanged:

| Protected file | Before SHA-256 | After SHA-256 |
|---|---|---|
| `Cargo.lock` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` |
| `docker-compose.yml` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` |
| `.github/workflows/ci.yml` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` |
| `crates/intent-bridge/src/mt5_v1.rs` | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` |
| `docs/QOS-020_TEST_INFRASTRUCTURE.md` | `a439e0ca37f4d863a381c523bf3b3ba3548a2eab0d678b0e947a3b946a167bbb` | `a439e0ca37f4d863a381c523bf3b3ba3548a2eab0d678b0e947a3b946a167bbb` |
| `docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md` | `7e7b0961509be76ac455e299b878d5f4dffdec8f5a40bc5f927e556bb702dada` | `7e7b0961509be76ac455e299b878d5f4dffdec8f5a40bc5f927e556bb702dada` |
| `docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md` | `de5e24235d0958d322330d9fde2f76fa193b9a9fcfa560ade0dc75b166a7f44d` | `de5e24235d0958d322330d9fde2f76fa193b9a9fcfa560ade0dc75b166a7f44d` |
| `scripts/traderos-migration/migrate.py.before-http-debug` | `3156c8bb1c26ea24baa9a3eaaa35426753d350ce61f3d1801950a41a36b402a` | `3156c8bb1c26ea24baa9a3eaaa35426753d350ce61f3d1801950a41a36b402a` |
| `scripts/traderos-migration/migrate.py.debug-backup` | `3156c8bb1c26ea24baa9a3eaaa35426753d350ce61f3d1801950a41a36b402a` | `3156c8bb1c26ea24baa9a3eaaa35426753d350ce61f3d1801950a41a36b402a` |

## Deferred work

- QOS-021A-E2 account environment and provenance schema/bootstrap.
- The trusted execution coordinator and durable authorization/claim lifecycle.
- Any broker adapter or private coordinator-owned broker transport. No such capability is added in R1.
- QOS-021A-E1 overall authorization readiness remains blocked until the coordinator, persistence, and authorization-bound test matrix are implemented.

## Side-effect confirmation

No migrations, database writes/restores, broker connections/orders, trading execution, Docker lifecycle operations, service restarts, deployments, commits, or pushes occurred. QOS-020 guard inspection was read-only. Staging and production remained untouched.

## Final Git status

Repository remains on `main` at `f6446dd33dc74a86ede9064f35aef2ec082a1c59`. Status after creating this report:

```text
 M .github/workflows/ci.yml
M  Cargo.lock
 M crates/domain/src/lib.rs
 M crates/event-trading/src/execution/fill.rs
 M crates/execution-engine/src/oms.rs
 M crates/intent-bridge/src/lib.rs
 M crates/intent-bridge/src/mt5_runtime.rs
 M crates/intent-bridge/src/mt5_transport.rs
 M crates/intent-bridge/src/mt5_v1.rs
 M crates/intent-bridge/tests/mt5_transport_mtls.rs
 M crates/reconciliation/tests/venue_vs_internal.rs
 M crates/trading-runtime/src/multi_account.rs
 M crates/trading-runtime/src/pipeline.rs
 M crates/trading-runtime/src/runtime.rs
 M crates/venue-connectors/src/adapter.rs
 M crates/venue-connectors/src/bridge.rs
 M crates/venue-connectors/src/lib.rs
 M crates/venue-connectors/src/prop.rs
 M crates/venue-connectors/src/simulated.rs
 D crates/venue-connectors/tests/ambiguous_unknown.rs
 D crates/venue-connectors/tests/cancel_confirmation.rs
 M crates/venue-connectors/tests/e2e_pipeline.rs
 D crates/venue-connectors/tests/failure_injection.rs
 M crates/venue-connectors/tests/prop_pipeline.rs
M  docker-compose.yml
?? crates/domain/src/execution_authorization.rs
?? crates/venue-connectors/src/tests/
?? docs/QOS-020_TEST_INFRASTRUCTURE.md
?? docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md
?? docs/QOS-021A-E1-R1_GENERIC_VENUE_EXECUTION_BOUNDARY.md
?? docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md
?? scripts/qos020/
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

No commit or push was made.

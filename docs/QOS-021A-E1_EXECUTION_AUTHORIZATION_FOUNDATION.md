# QOS-021A-E1 — Execution Authorization Foundation

## Executive summary

**Overall readiness: BLOCKED.** This phase adds a fail-closed account-environment policy type and disables the public MT5 runtime submit entry point. The raw authenticated MT5 frame writer is crate-private, and the former MT5 submit-permit module is private. A guarded integration test verifies a submit attempt is rejected, sends no application frame, and creates no durable MT5 order claim.

The full execution authorization coordinator is not implemented. The current authoritative account schema does not store verified environment provenance, and the repository has no general durable authorization/claim model. Those changes depend on QOS-021A-E2's forward-only migration/bootstrap design. No migration was added. The public generic `VenueAdapter::submit_order` and `submit_approved_order` path also remains callable; current concrete adapters are simulated or paper-only, but the type system does not yet prevent a future broker-capable implementation from bypassing a coordinator. Therefore E1's completion criteria are not met and no broker execution is authorized.

## Files created and modified

Created:

- `crates/domain/src/execution_authorization.rs`
- `docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md` (this report)

Modified for E1:

- `crates/domain/src/lib.rs` — exports the policy types and check.
- `crates/intent-bridge/src/lib.rs` — hides the MT5 dispatch module and removes its public permit exports.
- `crates/intent-bridge/src/mt5_runtime.rs` — fails closed on every runtime submit request.
- `crates/intent-bridge/src/mt5_transport.rs` — restricts raw authenticated frame writes to the crate.
- `crates/intent-bridge/tests/mt5_transport_mtls.rs` — replaces the former successful-send expectation with fail-closed/no-frame/no-claim assertions.

No CI, Compose, migration, service, or credential configuration was changed for E1. Pre-existing work is listed under [Worktree and preservation evidence](#worktree-and-preservation-evidence).

## Architecture before and after

Before E1, an application caller could call `Mt5BridgeRuntime::dispatch_submit`; the publicly exported permit API was reachable, and `AuthenticatedMt5Connection::write_frame` was public. The generic venue adapter trait also exposed `submit_order`.

After E1, `dispatch_submit` returns an error before claiming an order or writing a frame. The MT5 dispatch implementation remains internal and its public exports are removed. `write_frame` is `pub(crate)`. The new domain policy distinguishes `PAPER`, `SHADOW`, and `LIVE` from `AccountType`, models unclassified and unverified legacy provenance, and only accepts a positive, non-future verified revision for `LIVE` as one necessary condition. The policy does not issue a grant and does not authorize execution on its own.

The generic venue submission API remains unchanged. No real broker adapter exists in the audited source; `SimulatedVenue` and paper-mode `PropVenue` are current implementations, and Prop live submission remains unimplemented. Still, ordinary application code can invoke the generic submit API. This is a known E1 blocker, not an assumed-safe boundary.

## Coordinator and authorization lifecycle

**Status: BLOCKED / DEFERRED.** There is no authoritative coordinator yet. No server-generated grant is minted, and no context is bound across tenant/owner, canonical account, broker/server, verified environment revision, exact order, risk decision, issue/expiry, authorization ID, and replay nonce. Callers cannot obtain an E1 authorization grant because the grant type and issuance lifecycle have not been introduced.

`require_verified_live_environment` validates only the environment subcondition. Its return value is the verified revision, not an execution permit. It rejects missing/unclassified environment, unverified legacy provenance, zero revision, a future verification timestamp, and non-LIVE environments.

The MT5 runtime currently denies all submit attempts. Read-only market-data, heartbeat, and reconciliation behavior is not changed by this gate.

## Authorization and idempotency data model

The E1 domain policy introduces:

- `AccountEnvironment::{Paper, Shadow, Live}`.
- `EnvironmentVerification::{Unclassified, UnverifiedLegacy, Verified { revision, verified_at }}`.
- `BrokerAuthorizationError` with explicit fail-closed error cases.

No database schema or durable execution-authorization table was introduced. The existing `mt5_bridge_order_keys` table is bridge-specific and cannot establish verified environment provenance, tenant ownership, risk-decision identity, or a general execution authorization. No claim is made that this table provides general exactly-once execution.

## Database migration decision

No migration was created or applied. The account environment/provenance and durable grant/claim schema are deferred to QOS-021A-E2, which must resolve migration ordering and bootstrap compatibility first. Existing historical migration files were not edited. The required safe persistence boundary is therefore unavailable, and durable authorization issuance, single-use consumption, concurrency handling, UNKNOWN transition, and replay rejection remain blocked.

## Test coverage and results

Added/updated coverage:

- Domain policy rejects unclassified, unverified legacy, PAPER, and SHADOW states for broker-LIVE eligibility.
- Domain policy rejects zero revisions and future verification timestamps; accepts a positive current verified LIVE revision.
- MT5 transport integration test confirms dispatch returns an error, the peer receives no application frame, and the exact account/client-order key has zero durable claims.
- Existing MTLS, identity, freshness, reconciliation, and session-gating tests remain in the suite.

QOS-020 identity was checked before all DB-backed Rust tests. The validated run was `20261110-230504-ffb59dfd0ab4`, with project `qos020-integration-20261110-230504-ffb59dfd0ab4`, loopback endpoint `127.0.0.1:51462`, database `qos020_test_master`, and role `qos020_sqlx`. The guard verified the registered container/project/network/volume/image identity. Guard unit tests reported **19 passed**, including `test_protected_port_15543_rejected`.

Final verification command (database-capable test commands were invoked only as the child of the QOS-020 guard):

```text
python3 scripts/qos020/qos020_guard.py --manifest '<QOS-020 run manifest>' --secrets-dir '<QOS-020 run secrets directory>' --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 -- bash -euc 'cargo fmt --all -- --check && cargo test --workspace -- --test-threads=1 && cargo clippy --workspace --all-targets -- -D warnings'
```

Results: formatting passed; the full Rust workspace test suite passed, including SQLx-backed tests on the disposable QOS-020 database; workspace Clippy passed with `-D warnings`. The MT5 integration test binary reported **7 passed**. No GitHub Actions workflow run was performed.

## QOS-020 and protected environment evidence

- **PASS:** QOS-020 guard validated the exact recorded integration-test run and endpoint before database tests.
- **PASS:** QOS-020 negative guard suite rejected protected port `15543` without connecting to it.
- **PASS:** Rust database tests ran under the guard against `qos020_test_master` at loopback port `51462`.
- **PASS:** No staging or production database connection, migration, write, restore, or configuration change was performed for this task.
- **PASS:** No container lifecycle action, service restart, broker connection, or trading submission was performed.

## Worktree and preservation evidence

Repository: `/Users/olaoluwatunmise/event-driven-trading-rs`  
Branch: `main`  
HEAD: `f6446dd33dc74a86ede9064f35aef2ec082a1c59`

Protected pre-existing file hashes before E1 and at report generation:

| File | Before SHA-256 | After SHA-256 | Result |
|---|---|---|---|
| `Cargo.lock` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` | Preserved |
| `docker-compose.yml` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` | Preserved |
| `.github/workflows/ci.yml` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` | Preserved |
| `crates/intent-bridge/src/mt5_v1.rs` | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` | Preserved |
| `docs/QOS-020_TEST_INFRASTRUCTURE.md` | `a439e0ca37f4d863a381c523bf3b3ba3548a2eab0d678b0e947a3b946a167bbb` | `a439e0ca37f4d863a381c523bf3b3ba3548a2eab0d678b0e947a3b946a167bbb` | Preserved |
| `docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md` | `7e7b0961509be76ac455e299b878d5f4dffdec8f5a40bc5f927e556bb702dada` | `7e7b0961509be76ac455e299b878d5f4dffdec8f5a40bc5f927e556bb702dada` | Preserved |
| `scripts/traderos-migration/migrate.py.before-http-debug` | `3156c8bb1c26ea24baa9aa3eaaa35426753d350ce61f3d1801950a41a36b402a` | `3156c8bb1c26ea24baa9aa3eaaa35426753d350ce61f3d1801950a41a36b402a` | Preserved |
| `scripts/traderos-migration/migrate.py.debug-backup` | `3156c8bb1c26ea24baa9aa3eaaa35426753d350ce61f3d1801950a41a36b402a` | `3156c8bb1c26ea24baa9aa3eaaa35426753d350ce61f3d1801950a41a36b402a` | Preserved |

Complete status at report generation:

```text
 M .github/workflows/ci.yml
M  Cargo.lock
 M crates/domain/Cargo.toml
 M crates/domain/src/lib.rs
 M crates/intent-bridge/src/lib.rs
 M crates/intent-bridge/src/mt5_runtime.rs
 M crates/intent-bridge/src/mt5_transport.rs
 M crates/intent-bridge/src/mt5_v1.rs
 M crates/intent-bridge/tests/mt5_transport_mtls.rs
M  docker-compose.yml
?? crates/domain/src/execution_authorization.rs
?? docs/QOS-020_TEST_INFRASTRUCTURE.md
?? docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md
?? docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md
?? scripts/qos020/
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

The `Cargo.lock`, Compose, CI, `mt5_v1.rs`, QOS-020 docs/scripts, and migration backups above are pre-existing work. Their original contents are preserved. No staging, reset, clean, stash, checkout, commit, push, or deployment operation was performed.

## Outstanding blockers and E2 prerequisites

1. Add the forward-only account environment/provenance schema and bootstrap behavior in QOS-021A-E2; legacy rows must remain explicitly unverified.
2. Add durable authorization and account/client-order claim persistence with uniqueness, expiry, single-use consumption, concurrency control, and UNKNOWN/reconciliation semantics.
3. Design one coordinator that resolves tenant/account/broker/server from trusted persisted state and binds verified environment revision, exact order, risk decision, authorization ID, expiry, and nonce.
4. Remove broker-capable `submit_order` from the general public `VenueAdapter` surface or introduce an API/type split that makes simulation callable while making any future broker transport coordinator-only. Update affected call sites and prove simulation remains functional.
5. Integrate MT5 only through the coordinator-owned private transport capability; preserve the current deny-all submit behavior until end-to-end authorization tests pass.
6. Add fake-transport tests for grant binding, expiry/replay, tenant/account/broker/order/risk mismatch, concurrency, UNKNOWN, and unauthorized-request non-delivery.

## Completion status

| Criterion | Status | Evidence |
|---|---|---|
| Authoritative coordinator exists | **BLOCKED** | Trusted environment and durable authorization persistence are deferred to E2. |
| Ordinary code cannot bypass via broker adapters or raw MT5 transport | **FAIL** | MT5 raw submit path is inaccessible/denied; generic public venue submit API remains. |
| Authorization binds trusted account, broker, order, and risk context | **BLOCKED** | No coordinator or persisted trusted context/grant exists. |
| Single-use and replay protections | **BLOCKED** | No general authorization grant/claim schema exists. |
| Durable claims and ambiguous outcome handling | **DEFERRED** | Existing MT5 key table is not a general authorization model; E2 schema dependency documented. |
| PAPER/SHADOW cannot reach MT5 broker transport | **PASS for MT5 dispatch** | Runtime dispatch denies all requests; domain policy rejects both environments for LIVE eligibility. Generic adapter split is still outstanding. |
| Unverified accounts fail closed | **PASS for policy function; BLOCKED end-to-end** | Explicit unverified state is rejected, but current account persistence does not provide it. |
| Fake transport and bypass tests cover all required scenarios | **FAIL** | Added MT5 no-send/no-claim test; full grant binding and concurrency matrix awaits coordinator/schema. |
| Relevant Rust checks/tests pass | **PASS** | Workspace formatting, tests, and Clippy passed through QOS-020. |
| QOS-020 remains isolated and protected environments untouched | **PASS** | Guard identity and rejection tests passed; no staging/prod operation occurred. |
| Pre-existing worktree changes preserved | **PASS** | Protected hashes and complete status recorded above. |

**QOS-021A-E1 is not complete. Overall status: BLOCKED pending QOS-021A-E2 and the public venue API boundary work.**

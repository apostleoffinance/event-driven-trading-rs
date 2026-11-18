# QOS-021A-E2 — Account Environment, Migration Integrity and Durable Authorization Schema

**Result: COMPLETE for the authorized E2 foundation.** This delivers persistent environment provenance, guarded SQLx migration bootstrap, forward-only Journal FK repair, and additive authorization/claim tables. It does not add an authorization coordinator, an environment-verification HTTP endpoint, or any broker execution path. MT5 dispatch and broker execution remain disabled.

## 1. Repository identity and preservation preflight

- Repository: `/Users/olaoluwatunmise/event-driven-trading-rs`
- Branch: `main`
- HEAD before and after: `f6446dd33dc74a86ede9064f35aef2ec082a1c59`
- Pre-existing worktree was dirty and included staged `Cargo.lock` and `docker-compose.yml`, unstaged QOS-020 CI changes, E1/R1 execution-boundary changes, modified Rust runtime/connector files, deleted venue connector tests, untracked QOS reports and QOS-020 scripts, and both untracked TraderOS migration backups.
- The ignored `scripts/qos020/__pycache__/` directory was present before work and remains present.
- No pre-existing file was reset, cleaned, stashed, checked out, staged, committed, or deleted by E2. The pre-existing staged files remain staged. Existing deleted venue test files remain deleted as pre-existing work.

Initial pre-existing status paths, grouped only for readability:

```text
M  Cargo.lock                         M  docker-compose.yml
 M .github/workflows/ci.yml
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
?? crates/domain/src/execution_authorization.rs
?? crates/venue-connectors/src/tests/
?? docs/QOS-020_TEST_INFRASTRUCTURE.md
?? docs/QOS-021A-E1-R1_GENERIC_VENUE_EXECUTION_BOUNDARY.md
?? docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md
?? docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md
?? scripts/qos020/
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

The QOS-020 guard tests confirmed 19/19 cases pass, including rejection of protected PostgreSQL port `15543`. The expected isolated test instance identity was verified before database tests:

- Run: `20261110-230504-ffb59dfd0ab4`
- Container: `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82`
- Project: `qos020-integration-20261110-230504-ffb59dfd0ab4`
- Loopback endpoint: `127.0.0.1:51462`, database `qos020_test_master`, role `qos020_sqlx`
- The guard loaded the secret file internally; no credential value was printed or copied into this report.

## 2. Account environment and provenance schema

`accounts.environment` remains independent of `account_type`. The forward migration removes the historical `PAPER` default and makes the column nullable so an unclassified legacy row can be represented. It introduces:

- `environment_verification`: `UNCLASSIFIED`, `UNVERIFIED`, `UNVERIFIED_LEGACY`, or `VERIFIED`.
- `environment_revision BIGINT`.
- `environment_verified_at TIMESTAMPTZ`.
- `environment_verified_by TEXT`, referencing `identity_users(id)` with `ON DELETE RESTRICT`.
- Optional `environment_verification_reason`.

Checks reject unknown environments/states, negative revisions, inconsistent nullability, and verified records lacking a positive revision, timestamp, and attributable existing identity. Existing rows retain their exact `PAPER`, `SHADOW`, or `LIVE` value and become `UNVERIFIED_LEGACY` at revision zero. A null legacy environment becomes `UNCLASSIFIED`.

A database insert trigger rejects a new account that omits its environment or attempts to insert it as `UNCLASSIFIED`. Explicit new classifications start `UNVERIFIED` at revision one; the database supplies no environment default. An update trigger requires the revision to increase when environment or verification eligibility/provenance fields change.

The domain `Account` now holds optional environment, revision, verification state, verifier, and reason. Its constructor requires an explicit `AccountEnvironment`; simulated account helpers explicitly pass `PAPER`. The domain eligibility check requires `LIVE`, `VERIFIED`, a positive revision equal to the current account revision, a non-future verification time, and a non-empty verifier identity. It remains necessary but insufficient for execution authorization.

Persistence inserts environment explicitly and does not overwrite environment/provenance during ordinary account upsert. It reads and validates persisted values. `verify_account_environment` runs in a transaction, locks the account row, checks the caller's expected revision, requires a non-empty verifier identity, and increments the revision. Concurrent stale updates serialize and one fails its expected-revision check.

The dashboard account response returns stored environment, verification state, revision, timestamp, and verifier. Null environment is returned as null with `UNCLASSIFIED`; legacy `PAPER` remains visible with `UNVERIFIED_LEGACY`. The paper-account request requires an explicit environment and accepts only `PAPER` for that paper-specific route. It no longer fabricates a classification in the response.

No HTTP operation that verifies an environment was added. The persistence operation is explicit and attributable; exposing it to callers requires a separately reviewed authorization policy so this schema work does not create a permissive verification route.

## 3. Forward-only migrations and checksums

No historical migration was edited, renamed, deleted, or renumbered. SHA-384 below is SQLx 0.8.6's checksum of the exact SQL file contents (`Migration::new` hashes `sql.as_bytes()`). The six core migration hashes were compared to SQLx's loaded migration checksums in the fresh bootstrap integration test.

| Version / file | SHA-384 |
|---|---|
| `20260306000001_init.sql` | `2039385e4e356cc2913ac4e46ee22887d71ba33a2ceaa8e4d246d1f167093d6aaffa9dfa5dfb35f1cb8f091dbd9d6c8e` |
| `20260906000001_instrument_spec.sql` | `db022e951514e566a2126f7bffe805052a1cd626a9c006c0b1d3c48f857f2a21eee05fc4a02a80bd4f6271ee9f181b79` |
| `20261027000001_sizing_decision_audits.sql` | `2c0275b9824e99ce0f14b278ba8d785f90a30258ba1504e4a90befee2c66a3951ac7ce1c2f0c79c145fac5aa006e4db3` |
| `20261027000002_mt5_bridge_idempotency.sql` | `a9aa9ba41bfe99913e85c3051b554b41e6fe83d9a61140ebf997627ec985e05ab5e6bb8f90d13dd39c8b6ce6f80078bd` |
| `20261027000003_mt5_reconciliation_snapshots.sql` | `5e49ddaac69e47d0285926b482e6497ab4834e02973974e35b76ead52ad969bdfaf702b8b6df8fc7085fbdd38c800df4` |
| `20261027000004_mt5_market_data_snapshots.sql` | `06a989e74a1e0a04955bcac2f304e7d8da7b98e774a7025d48e00c0eadc2ed02551815a2a3839ec06c51822d3b9190f1` |
| `20261111000001_journal_trading_account_fk_repair.sql` | `1efd357c0e24fc993652d5f5d84832495708f730a1cc4b741afd313b064131250cd3814a8b7b43666f2c23d6c15ed1b8` |
| `20261111000002_account_environment_provenance.sql` | `cd091bc417cfa05ce800ab589d5702cb50fb7cfb60562b595786f3cae95b0f1a0fa6594955e773c7b725121d61ba8b11` |
| `20261111000003_execution_authorizations_and_claims.sql` | `a54a7fb254c14cb1135c648e7df22570a0bf4cc51d2d6dfee523789db9966b4e3a5ac5aa7daf5007fde1f050db168247` |

### SQLx bootstrap and history classification

`persistence::migrate` reads SQLx history and schema before applying anything. It serializes that read/classify/apply operation with a dedicated PostgreSQL session advisory lock, preventing concurrent service startup from racing between the core subset and the full migration set. It also uses SQLx's normal migration lock for each migrator run.

On a genuinely fresh database (no history and no application tables), it selects the six original core `Migration` objects from the full SQLx migrator, retaining their original versions, SQL, and checksums; runs that subset; then runs the ordinary full migrator. SQLx 0.8.6 validates already-applied checksums and accepts those versions in the full migrator, then applies Journal migrations. The implementation does not duplicate SQL in a second bootstrap file.

Existing accepted states are exactly: the six known core versions/checksums; the complete known pre-E2 migration set (which includes Journal-first history); or the complete current migration set. Failed/dirty history, unknown versions, partial sets, checksum mismatches, an empty history over existing application tables, and expected histories missing required schema objects fail closed. A recognized complete E2 history is also checked for the account provenance columns, both durable tables, and a validated Journal FK.

The SQLx 0.8.6 `Migrator.migrations` and `Migrator::DEFAULT` fields are public but documented by SQLx as hidden/semver-exempt. Cargo.lock currently fixes SQLx 0.8.6; any SQLx upgrade must revalidate the filtered-migrator bootstrap and these tests.

### Journal account FK repair

`journal_accounts.linked_trading_account_id` and `accounts.id` are verified as nullable/text and text respectively. The migration examines `pg_constraint` by source/target column, referenced relation, match mode, and update/delete actions. A correct constraint under any name is validated and retained without duplication. A named or column-level conflicting FK fails without drop/replace. With no FK it detects orphaned non-null links, fails with the orphan count, otherwise adds the intended FK `NOT VALID`, validates it, and verifies the resulting catalog entry. Required actions are `ON UPDATE NO ACTION` and `ON DELETE NO ACTION`.

## 4. Durable authorization and claim schema

The additive schema references existing `identity_users`, `dashboard_account_access`, `accounts`, `orders`, and `risk_decisions` records. It adds binding constraints for owner/account, order/account/client-order, and risk-decision/account so the authorization cannot point across unrelated identities. It stores broker/server identity as non-empty strings because the current repository has no authoritative broker-server registry to reference.

`execution_authorizations` stores owner, account, broker/server, environment/revision, order ID, canonical SHA-256 order-payload hash, client-order ID, immutable risk-decision ID, unique nonce, state, issue/expiry/consumption times, and audit timestamps. Checks enforce hash format, positive environment revision, valid state, expiry after issue, and consumed timestamp/state consistency.

`execution_claims` enforces unique `(account_id, client_order_id)`, authorization/order identity linkage, unique submission attempt ID, lifecycle states including `ACKNOWLEDGED`, `REJECTED`, `UNKNOWN`, and `RECONCILIATION_REQUIRED`, and durable venue correlation/outcome fields. Unknown/reconciliation-required outcomes cannot be represented as successful terminal results. Duplicate claims fail at the unique constraint.

These are schema records only. E2 does not issue or consume authorizations, create execution claims in application code, transition claims after real submissions, guarantee exactly-once broker execution, or provide a broker method. External ambiguous outcomes remain explicit and require later reconciliation.

## 5. Migration ownership and deployment compatibility

Current startup migration callers remain unchanged:

- `crates/identity-api/src/main.rs` calls `persistence::migrate`.
- `crates/journal-api/src/main.rs` calls `persistence::migrate`.
- `crates/persistence/src/bin/persistence-migrate.rs` calls `migrate`.

All therefore receive the new fail-closed bootstrap via the shared persistence function, but this phase does not change migration-on-startup ownership. The intended later design remains one dedicated migration job per deployment. Until that transition is separately implemented, concurrent startup is serialized by the new session lock. During rolling deploys, an older binary whose embedded migrations do not know newer applied versions may reject that database; deployment order and old/new binary compatibility must be rehearsed before production rollout.

## 6. Synthetic migration fixtures and results

The new `crates/persistence/tests/qos021a_e2_migrations.rs` uses SQLx test-created disposable databases, not the QOS-020 master schema. It covers:

1. Fresh empty database; six core migrations installed before Journal foundation; versions/checksums recorded; standard full migrator succeeds; repeat run is safe.
2. Exact six-core history upgrades.
3. Complete historical migration set with Journal-first ordering upgrades.
4. Concurrent fresh bootstrap calls serialize and both complete.
5. Legacy `PAPER`, `SHADOW`, and `LIVE` values are preserved as `UNVERIFIED_LEGACY`.
6. Omitted environment insert rejected; explicit new environment is `UNVERIFIED` revision 1.
7. Environment load/round-trip, explicit verifier attribution, expected-revision conflict, concurrent revision updates, and domain eligibility.
8. Empty migration table plus application schema fails as non-fresh.
9. Partial, failed/dirty, divergent checksum, and recognized-history/missing-schema states fail closed.
10. Correct FK, equivalent correct custom-named FK, orphan, conflicting FK, FK enforcement, and repeated repair SQL.
11. Authorization/claim identity links, expiry checks, unique client-order claim, and `UNKNOWN` outcome representation.
12. Dashboard serialization preserves unclassified and legacy states; account API deserialization requires an explicit environment.

Focused final migration suite: **16 passed, 0 failed**. Full final workspace run: **347 passed, 0 failed, 0 ignored**, process exit code 0. The QOS-020 cleanup verification found no `_sqlx_test_*` databases remaining.

## 7. Validation command ledger

All database-capable commands below were children of `scripts/qos020/qos020_guard.py` with the exact run ID above. The guard verified Docker container, image, volume, network, host mapping, database marker, role privileges, and URL identity before starting the child. Its negative test suite includes rejection of port 15543.

| Command | Outcome |
|---|---|
| `python3 scripts/qos020/test_guard.py` | 19 passed, including staging port 15543 rejection. |
| `cargo fmt --all -- --check` | Passed after formatting. |
| `cargo check --workspace --all-targets` | Passed. |
| `cargo test -p journal-api` | 17 passed, including account-request and dashboard environment serialization tests. |
| Guarded `cargo test -p persistence --test qos021a_e2_migrations -- --test-threads=1` | Final focused run: 16 passed, 0 failed. |
| Guarded `cargo test --workspace --all-targets -- --test-threads=1` | 347 passed, 0 failed, 0 ignored; exit code 0. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| Guard `--verify-sqlx-cleanup` | QOS-020 identity valid; SQLx generated test databases: none remain. |
| `git diff --check` | Passed. |
| SHA-256 protected-file recheck | All values match preflight (table below). |

Exact guarded database-test invocations (the guard injects `DATABASE_URL` only after verifying the manifest and Docker/database identity):

```sh
python3 scripts/qos020/qos020_guard.py --manifest /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json --secrets-dir /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 -- cargo test -p persistence --test qos021a_e2_migrations -- --test-threads=1

python3 scripts/qos020/qos020_guard.py --manifest /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json --secrets-dir /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 -- cargo test --workspace --all-targets -- --test-threads=1

python3 scripts/qos020/qos020_guard.py --manifest /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json --secrets-dir /private/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets --lane integration-test --run-id 20261110-230504-ffb59dfd0ab4 --verify-sqlx-cleanup
```

Diagnostic iteration history: the first focused migration run found the FK migration's uninitialized PL/pgSQL record handling. Subsequent fixture runs found missing required timestamps in synthetic deployment/risk rows. Those failures were fixed; no test was weakened. The final 16-case focused suite and final 347-case workspace suite pass.

Protected file SHA-256 before and after E2:

| File | SHA-256 |
|---|---|
| `Cargo.lock` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` |
| `docker-compose.yml` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` |
| `.github/workflows/ci.yml` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` |
| `crates/intent-bridge/src/mt5_v1.rs` | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` |
| `docs/QOS-020_TEST_INFRASTRUCTURE.md` | `a439e0ca37f4d863a381c523bf3b3ba3548a2eab0d678b0e947a3b946a167bbb` |
| `docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md` | `7e7b0961509be76ac455e299b878d5f4dffdec8f5a40bc5f927e556bb702dada` |
| `docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md` | `de5e24235d0958d322330d9fde2f76fa193b9a9fcfa560ade0dc75b166a7f44d` |
| `docs/QOS-021A-E1-R1_GENERIC_VENUE_EXECUTION_BOUNDARY.md` | `ee69148e712870d791b11f3f7854cb56785dc40ba994adb1c97f8f7559a9d214` |
| `scripts/traderos-migration/migrate.py.before-http-debug` | `3156c8bb1c26ea24baa9aa3eaaa35426753d350ce61f3d1801950a41a36b402a` |
| `scripts/traderos-migration/migrate.py.debug-backup` | `3156c8bb1c26ea24baa9aa3eaaa35426753d350ce61f3d1801950a41a36b402a` |

## 8. Files created or modified by E2

Created:

- `migrations/20261111000001_journal_trading_account_fk_repair.sql`
- `migrations/20261111000002_account_environment_provenance.sql`
- `migrations/20261111000003_execution_authorizations_and_claims.sql`
- `crates/persistence/tests/qos021a_e2_migrations.rs`
- `docs/QOS-021A-E2_ACCOUNT_ENVIRONMENT_AND_MIGRATION_FOUNDATION.md`

Modified for E2:

- `crates/domain/src/account.rs`
- `crates/domain/src/execution_authorization.rs` (pre-existing untracked E1 source, extended without replacing it)
- `crates/domain/src/lib.rs` (pre-existing dirty source, retained and extended)
- `crates/domain/tests/phase1_architecture.rs`
- `crates/account-engine/src/simulated.rs`
- `crates/journal-api/src/dashboard.rs`
- `crates/persistence/src/pool.rs`
- `crates/persistence/src/repos/accounts.rs`
- `crates/persistence/src/repos/mod.rs`
- `crates/persistence/tests/survive_restart.rs`

E2 deleted no files. No historical migration or protected staged file changed.

## 9. Final status and operational boundaries

Final repository root, branch, and HEAD remain the values listed in section 1. The E2 files above are unstaged/untracked along with the preserved pre-existing work. `Cargo.lock` and `docker-compose.yml` remain staged as before. `git diff --check` is clean.

The only database resources created were SQLx-generated per-test databases inside the identity-verified QOS-020 PostgreSQL cluster; SQLx removed them after each test, and the guard confirmed none remain. The QOS-020 master identity schema was only read by the guard. No Docker container, image, volume, network, Compose resource, staging database, production database, recovery dump, broker, MT5 terminal, or deployment was modified or accessed for E2 validation. No credential values were inspected or reported.

Remaining separate work: dedicated migration-job ownership, an authenticated environment-verification workflow, authorization issuance/consumption coordination, claim state orchestration/reconciliation, and production deployment rehearsal. E2 leaves all those execution/runtime capabilities disabled.

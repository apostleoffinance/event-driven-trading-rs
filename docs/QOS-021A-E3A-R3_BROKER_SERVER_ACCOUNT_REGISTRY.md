# QOS-021A-E3A-R3 — Broker, Server and External Account Registry

**Status: PARTIAL**  
**Repository:** `/Users/olaoluwatunmise/event-driven-trading-rs`  
**Branch / HEAD:** `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59`  
**Migration:** `20261113000001_broker_account_registry.sql`  
**Migration SHA-256:** `ee76686a784a3b62d0ae6743a78549ee4c4bbe9a548820115efd59f03c24b2be`

## 1. Repository and baseline verification

**VERIFIED.** The repository root, branch, HEAD and `origin` match the authorized baseline. `origin` is `https://github.com/apostleoffinance/event-driven-trading-rs` for fetch and push. No fetch or repository-history mutation was performed.

The worktree was already substantially modified before R3 began. In particular, the staged `Cargo.lock` and `docker-compose.yml` changes, unstaged R2 implementation work, deleted legacy test files, untracked R2 files, QOS-020 infrastructure and the two migration backup files were pre-existing and preserved.

## 2. Initial and final Git status

**VERIFIED.** Initial status was captured before R3 changes. It contained the staged `Cargo.lock` and `docker-compose.yml` deltas; unstaged changes across CI, accounts, domain, execution, MT5 bridge, API, persistence, reconciliation, runtime and venue connector code; three deleted venue connector tests; and untracked R2 implementation, test, report, migration, QOS-020, fixture, and migration-backup artifacts.

Final status retains those entries and adds the R3 work listed below. `Cargo.lock` is `MM`: the pre-existing staged change remains staged, and the R3 dependency resolution is an additional unstaged change. `docker-compose.yml` remains `M ` with its original staged state. The final staged name/status output is exactly:

```text
M  Cargo.lock
M  docker-compose.yml
```

The R3 additions are:

```text
 M Cargo.lock
 M crates/domain/src/ids.rs
 M crates/domain/src/lib.rs
 M crates/journal-api/Cargo.toml
 M crates/journal-api/src/main.rs
 M crates/persistence/src/lib.rs
 M crates/persistence/src/pool.rs
 M crates/persistence/src/repos/mod.rs
?? crates/domain/src/broker_registry.rs
?? crates/journal-api/src/broker_registry.rs
?? crates/persistence/src/repos/broker_registry.rs
?? crates/persistence/tests/qos021a_e3a_r3_broker_registry.rs
?? migrations/20261113000001_broker_account_registry.sql
?? docs/QOS-021A-E3A-R3_BROKER_SERVER_ACCOUNT_REGISTRY.md
```

The complete final `git status --short --untracked-files=all` also continues to list the pre-existing files described in the first paragraph of this section; no such artifact was removed, renamed, staged, or discarded. Ignored build outputs were not cleaned.

## 3. Preserved work and protected hashes

**VERIFIED.** Protected legacy worktree source hashes match the pre-R3 values supplied by the baseline record. The pre-existing staged lockfile blob is unchanged.

| Path | Final SHA-256 | Result |
|---|---|---|
| `git show :Cargo.lock` | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` | Pre-existing staged blob preserved |
| `Cargo.lock` worktree | `549d67b306e609fc6360a6f8d20281dcd33c17949904d510e2807d6eedd891d5` | Additional unstaged R3 dependency resolution |
| `docker-compose.yml` | `0a761eb4c42c5d9b72cd9f642ea7ef8411fc70106205b2acbcb2e90ef4789866` | Preserved |
| `.github/workflows/ci.yml` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` | Preserved |
| `crates/intent-bridge/src/mt5_runtime.rs` | `6799d6d47f5b497366aeb90ba945658e9b741138459e0ed42571120f0eff5ccc` | Preserved |
| `crates/intent-bridge/src/mt5_v1.rs` | `514c6bd0b7da5846e558dfc6e221e1e5192472e4b359f7200d493f67dbeddcb2` | Preserved |
| `crates/intent-bridge/src/mt5_transport.rs` | `a3343826b93724c0e0fc1de6722e2fc0b9b5d231b29557c8562e4b369d0a2ea0` | Preserved |
| `crates/venue-connectors/src/adapter.rs` | `8a3cdb402566e74a1386c359ce3919b721b2c08965fe8a41c68a3c0a55c76ddf` | Preserved |
| `migrations/20261113000001_broker_account_registry.sql` | `ee76686a784a3b62d0ae6743a78549ee4c4bbe9a548820115efd59f03c24b2be` | New R3 migration |

The two pre-existing migration backup files remain untracked and preserved. No commit or push was made.

## 4. Domain and database entity model

**VERIFIED.** The domain layer now models broker, broker server, external broker account, Quant account binding, provider evidence and verification records. New strongly typed identifiers are exported from `domain`.

The forward-only migration adds:

- `brokers` and `broker_servers` as controlled trusted catalogue entities.
- `external_broker_accounts` and versioned HMAC lookup aliases.
- `trading_account_broker_bindings` linked to the R2 organization entitlement.
- Append-only `broker_account_evidence`, `broker_account_verifications`, and binding event history.

The migration does not seed fake broker/server records and does not backfill existing accounts. It adds the composite entitlement key needed for organization-safe binding references, constrained status and revision fields, foreign keys, identity uniqueness, one-to-one pending/verified/suspended reservations, and database guards against invalid verified state and mutation of canonical identity. Historical bindings and evidence use restrictive/append-only behavior rather than cascade deletion.

## 5. Canonical broker/server identities and external identifier protection

**VERIFIED.** Broker and server identifiers use deterministic, ASCII-only, lowercase slug normalization with trim and separator normalization. Their normalization version is recorded. External account identifiers trim only surrounding ASCII whitespace, preserve case and leading zeros, reject ambiguous input, and do not appear in ordinary API responses.

The API calculates domain-separated HMAC-SHA256 lookup aliases scoped to broker, server and normalization version. It supports a keyset configuration with retained key versions and an active-key version; malformed or absent key configuration causes registration to fail closed with service-unavailable behavior. Single-key compatibility settings are supported. Raw and normalized identifier buffers are zeroized after use. HMAC values are not returned by the API or ordinary listing queries. Broker credentials, passwords, access tokens and private keys are not stored.

**PARTIALLY VERIFIED.** No suitable existing KMS/secret-manager contract was present, so an explicit process configuration contract was introduced. Key rotation supports overlapping aliases for duplicate detection, but a controlled alias re-key/enrollment workflow is not yet implemented. Since raw identifiers are intentionally not recoverable from storage, safe retirement of old keys needs a future operator-mediated process.

## 6. Verification, approval and provider boundary

**VERIFIED.** Registration creates `PENDING_EVIDENCE` records and audit history. An organization OWNER cannot approve their own claim. The API refuses transition to `VERIFIED`; the database requires authenticated matching evidence and approval-authority references before a verified record can exist.

**PARTIALLY VERIFIED / NOT IMPLEMENTED.** No established platform administrator/approval authority exists in this checkout, and no authenticated provider evidence ingestion adapter exists. No MT5 account metadata or other provider evidence was collected. Therefore registrations remain pending. The implementation deliberately does not create a global administrator role, accept terminal heartbeats as proof, or let an ordinary organization member self-certify ownership.

The configured production direction remains the official Windows MetaTrader5 Python API, but provider connectivity, conformance and evidence capture are outside this registry change. MQL5 EA heartbeat is not treated as independent ownership evidence.

## 7. Organization entitlement integration and lifecycle

**VERIFIED.** Registration transactionally rechecks active OWNER membership, organization state, active entitlement for the selected Quant account, and trusted broker/server existence. Bindings reference the existing R2 organization entitlement; the entitlement system was not replaced or duplicated. Duplicate identity conflicts return a generic response that does not reveal another tenant's ownership.

Owner lifecycle actions can suspend or revoke a binding using expected revisions; they append verification/event history. The route does not allow verified state. Partial unique constraints reserve a broker account against simultaneous pending/verified/suspended assignments, and reserve a Quant account against another active binding.

**PARTIALLY VERIFIED.** Explicit cross-organization reassignment has not been implemented. Revocation preserves history and uniqueness prevents implicit transfer; a future reassignment flow must require authority in the relevant organization(s), provider evidence, approval, reason and transactional revision checks.

## 8. Tenant-scoped API access inventory

**VERIFIED.** Added routes:

- `GET /api/v1/brokers` — read trusted broker/server catalogue.
- `GET /api/v1/organizations/{organization_id}/broker-account-bindings` — tenant-scoped listing.
- `POST /api/v1/organizations/{organization_id}/broker-account-bindings` — OWNER-only pending registration.
- `PATCH /api/v1/organizations/{organization_id}/broker-account-bindings/{binding_id}` — OWNER lifecycle change to suspend/revoke with revision.

JWT subject and active organization membership are revalidated. Path/header tenant mismatch and cross-tenant lookup are concealed as not found; an active member lacking the OWNER permission receives forbidden. Catalogue writes and approval/evidence ingestion are absent. Ordinary API responses do not contain external account identifiers or HMAC lookup tokens. Journal ownership behavior was not changed.

**PARTIALLY VERIFIED.** Trusted catalogue changes are restricted to migrations/controlled provisioning because there is no established administrative API authority. Database runtime-role least-privilege/RLS configuration is not introduced by this migration and remains an infrastructure task.

## 9. Migration versions and database evidence

**VERIFIED.** R3 migration is `20261113000001_broker_account_registry.sql`, SHA-256 `ee76686a784a3b62d0ae6743a78549ee4c4bbe9a548820115efd59f03c24b2be`. Historical migrations were not edited. The persistence bootstrap now recognizes an exact R2-complete migration history and applies R3; schema recognition checks the new alias-token structure.

Database tests used only the guarded disposable QOS-020 PostgreSQL 16 cluster. Before every database-capable test command, the existing `qos020_guard.py` validated the exact manifest/run identity and command. No staging or production endpoint was used.

**VERIFIED — fresh install and R2 upgrade:** the latest guarded R3 persistence suite passed fresh installation and upgrade from the exact supported R2 migration state. Upgrade fixtures retained organization/entitlement/environment state and received no inferred broker identity. Migration integrity tests also passed.

**LIMITATION:** These are disposable QOS-020 integration database executions. No migration was applied to MT5 staging or production. Backup restoration rehearsal is not part of R3.

## 10. Test and static-check results

All database-capable integration tests were run through the verified QOS-020 guard. Latest successful results:

| Command / suite | Result |
|---|---|
| Guarded `cargo test -p persistence --test qos021a_e2_migrations --test qos021a_e3a_r2_tenant --test qos021a_e3a_r2_concurrency --test qos021a_e3a_r3_broker_registry -- --test-threads=1` | PASS: 18 migration + 8 R2 tenant + 3 R2 concurrency + 9 R3 tests (38 total) |
| Guarded `cargo test -p journal-api -- --test-threads=1` | PASS: 23 tests, including R3 HTTP authorization/privacy/registration flow and existing SQLx-backed R2 coverage |
| `cargo test -p domain` | PASS: 41 unit tests + 1 architecture integration test (42 total); doc tests passed |
| `cargo test -p persistence --lib` | PASS: 0 library unit tests; integration tests are reported separately above |
| `cargo check -p domain -p persistence -p journal-api --all-targets` | PASS |
| `cargo clippy -p domain -p persistence -p journal-api --all-targets -- -D warnings` | PASS (after final implementation changes) |
| `rustfmt --edition 2021 --check` on changed R3 Rust files | PASS |
| `git diff --check` | PASS |
| QOS-020 SQLx cleanup verifier | PASS: no generated `_sqlx_test_*` databases remain |

The final domain test, persistence library test, all-target check, targeted formatting and whitespace validation were rerun after the final source changes. The guarded persistence/API runs and clippy passed after the latest schema/API changes. No tests were run against staging or production.

## 11. Concurrency and SQLx cleanup

**VERIFIED.** Actual PostgreSQL concurrency tests covered simultaneous cross-tenant claims for one external identity, competing claims for one Quant account, and competing binding lifecycle revisions. Final state checks confirmed one winner and no partial duplicate state. Persistence tests also covered duplicate aliases across retained key versions, entitlement enforcement, lifecycle revisions, evidence identity mismatch, verified-state rejection, and append-only history.

**VERIFIED.** The QOS-020 cleanup verifier found no SQLx-generated test databases after successful runs. Earlier failed R3 attempts left two generated test DBs; those exact databases were removed through the QOS-020-guarded disposable endpoint. Subsequent cleanup verification passed. No container, network or volume was removed.

QOS-020 identity used for these checks:

- Run: `20261110-230504-ffb59dfd0ab4`
- Project: `qos020-integration-20261110-230504-ffb59dfd0ab4`
- Container: `qos020-integration-20261110-230504-ffb59dfd0ab4-postgres-1`
- Container ID: `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82`
- Network: `qos020-integration-net-20261110-230504-ffb59dfd0ab4`
- Volume: `qos020_it_2026_11_10_230504_ffb59dfd0ab4`
- Image: `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777`
- Endpoint: `127.0.0.1:51462`; disposable database `qos020_test_master`; restricted SQLx role `qos020_sqlx`.

## 12. Execution boundary and security limitations

**VERIFIED.** R3 does not modify the E1-R1 venue adapter boundary, MT5 transport visibility, MT5 `dispatch_submit` deny-all behavior, account environment classification, or execution authorization rules. Broker identity registration does not alter PAPER/SHADOW/LIVE environment, verify LIVE environment, or grant execution authority. No broker order submission, MT5 order submission, terminal operation or trading action occurred.

Remaining limitations:

1. No authenticated provider evidence source or evidence ingestion API.
2. No platform approval authority; therefore no verified external ownership claims.
3. No explicit reassignment workflow.
4. No catalogue administration API; controlled provisioning only.
5. HMAC old-key retirement/alias migration process not yet implemented.
6. Dedicated DB role/RLS hardening for the new registry remains an infrastructure concern.
7. R3 registration and persistence integration tests do not establish provider conformance or execution readiness.

## 13. Staging, production and broker-service confirmation

**VERIFIED.** No staging or production database was changed, and no staging/production migration ran. No broker service, MT5 terminal, worker, daemon, certificate, Docker lifecycle, or trading execution was touched. QOS-020 container, network and volume remain provisioned for future isolated testing.

## 14. Acceptance status

**PARTIAL.** The domain, schema, persistence, tenant-scoped registration/lifecycle API, privacy controls, migration recognition, guarded fresh-install/R2-upgrade coverage, and database concurrency checks are implemented and validated. R3 cannot be marked COMPLETE because authenticated provider evidence, established administrative approval authority, explicit reassignment, and key-retirement operations remain unimplemented. All external registrations remain pending, and no external account ownership or execution readiness is asserted.

Recommended next sequence: establish a platform approval authority and authenticated provider evidence contract; implement evidence ingestion and approval with independent actors; add controlled reassignment and HMAC alias retirement; define DB runtime-role/RLS policy; then rerun guarded migration, API, concurrency, privacy and QOS-020 cleanup acceptance. R4 and all execution work require their separate authorization.

# QOS-021A-E3A-R3B-A-I1-R1 Acceptance Remediation Report

**Repository:** `/Users/olaoluwatunmise/event-driven-trading-rs`  
**Branch / HEAD:** `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59`  
**Recommendation:** **CONDITIONAL ACCEPT**  
**Scope:** I1-R1 only. No I2 work, deployment, staging or production operation was performed.

## Summary

The three high-severity audit findings were addressed in code and a forward-only migration. Refresh, access-token validation, logout, access-token revocation, and account suspension now serialize using a consistent user → session → token lock order. Suspension atomically revokes sessions and their refresh/access tokens, and database triggers prevent event UPDATE, DELETE, and TRUNCATE while a composite foreign key binds event attribution to the session owner. Both APIs now use a separate migration connection and fail startup unless the runtime role meets the session-event privilege contract.

Targeted Identity API, persistence, Journal API, and JWT tests passed. The QOS-020 guard passed before each database-capable test command, and its final cleanup check found no leftover SQLx databases. Production runtime-role acceptance remains unverified: the disposable QOS-020 SQLx role owns its test tables and cannot create a separate runtime role. The positive least-privilege startup path therefore needs verification in a separately provisioned isolated deployment rehearsal before production rollout.

## Findings and remediation

### Files changed for this remediation

- `crates/identity-api/src/service.rs`
- `crates/identity-api/src/main.rs`
- `crates/identity-api/README.md`
- `crates/journal-api/src/main.rs`
- `crates/journal-api/README.md`
- `crates/persistence/src/repos/identity_sessions.rs`
- `crates/persistence/src/pool.rs`
- `crates/persistence/src/lib.rs`
- `crates/persistence/tests/qos021a_i1_sessions.rs`
- `crates/identity-token/src/lib.rs`
- New forward-only migration: `migrations/20261115000001_identity_suspension_and_audit_hardening.sql`
- This report.

All source/test files above except the new migration and report already had dirty or untracked state at preflight; changes were applied in place without discarding their existing work.

### 1. Suspension and reactivation could leave sessions usable — REMEDIATED

- Added migration `20261115000001_identity_suspension_and_audit_hardening.sql`. Its `identity_user_suspension_revokes_sessions` trigger runs in the same transaction as `identity_users.is_active = FALSE`; it revokes active sessions, increments each session revision, revokes refresh/access tokens, and appends a revocation event. Trigger failures abort the identity update. A later reactivation does not change the revoked session state.
- `crates/identity-api/src/service.rs`, `refresh`, now discovers token lock keys without locking and then locks user, session, and refresh token in order. It checks inactive users and propagates revocation and commit failures instead of discarding them. Expired-token revocation errors also fail closed.
- `crates/persistence/src/repos/identity_sessions.rs`, `validate_access_token`, follows the same user → session → access-token lock order and returns database errors rather than treating failed invalidation as successful authorization.
- `crates/identity-api/src/service.rs`, `logout` and `revoke_access_token`, lock and revalidate the current user/session within their mutation transaction before revoking.

### 2. Session-event audit immutability and attribution — REMEDIATED WITH DEPLOYMENT ROLE REQUIREMENT

- The new migration adds an UPDATE/DELETE row trigger and a TRUNCATE statement trigger, revokes UPDATE/DELETE/TRUNCATE from `PUBLIC`, and replaces the single-column event/session relationship with `(session_id,user_id) → identity_sessions(id,user_id) ON DELETE RESTRICT`.
- Where `edgenest_runtime` exists before migration, the migration grants it SELECT/INSERT on `identity_session_events` and USAGE/SELECT on its serial sequence, after revoking its direct table/sequence privileges. The suspension trigger function is `SECURITY DEFINER`, has a fixed search path, and has PUBLIC execution revoked so the runtime role need not receive broad session/token mutation privileges for the trigger to work.
- `crates/persistence/src/pool.rs`, `validate_identity_runtime_role`, first confirms that the runtime connection points at a database with the exact successful I1-R1 SQLx checksum and required triggers/composite FK. It then rejects a runtime role that is superuser, has CREATEROLE/CREATEDB, owns or inherits the event-table owner role, can UPDATE/DELETE/TRUNCATE/TRIGGER the event table, lacks SELECT/INSERT or sequence USAGE, or can CREATE in the table schema. Both service entrypoints call this check after applying migrations and opening the runtime pool; this also prevents the migration URL and runtime URL from silently targeting different schema states.
- A positive runtime-role connection was not available in QOS-020. Its `qos020_sqlx` role is intentionally CREATEDB-capable and owns SQLx-created tables; a test verifies that this owner role fails the check. Provisioning and proving the separate restricted runtime role remains a deployment gate.

### 3. Migration cutover and mixed-version behavior — DOCUMENTED AND FAIL-CLOSED

- `crates/identity-api/src/main.rs` and `crates/journal-api/src/main.rs` require `MIGRATOR_DATABASE_URL` for migration execution, close the migration pool, then open `DATABASE_URL` and validate runtime privileges before binding their listeners.
- Both service READMEs document the coordinated rollout requirement, role separation, fail-closed startup, forced reauthentication, and the lack of a down migration. Older binaries are not supported after the new migration is applied; the migration history is forward-only and old SQLx binaries must not participate in a mixed-version rollout.
- The migrator recognizes the prior complete I1 history (`20261114000001`) as a valid forward-upgrade point and validates the new hardened schema once the `20261115000001` migration is present. Earlier migrations, including `20261114000001_identity_sessions_and_tokens.sql`, were not edited.
- No deployment or migration rehearsal against staging/production was performed. The migration executed only through the guarded SQLx disposable databases described below.

## Migration assessment

**Migration:** `20261115000001_identity_suspension_and_audit_hardening.sql`

- Forward-only; no existing migration file or checksum was changed.
- Performs constraint/trigger/function/privilege DDL; it does not delete application rows or rewrite existing account/user/refresh-token data.
- Adds a table constraint requiring event session and user attribution to match. Existing rows that do not match will cause the migration to fail instead of being silently repaired or discarded; preflight must check for such rows before production cutover.
- Adds locking DDL and triggers. Apply during the coordinated identity/journal maintenance window with old writers drained. The migration runs under the SQLx migration transaction; migration/lock duration has not been measured on production-sized tables.
- The `edgenest_runtime` database role must exist before this migration for the conditional grants to apply. If created later, an authorized database administrator must apply the documented equivalent grants and re-run startup role validation before service readiness.
- No rollback migration is provided. Recovery is a forward fix or a separately authorized restore. Existing I1 migration behavior already revokes legacy unbound refresh tokens; affected users must authenticate again.

## Test and verification evidence

All database-capable test invocations below were run through the unchanged guard. It verified run `20261110-230504-ffb59dfd0ab4`, project `qos020-integration-20261110-230504-ffb59dfd0ab4`, endpoint `127.0.0.1:51462`, database `qos020_test_master`, and role `qos020_sqlx`. The guard reported exit code `0` before each test command.

### Passing commands

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo check --locked -p identity-token -p persistence -p identity-api -p journal-api` | PASS, exit 0 |
| `cargo test --locked -p identity-token` | PASS, 7 passed; includes unknown `kid`, tampered signature, issuer/audience/type mismatch, incomplete claims, expiration, key-pair/rotation, retired-key, and Unix private-key mode checks |
| `cargo clippy --locked --bins -p identity-token -p persistence -p identity-api -p journal-api -- -D warnings` | PASS, exit 0 |
| Guarded `cargo test --locked -p identity-api --bin identity-api -- --test-threads=1` | PASS, 11 passed: refresh replay, refresh/logout race, refresh/access-revocation race, refresh/suspension race, suspension/reactivation, refresh and suspension audit-failure injection, event TRUNCATE/cross-user attribution, and the two service unit tests |
| Guarded `cargo test --locked -p persistence --test qos021a_i1_sessions -- --test-threads=1` | PASS, 3 passed: active/revision/inactive/expired/revoked token checks and owner-role rejection |
| Guarded `cargo test --locked -p journal-api --bin journal-api authenticated_http_routes_enforce_tenant_role_and_revalidation -- --test-threads=1` | PASS, 1 passed: protected HTTP route session revalidation and tenant/role enforcement |
| Final unchanged guard with `--verify-sqlx-cleanup` | PASS, exit 0; `SQLx generated test databases: none remain` |

The guard was invoked with this prefix for each DB-capable test (followed by the exact `cargo test` command shown above):

```sh
python3 scripts/qos020/qos020_guard.py \
  --manifest '/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/manifest.json' \
  --secrets-dir '/var/folders/9f/dspzp56j3pjcftph22hs9gw40000gn/T/qos020/20261110-230504-ffb59dfd0ab4/secrets' \
  --lane integration-test \
  --run-id 20261110-230504-ffb59dfd0ab4 -- <cargo test command>
```

Guard identity evidence (credential values were not inspected or recorded):

```text
QOS-020 identity verified: project=qos020-integration-20261110-230504-ffb59dfd0ab4 run_id=20261110-230504-ffb59dfd0ab4 host=127.0.0.1 port=51462 database=qos020_test_master role=qos020_sqlx
SQLx generated test databases: none remain
exit_code=0
```

### Lint limitation

`cargo clippy --locked --all-targets -p identity-token -p persistence -p identity-api -p journal-api -- -D warnings` returned exit `101` because the pre-existing Journal API test helper `request` in `crates/journal-api/src/main.rs:383` triggers `clippy::too_many_arguments` (8 arguments). The production binary Clippy command above passes. The helper was not refactored as unrelated work.

## Preservation and repository evidence

- Pre- and post-remediation branch/HEAD: `main`, `f6446dd33dc74a86ede9064f35aef2ec082a1c59`.
- The preflight worktree had 101 status entries. At completion, the original 101 entries remain with the same status codes; the only additional paths are the authorized migration and this report. Existing staged state (`Cargo.lock`, `docker-compose.yml`), modified/deleted paths, and untracked files remain present.
- No reset, stash, clean, checkout, stage, unstage, commit, or push was performed. No existing file was deleted or overwritten. Cargo used the repository's ignored `target/` build cache; it was not cleaned.
- Protected unchanged hashes (SHA-256):

| File/resource | SHA-256 |
|---|---|
| Original I1 implementation report | `2aba08e1b9ec5ba83d9e28ce5b97155b3c12199b71a161ee73ac45a2633dde4f` |
| Original I1 acceptance audit | `4e90768f50dd140408b6aa4bece81f739cff7ac12b542824c0efa55fbaf24b0b` |
| Prior I1 migration `20261114000001_identity_sessions_and_tokens.sql` | `3a2b1d9915838975bd7f02466dd4fa7e4da066335ac720f8e1aa2aafc9dd1ed8` |
| QOS-020 guard `scripts/qos020/qos020_guard.py` | `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b` |
| QOS-020 manifest | `f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e` |

`git diff --check` passed with exit `0`. No database outside the disposable QOS-020 test cluster, service, Docker resource, staging/production configuration, MT5 dispatch path, or broker execution configuration was modified. No trade was executed.

## Remaining risks and deployment requirements

1. **Runtime-role positive-path verification is outstanding.** Provision `edgenest_runtime` before migration with separate migration/runtime credentials and all required application table grants; confirm the event table/sequence privilege check passes using the runtime role and fails for owner/superuser/over-privileged roles. The QOS-020 cluster cannot safely provide that separate role under its current guard contract.
2. **Migration preflight and coordinated rollout are still required.** Check event/session attribution consistency, take and verify a backup, rehearse on a restored isolated copy, drain old Identity and Journal writers, provision roles, apply the migration once, deploy both compatible binaries together, verify readiness, and require reauthentication. No staging rehearsal was run in this phase.
3. **No down migration exists.** Do not roll back by starting an older API binary against the upgraded database. Use a forward repair or separately approved restore plan.
4. **Lint:** the all-target Clippy command remains blocked by the existing unrelated Journal API test helper lint noted above.

## Acceptance recommendation

**CONDITIONAL ACCEPT for the I1-R1 source remediation.** The targeted behavior, fault-injection, concurrency, JWT, persistence, and Journal middleware checks pass on isolated QOS-020 databases, and the migration is forward-only. Production cutover is not accepted until a separately provisioned restricted runtime role passes the startup privilege check and the migration is rehearsed on a restored isolated database. No I2 work or production rollout has begun.

## Explicit non-actions

No staging or production database was accessed. No production migration, deployment, Docker lifecycle change, service restart, credential change, MT5 dispatch change, or broker execution occurred. The QOS-020 guard and manifest were not modified. The original I1 implementation report was not modified.

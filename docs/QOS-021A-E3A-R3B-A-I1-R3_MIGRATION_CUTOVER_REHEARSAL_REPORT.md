# QOS-021A-E3A-R3B-A-I1-R3 Migration Cutover Rehearsal

**Recommendation: CONDITIONAL ACCEPT for I1-R1 migration and runtime behavior in the isolated rehearsal.** The restored-copy migration, least-privilege startup, data preservation, suspension atomicity, reauthentication and failure recovery checks passed. This is not approval for staging or production migration or deployment. The exact legacy application executable was not available for a direct binary compatibility test; the old migration-history set was tested and SQLx rejected it against the upgraded database.

## 1. Repository identity and preservation

- Repository: `/Users/olaoluwatunmise/event-driven-trading-rs`
- Branch: `main`
- HEAD: `f6446dd33dc74a86ede9064f35aef2ec082a1c59`
- Before the report, `git status --short --untracked-files=all` had 104 entries. The complete captured output is in the Appendix.
- The pre-existing staged, unstaged, deleted and untracked paths remained unchanged. After report creation, the only status delta is this report as a new untracked file. No reset, stash, clean, checkout, restore, stage, unstage, commit or push was performed.
- `git status --ignored --short --untracked-files=all` captured 222,803 lines before the rehearsal. The 222,699 ignored entries were byte-for-byte identical at the end; their sorted status-line inventory SHA-256 was `1ff74a6ef369b2e388fd59168e704fcf57aca536896ef0e852d40089b049b889` both before and after. No Cargo build, formatting or test command was run, and no ignored worktree cleanup or restoration occurred. The existing debug binaries were reused.
- SHA-256 before and after the rehearsal matched for the existing binaries and protected files:

  | Artifact | SHA-256 |
  | --- | --- |
  | `target/debug/identity-api` | `9b7df1fb4ed1fbd486952b8c4f8d36ac6dafcf0e74943eba9f602ac17ddb30d7` |
  | `target/debug/journal-api` | `45b0668d0f2cb62be1ad8b7d9c1205d144d677493eb9863e9e707da2eea538ef` |
  | `target/debug/persistence-migrate` | `434efce559459cabb12b3ec5ea92e708697da38190686cc1f0dbe75827976f0d` |
  | QOS-020 guard `scripts/qos020/qos020_guard.py` | `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b` |
  | QOS-020 manifest | `f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e` |
  | Migration `20261114000001_identity_sessions_and_tokens.sql` | `3a2b1d9915838975bd7f02466dd4fa7e4da066335ac720f8e1aa2aafc9dd1ed8` |
  | Migration `20261115000001_identity_suspension_and_audit_hardening.sql` | `740c7f7074381c84f1aa9f326ecbb00cc844705d6f2f2e28611bfa8e6c28fef9` |

No source, migration, QOS-020 guard/manifest, staging or production file was changed. No repository build artifact was rebuilt.

## 2. Disposable environment and isolation

The rehearsal used Docker context `desktop-linux`, Docker Engine 27.5.1, PostgreSQL 16 on ARM64, and immutable image `postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777` (image ID `sha256:7e7dbab8d3b431a20793a6d99cb5a6bc84e44914309917f1bf5589a7568cdefd`). The container had a 1 GiB memory and 2 CPU limit, a dedicated network and volume, and published only `127.0.0.1:51558 -> 5432`.

| Resource | R3 identity | Verification |
| --- | --- | --- |
| Run ID | `20261112t134443z` | Private marker checked by the R3 safety preflight before database operations |
| Container | `qos021a-i1-r3-20261112t134443z-postgres`; ID `3a9783c86bf772ea0a6174bb52adc0023db16e855dd37f5f990be6072dd7da8f` | Running and healthy; exact digest, label, port, volume and network inspected |
| Volume | `qos021a-i1-r3-20261112t134443z-data` | Dedicated, labeled for this run |
| Network | `qos021a-i1-r3-20261112t134443z-net`; ID `d6f9c38a18b6907c26db753c08bc8f6227312f480978ef67530b88acd97197dc` | Dedicated; only the R3 PostgreSQL container attached |
| Host endpoint | `127.0.0.1:51558` | Loopback-only port binding |
| Databases | `qos021a_i1_r3_source`, `qos021a_i1_r3_bad`, `qos021a_i1_r3_rehearsal` | All run-marked; exact database name checked on every live preflight |

The container was created directly with `docker run`, not from repository Compose configuration. Compose-like project/config labels were explicit run labels, not evidence that Compose launched it. No repository Docker Compose file was used.

The dedicated private preflight checked the Docker context, protected resource IDs/labels, R3 container/image/volume/network/port identities, URL host/port/database/role, and the database marker. The exact command form was:

```sh
python3 /private/tmp/qos021a-i1-r3-20261112t134443z/preflight.py \
  --phase live --database <one-of-the-three-r3-databases> --role <expected-role>
```

Its output was `R3 PREFLIGHT PASS` before each migration, SQL command, backup/restore, API startup and negative operation. The API-start checks separately preflighted both `MIGRATOR_DATABASE_URL` and `DATABASE_URL`. Credentials were kept in mode-0600 private temporary files; none are present in this report.

Protected resource checks found QOS-020 container `f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82` (run `20261110-230504-ffb59dfd0ab4`) running and healthy, and MT5 staging container `dc9abdf6c9ae2a7560f95fb1ce5e5903e1ab4def23627bbd0d48c4bdff03a2e7` running and healthy. The protected staging volume and both protected networks retained their recorded identities.

## 3. Pre-migration source and fixture

The source database was migrated using byte-identical copies of the repository SQL files. The source was first brought through `20261113000001` (17 migrations). Before `20261114000001`, it received only synthetic fixture data: two users, one organization, two memberships, one active legacy refresh token and one already-revoked legacy refresh token. The raw synthetic refresh values and password hashes were not production credentials.

The guarded command was:

```sh
sqlx migrate run --source /private/tmp/qos021a-i1-r3-20261112t134443z/migrations-pre13
```

Result: exit `0`; all 17 migrations applied. Then the guarded command:

```sh
sqlx migrate run --source /private/tmp/qos021a-i1-r3-20261112t134443z/migrations-pre14
```

Result: exit `0`; 18 successful migration records, latest `20261114000001`. The database recorded checksum `844b3ff19f2c6f855038b2762fae328c215038b3e1340bcda6084ad66c3b10bec946645c20b810baff35f64560997b34`, matching the SQLx checksum of the unchanged migration file.

The `20261114000001` migration invalidated both legacy unbound refresh rows: after migration, 0 were active, 2 were revoked, and both retained `session_id IS NULL`. It did not invent sessions for the legacy tokens. The source was then populated with representative session-bound history:

| Data | Pre-R1 source count/state |
| --- | --- |
| Users | 2 |
| Organizations / memberships | 1 / 2 |
| Sessions | 3: 2 active, 1 revoked |
| Refresh tokens | 6: 2 active, 4 revoked; 2 old unbound rows revoked by `20261114000001` |
| Access tokens | 2: 1 active, 1 revoked |
| Session events | 4 |
| Event rows with mismatched session/user attribution | 0 |

Pre-migration integrity checks confirmed each event's `user_id` matched its referenced session owner. The source remained on 18 successful migrations, latest `20261114000001`.

## 4. Snapshot and restored-copy evidence

A PostgreSQL custom-format dump was taken from the run-marked source before `20261115000001`. The isolated safety schema was intentionally excluded from the application-data dump and recreated/retagged for each disposable database.

The backup was 256,331 bytes, SHA-256 `fd415773977e8ca8322125e244ad204b04c6cbfe97b31644feab7969d0160a21`. `pg_restore --list` exited `0`. It was restored into the separate `qos021a_i1_r3_rehearsal` database with `pg_restore --exit-on-error --no-owner`; restore exit code was `0`.

Before migration, the restored copy verified at 18 successful migrations and the same `20261114000001` checksum. It had the same 2 users, 1 organization, 2 memberships, 3 sessions, 6 refresh tokens, 2 access tokens and 4 session events as the snapshot source; there were 0 invalid event attributions and 0 active unbound legacy refresh tokens. This directly demonstrated restoration to the pre-I1-R1 schema in a separate database. The original source database was also left at that pre-migration schema as an additional disposable recovery copy.

## 5. Inconsistent-attribution migration failure

`qos021a_i1_r3_bad` was cloned from the pre-migration source. Its historical append-only trigger correctly rejected an attempted UPDATE, so no existing event was changed. The negative case then inserted one synthetic event whose `session_id` belonged to the owner user while `user_id` referenced the second valid user. The older schema's independent foreign keys accepted that row; preflight confirmed exactly 1 mismatched event before attempting migration.

The guarded command was:

```sh
sqlx migrate run --source /private/tmp/qos021a-i1-r3-20261112t134443z/migrations-all
```

Result: exit `1`, expected:

```text
error: while executing migration 20261115000001: error returned from database: insert or update on table "identity_session_events" violates foreign key constraint "identity_session_events_session_user_fk"
```

Atomicity checks then found 18 successful migrations and no `20261115000001` history row, no composite session/user FK, no truncate trigger, and no suspension trigger. The database still held 2 users, 1 organization, 2 memberships, 3 sessions, 6 refresh tokens, 2 access tokens and all 5 event rows including the single inconsistent row. No row was silently repaired or discarded.

## 6. Role separation and cutover sequence

Three principals were isolated to this disposable PostgreSQL cluster:

- `qos021a_r3_bootstrap`: cluster provisioning and read-only verification only; never used by either API.
- `qos021a_r3_migrator`: non-superuser migration/database owner, used for SQLx migrations and fixture preparation; never used by positive API runtime connections.
- `edgenest_runtime`: non-owner, non-superuser, no CREATEDB/CREATEROLE, used for API runtime connections. It had application CRUD/sequence grants required by the services, but only event SELECT/INSERT, no event UPDATE/DELETE/TRUNCATE/TRIGGER, no schema CREATE, sequence USAGE/SELECT without UPDATE.

The observed validator privilege vector was:

```text
superuser=false, createdb=false, createrole=false, event_owner=false,
owner_member=false, event_select=true, event_insert=true, event_update=false,
event_delete=false, event_truncate=false, event_trigger=false,
schema_create=false, sequence_usage=true, sequence_update=false
```

Rehearsed sequence:

1. Verify the pre-migration attribution query returns 0 inconsistent rows.
2. Create and verify the pre-migration custom backup.
3. Restore into the separate rehearsal database and compare migration checksum and representative row counts.
4. Check `pg_stat_activity`; there were 0 other application or migrator connections before cutover. No old service was started against this test database.
5. Apply `20261115000001` once using `qos021a_r3_migrator`.
6. Verify the SQLx record/checksum, FK, triggers and privileges.
7. Start Identity and Journal API together using separate `MIGRATOR_DATABASE_URL` and `DATABASE_URL` credentials.
8. Verify readiness, registration/token issuance, authenticated Journal access and legacy refresh rejection.

Migration command on the restored copy:

```sh
sqlx migrate run --source /private/tmp/qos021a-i1-r3-20261112t134443z/migrations-all
```

The command exited `0`; the database ended with 19 successful migrations, latest `20261115000001`. SQLx checksum was `c9b8f7bc0a8570f0780a1b6733ef80ae21062c032cc372246e9e17fce90afa2f448d60112c5e3df34c36e306d406fb92`, matching the authoritative migration. The composite FK was validated and the update/delete, truncate and suspension triggers were present. The initial application row counts remained unchanged by migration.

## 7. Post-migration integrity and behavior

### Audit and attribution protections

All operations below used the R3 safety preflight immediately beforehand.

| Attempt | Principal | Result |
| --- | --- | --- |
| Event UPDATE | `edgenest_runtime` | Exit `1`, permission denied |
| Event DELETE | `edgenest_runtime` | Exit `1`, permission denied |
| Event TRUNCATE | `edgenest_runtime` | Exit `1`, permission denied |
| Event trigger creation | `edgenest_runtime` | Exit `1`, permission denied |
| Schema CREATE | `edgenest_runtime` | Exit `1`, permission denied |
| Cross-user session-event INSERT | `edgenest_runtime` | Exit `1`, composite FK `identity_session_events_session_user_fk` rejected it |
| Event UPDATE / DELETE / TRUNCATE | Migration/table owner | Exit `1` for each; append-only database triggers raised `identity session events are append-only` |

The failed cross-user insert did not add a row.

### Suspension, rollback on audit failure, and reactivation

A temporary trigger in the disposable database raised `R3 injected suspension audit failure` when the suspension function attempted its audit insert. The runtime `UPDATE identity_users SET is_active=FALSE ...` exited `1`. Follow-up reads showed the user still active, both sessions still ACTIVE at revision 1, active refresh tokens unchanged, access token unchanged and no suspension audit events. The temporary fault-injection trigger/function was then removed.

The same suspension using `edgenest_runtime` then succeeded. It set the user inactive, changed both active sessions to REVOKED revision 2, revoked all four owner refresh rows and the active owner access token, and inserted 2 suspension events. Reactivating the user succeeded but left both sessions revoked at revision 2 and all those tokens revoked. This demonstrates transactional rollback on audit failure and no session revival on reactivation.

### API startup and authenticated requests

The pre-existing binaries were reused without rebuilding:

- Identity API: `target/debug/identity-api`, SHA-256 `9b7df1fb4ed1fbd486952b8c4f8d36ac6dafcf0e74943eba9f602ac17ddb30d7`.
- Journal API: `target/debug/journal-api`, SHA-256 `45b0668d0f2cb62be1ad8b7d9c1205d144d677493eb9863e9e707da2eea538ef`.

Test-only JWT fixture keys were copied to the private temporary directory; no production signing keys were used. Both APIs started together, passed runtime-role validation and returned readiness HTTP `200` on loopback ports `18182` and `18183`. Identity synthetic registration/token issuance returned HTTP `201`; the access token authenticated Journal `GET /api/v1/organizations` with HTTP `200`. Token values were suppressed. A known synthetic legacy unbound refresh token returned HTTP `401`, confirming reauthentication is required.

While both APIs were live, a guarded `pg_stat_activity` query observed 4 `edgenest_runtime` client connections and 0 `qos021a_r3_migrator` connections. Both processes were then stopped; their ports had no remaining listeners. Final rehearsal counts were 3 users (including the API-created synthetic user), 1 organization, 2 memberships, 4 sessions, 7 refresh tokens, 3 access tokens and 7 valid events. The newly registered API user had one active session.

### Unsafe role, wrong schema and mixed migration history

- Both APIs were started separately with the migration/table-owner role as runtime. Each exited `2` before listening with the `database runtime role is not sufficiently restricted` diagnostic.
- Both APIs were started with a valid migrator URL pointing at the upgraded rehearsal DB and runtime URL pointing at the source DB still at `20261114000001`. Each exited `2` before listening with the same validator diagnostic. The source remained at 18 successful migrations; the API did not migrate it because the migrator URL was deliberately separate.
- To test the old migration-history contract without running an unidentified old executable, SQLx was run on the upgraded disposable database with the pre-I1 migration set (through `20261114000001`). It exited `1`: `migration 20261115000001 was previously applied but is missing in the resolved migrations`. No schema change occurred. This proves that an application using the old migration set cannot successfully execute this startup migration path against the upgraded DB. It is not a binary-level test of a specific historical API artifact.

## 8. Recovery and rollback assessment

There is no down migration. The verified recovery artifact is the pre-migration custom dump above. Its successful listing and restoration into a separate database demonstrated recoverability to the pre-I1-R1 migration state. During the rehearsal, the source copy remained at version `20261114000001` while the restored rehearsal copy advanced to `20261115000001`.

For a real cutover, prefer forward repair if post-migration writes have occurred and the data can be brought to a valid state. Restore the pre-migration snapshot only under an explicit recovery decision that accounts for writes since the snapshot and accepted data loss/reconciliation. Do not start an older API binary against the upgraded database. Rehearse backup age, restore duration, lock duration and application recovery using production-sized representative data before production approval; none of those timings or production data conditions were tested here.

## 9. Cleanup and protected-resource verification

After capturing evidence, only these R3 resources were removed:

```sh
docker rm -f qos021a-i1-r3-20261112t134443z-postgres
docker volume rm qos021a-i1-r3-20261112t134443z-data
docker network rm qos021a-i1-r3-20261112t134443z-net
```

The two API processes were already stopped. Post-cleanup inspection returned not-found for each R3 container/volume/network. QOS-020 remained running and healthy with its expected run label; MT5 staging remained running and healthy with its expected container ID; its protected volume and network IDs remained unchanged. No other Docker resources were removed or modified. The private R3 directory containing temporary credentials, test keys, dump and harness is removed after report verification.

## 10. Deviations and residual limitations

- The exact old Identity/Journal executable was not available or run. The old migration set was tested against the upgraded disposable DB and SQLx rejected it. Actual historical binary provenance and behavior remain unverified. Production cutover must drain and stop every old writer before migration and deploy only the compatible pair.
- No source-level Cargo test suite or build was run during R3. Direct SQL and live API checks exercised the required behavior, and the earlier R1 report contains the targeted test-suite results. The binaries used here were pre-existing local build artifacts; hashes were recorded before and after, but no new source-to-binary provenance claim is made.
- R3 used a synthetic database on local Docker Desktop. It does not establish production-size DDL lock duration, production data consistency, staging/production backup restore, TLS/network behavior, availability/failover, or deployment supervision.
- Safe preflight denials and fixture-command errors occurred during harness setup. Each was fail-closed or rejected before changing protected data: an early preflight lacked the recorded container ID; an idempotent marker attempt encountered an already-existing schema; an initial role script stopped because the bootstrap role already existed; an initial malformed fixture statement failed before inserts; an event UPDATE in the negative clone was blocked by the old append-only trigger; and an early restore preflight caught the copied source database name in the target marker. The marker was corrected under the initialization gate, and all acceptance checks were rerun successfully. No denied operation touched staging, production or protected QOS-020 resources.

## 11. Acceptance recommendation

**CONDITIONAL ACCEPT for I1-R1 migration behavior and isolated deployment contract.** Passed: isolated preflight, accurate pre-I1 state, backup listing and restore, migration checksum, valid-data preservation, atomic rejection of inconsistent attribution, restricted-role acceptance, unsafe-role and schema-mismatch rejection, append-only enforcement, suspension transaction rollback, reactivation safety, API readiness, authenticated request and legacy-token reauthentication. Remaining condition: verify the exact supported old service artifacts are drained and cannot enter the coordinated cutover; production-sized backup, lock and restore planning also remains outstanding. No staging or production migration/deployment is authorized by this result.

## Appendix A — Complete Git status before report creation

The following is the complete output of `git status --short --untracked-files=all` immediately before adding this report. After adding the report, every line below remained unchanged and the only additional line was `?? docs/QOS-021A-E3A-R3B-A-I1-R3_MIGRATION_CUTOVER_REHEARSAL_REPORT.md`.

```text
 M .github/workflows/ci.yml
MM Cargo.lock
 M Cargo.toml
 M crates/account-engine/src/simulated.rs
 M crates/domain/src/account.rs
 M crates/domain/src/ids.rs
 M crates/domain/src/lib.rs
 M crates/domain/tests/phase1_architecture.rs
 M crates/event-trading/src/execution/fill.rs
 M crates/execution-engine/src/oms.rs
 M crates/identity-api/Cargo.toml
 M crates/identity-api/README.md
 M crates/identity-api/src/main.rs
 M crates/identity-api/src/service.rs
 M crates/intent-bridge/src/lib.rs
 M crates/intent-bridge/src/mt5_runtime.rs
 M crates/intent-bridge/src/mt5_transport.rs
 M crates/intent-bridge/src/mt5_v1.rs
 M crates/intent-bridge/tests/mt5_transport_mtls.rs
 M crates/journal-api/Cargo.toml
 M crates/journal-api/README.md
 M crates/journal-api/src/auth.rs
 M crates/journal-api/src/dashboard.rs
 M crates/journal-api/src/main.rs
 M crates/persistence/Cargo.toml
 M crates/persistence/src/lib.rs
 M crates/persistence/src/pool.rs
 M crates/persistence/src/repos/accounts.rs
 M crates/persistence/src/repos/mod.rs
 M crates/persistence/tests/survive_restart.rs
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
?? crates/domain/src/broker_registry.rs
?? crates/domain/src/execution_authorization.rs
?? crates/domain/src/organization.rs
?? crates/identity-token/Cargo.toml
?? crates/identity-token/src/lib.rs
?? crates/identity-token/tests/fixtures/rotation_test_private.pem
?? crates/identity-token/tests/fixtures/rotation_test_public.pem
?? crates/journal-api/src/broker_registry.rs
?? crates/journal-api/src/organizations_api.rs
?? crates/journal-api/tests/fixtures/r2_test_private.pem
?? crates/journal-api/tests/fixtures/r2_test_public.pem
?? crates/persistence/src/repos/broker_registry.rs
?? crates/persistence/src/repos/identity_sessions.rs
?? crates/persistence/src/repos/organizations.rs
?? crates/persistence/tests/qos021a_e2_migrations.rs
?? crates/persistence/tests/qos021a_e3a_r2_concurrency.rs
?? crates/persistence/tests/qos021a_e3a_r2_tenant.rs
?? crates/persistence/tests/qos021a_e3a_r3_broker_registry.rs
?? crates/persistence/tests/qos021a_i1_sessions.rs
?? crates/venue-connectors/src/tests/ambiguous_unknown.rs
?? crates/venue-connectors/src/tests/cancel_confirmation.rs
?? crates/venue-connectors/src/tests/failure_injection.rs
?? docs/EDGENEST_MT5_MVP_IMPLEMENTATION_ROADMAP.md
?? docs/EDGENEST_MT5_MVP_READINESS_AUDIT.md
?? docs/QOS-020-R2_READ_ONLY_RECOVERY_DIAGNOSIS.md
?? docs/QOS-020-R3_READ_ONLY_GUARD_VERIFICATION.md
?? docs/QOS-020_TEST_INFRASTRUCTURE.md
?? docs/QOS-021A-E1-R1_GENERIC_VENUE_EXECUTION_BOUNDARY.md
?? docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md
?? docs/QOS-021A-E2_ACCOUNT_ENVIRONMENT_AND_MIGRATION_FOUNDATION.md
?? docs/QOS-021A-E3A-R1_ARCHITECTURE_DECISION_RESOLUTION.md
?? docs/QOS-021A-E3A-R2-F_VALIDATION_AND_COMPLETION.md
?? docs/QOS-021A-E3A-R2_ORGANIZATION_TENANT_FOUNDATION.md
?? docs/QOS-021A-E3A-R3A_BROKER_VERIFICATION_AUTHORITY_AND_EVIDENCE_ARCHITECTURE.md
?? docs/QOS-021A-E3A-R3B-A-I1-ACCEPTANCE_AUDIT.md
?? docs/QOS-021A-E3A-R3B-A-I1-R1_ACCEPTANCE_REMEDIATION_REPORT.md
?? docs/QOS-021A-E3A-R3B-A-I1-R2_RUNTIME_ROLE_VERIFICATION_REPORT.md
?? docs/QOS-021A-E3A-R3B-A-I1_IMPLEMENTATION_REPORT.md
?? docs/QOS-021A-E3A-R3B-A-R1_ARCHITECTURE_DECISION_RESOLUTION.md
?? docs/QOS-021A-E3A-R3B-A-R2_IMPLEMENTATION_SCOPE.md
?? docs/QOS-021A-E3A-R3B-A_STEP_UP_AUTHENTICATION_ARCHITECTURE.md
?? docs/QOS-021A-E3A-R3B-R_QOS020_RECOVERY_INVESTIGATION.md
?? docs/QOS-021A-E3A-R3_BROKER_SERVER_ACCOUNT_REGISTRY.md
?? docs/QOS-021A-E3A_TRUSTED_IDENTITY_AND_IMMUTABLE_RISK_ARCHITECTURE.md
?? docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md
?? migrations/20261111000001_journal_trading_account_fk_repair.sql
?? migrations/20261111000002_account_environment_provenance.sql
?? migrations/20261111000003_execution_authorizations_and_claims.sql
?? migrations/20261112000001_organization_tenant_foundation.sql
?? migrations/20261113000001_broker_account_registry.sql
?? migrations/20261114000001_identity_sessions_and_tokens.sql
?? migrations/20261115000001_identity_suspension_and_audit_hardening.sql
?? scripts/qos020/docker-compose.qos020.yml
?? scripts/qos020/init/10-qos020-init.sql
?? scripts/qos020/provision.py
?? scripts/qos020/qos020_guard.py
?? scripts/qos020/test_guard.py
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

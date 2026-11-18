# QOS-021A-E3A-R3B-A-I1-R — Independent Acceptance Audit

**Recommendation: CONDITIONAL ACCEPT.** The token verification and session enforcement structure is sound, and the implementation report records passing I1 test runs. I do not recommend closing I1 until the session-suspension lifecycle, audit immutability boundary, and migration cutover contract below are resolved. This is a source and existing-evidence review only; no database-capable tests were run.

## Scope and evidence standard

Reviewed the governing R1 and R2 documents, the I1 report, current source, migration, and checked-in test source. The reported test outcomes are taken from the I1 report; no CI run log was present in the repository, and the DB tests were not rerun as instructed. “PASS” below means an existing test is reported as passing and its source exercises the stated behavior, or the source directly establishes the behavior. “PARTIAL” means implementation or test coverage leaves a material gap.

## Findings by severity

### High — Suspension does not synchronously invalidate a user’s sessions

`crates/persistence/src/repos/identity_sessions.rs:38-49` discovers an inactive identity only when a token is checked, then attempts to revoke that session. `crates/identity-api/src/service.rs:423-433` does a similar check only when a refresh request arrives. No user-suspension/update path is wired to revoke all that user’s active sessions and refresh tokens.

As a result, a user can be made inactive and then reactivated without any request arriving during the inactive interval; the old active session and refresh token remain valid. The existing inactive-user test seeds an already-inactive user and validates once, so it does not exercise suspend/reactivate without an intervening request. In addition, the refresh inactive-user branch discards both `revoke_session_tx` and commit errors, so it can report unauthorized without proving that revocation persisted.

**Acceptance impact:** R1 ADR-6 and R2 I1 require account-suspension integration with session lifecycle. Treat that criterion as **FAIL** until suspension revokes sessions transactionally and failures are surfaced. Add a test for inactive-to-active transition without an intervening authenticated request, and one for revocation transaction failure.

### High — Append-only event protection does not cover all database mutation paths

`migrations/20261114000001_identity_sessions_and_tokens.sql:84-91` installs a row trigger for `UPDATE` and `DELETE`. It does not block `TRUNCATE`, and the migration adds no privilege boundary that denies the runtime role truncate/ownership rights. Both API binaries run `persistence::migrate` during startup (`crates/identity-api/src/main.rs:80-89`; `crates/journal-api/src/main.rs:254-261`), using their configured database URL. If that role owns the table or has `TRUNCATE`, it can erase the event history without invoking the row trigger.

The existing test checks an `UPDATE` fails (`crates/identity-api/src/main.rs:252-258`); it does not test `DELETE`, `TRUNCATE`, or application-role privileges. The events table also has independent foreign keys for `session_id` and `user_id`, rather than a composite foreign key proving that the event user owns the referenced session (`migration:65-79`). Application code currently supplies matching values, but the database does not enforce that attribution invariant.

**Acceptance impact:** Append-only audit is **PARTIAL**, not established as a database invariant. Add a truncate guard and/or least-privilege grants under a runtime role that cannot alter the trigger/table; constrain `(session_id,user_id)` together; then test with that role.

### High — Migration is not compatible with a mixed old/new Identity API deployment

The new Identity API applies migrations at startup (`crates/identity-api/src/main.rs:80-89`). The migration revokes every existing unbound refresh token and adds `CHECK (session_id IS NOT NULL OR revoked_at IS NOT NULL)` (`migrations/20261114000001_identity_sessions_and_tokens.sql:32-40`). The pre-I1 Identity API does not write `session_id`/`token_family_id`, so after this migration its refresh-token inserts fail the new check. Meanwhile the new Journal middleware requires `jti`, `sid`, `srv`, and a matching database allowlist entry; pre-I1 access tokens lack those fields and are rejected (`crates/identity-token/src/lib.rs:134-159`; `crates/journal-api/src/main.rs:198-218`).

This is a forward-only migration with an unavoidable reauthentication event and no mixed-version compatibility window. It can be applied as a coordinated cutover, but it is not safe as an ordinary rolling deployment. The I1 report correctly says no migration was run and production rollout is not approved, but it does not state the necessary service ordering/downtime contract.

**Acceptance impact:** Migration data handling is **PASS** for preservation/no promotion, but deployment compatibility is **FAIL** until a coordinated cutover procedure is approved and rehearsed. Keep old Identity instances out of service before applying the migration; deploy the new issuer and session-aware Journal verifier as a coordinated release; plan forced reauthentication; verify rollback behavior without attempting a down migration.

### Medium — Refresh/logout and refresh/revocation races are reasoned about but not demonstrated

Refresh locks the selected refresh row, then the session (`service.rs:328-390`), and performs token replacement, session expiry update, access-token insertion, and audit insertion in one transaction (`service.rs:435-468`). Session revocation updates session state and then all refresh/access tokens (`service.rs:579-597`). A refresh that observes a committed revoked session returns unauthorized; transaction rollback prevents partially issued tokens on the handled insertion errors.

The only concurrency test, `concurrent_refresh_replay_has_one_rotation_and_revokes_the_family` (`identity-api/src/main.rs:294-317`), races two uses of the same refresh token. It asserts one 200, one 401, and a revoked session. It does not race refresh with logout, explicit access-token revocation, or an account suspension, nor inject failures after intermediate writes. Logout first validates in one transaction and revokes in another (`service.rs:483-514`), leaving a race window. Lock ordering differs: refresh locks a refresh row before the session, while session revocation locks the session before bulk-updating refresh rows. A concurrent refresh/logout can therefore deadlock; PostgreSQL will abort a transaction, but the HTTP outcome and retry policy are not tested.

**Acceptance impact:** “Concurrent refresh/revoke behavior has a deterministic safe outcome” is **PARTIAL**. Add guarded tests for refresh/logout and refresh/session-revocation races, including deadlock/transaction-abort handling and proof that no token remains usable after a successful revocation.

### Medium — Several JWT rejection paths are established by code, not negative tests

`JwtKeyRing::verify` pins RS256, requires a configured `kid`, selects only from the configured map, validates signature/issuer/audience/expiry, and checks access-token type plus required identity/session claims (`crates/identity-token/src/lib.rs:134-159`). Expiry uses zero leeway (`:209-219`). Startup validates exactly one active private key and its paired public key (`:72-121`, `:178-205`). Unknown keys cannot select a key outside the map, and key-file entries reject duplicate IDs (`:234-255`). On Unix, private key mode bits must exclude group/other access (`:258-264`); non-Unix builds return `true` and leave ACL enforcement to deployment policy (`:266-270`).

Existing tests are `active_key_id_is_signed_and_old_key_remains_valid_during_overlap`, `expired_token_is_rejected`, `rotation_rollback_restores_old_signer_while_trusting_both_kids`, `retirement_removes_trust_for_old_key_id`, and `invalid_key_material_duplicate_ids_and_unknown_kid_fail_closed` (`identity-token/src/lib.rs:330-418`). They demonstrate valid overlap, expiry, rollback with distinct test keypairs, retirement, and startup mismatch rejection. Despite its name, the last test does not construct a token with an unknown `kid`; it tests invalid PEM, a private/public mismatch at startup, a malformed key ID, and a missing file. No test directly exercises a bad token signature under a trusted `kid`, wrong issuer/audience/type, `from_files` permission rejection, or process startup exit on invalid configuration.

**Acceptance impact:** Core key selection and validation are **PASS by code inspection**; the named rejection cases are **PARTIAL by test evidence**. Add explicit negative tests for those paths, particularly untrusted `kid`, tampered signature, issuer/audience/type mismatch, and private-key permission failure. There is no dynamic key reload: emergency removal takes effect only after a new configured ring is loaded (service restart), consistent with the report’s deployment limitation.

## JWT key-ring trace

| Path | Finding |
|---|---|
| Configuration loading | Identity API requires active `kid`, signing file list, verification file list, issuer and audience, then loads the ring before connecting/migrating (`identity-api/src/main.rs:38-54`). Journal loads a public-only ring from configured files before database connection (`journal-api/src/main.rs:230-238`, `journal-api/src/auth.rs:22-50`). |
| Active signing and emitted `kid` | Exactly one private key is accepted and its ID must equal `active_kid`; `issue` uses that key and emits RS256 plus that `kid` (`identity-token/src/lib.rs:72-131`). |
| Verification selection | The untrusted header `kid` is only a map lookup; unknown/missing IDs fail. No URL/JWKS/header key retrieval is implemented (`:134-147`). |
| Algorithm and claims | RS256 is pinned; signature, `iss`, `aud`, `exp`, `token_type`, nonempty subject/jti/session ID and positive revision are checked (`:134-159`, `:209-219`). |
| Pair check and startup | The active signer’s probe token must verify under the active public key (`:178-205`); main exits on invalid ring (`identity-api/src/main.rs:44-54`). |
| Permissions | Unix private-file group/other mode bits are checked. The check is not tested and does not validate owner, parent-directory permissions or ACLs. Non-Unix code does not inspect ACLs. |
| Rotation, retirement, rollback | Overlap and rollback with different fixture RSA pairs and old-key retirement are unit-tested (`identity-token/src/lib.rs:330-392`). Emergency removal is code/config behavior, not an operational exercise. The issuer has one active private key at a time; rollback requires restoring the old private key and trusting both public keys during the overlap window. |

No code path accepts a key solely because a token names it. A key remains trusted only while explicitly present in that process’s configured verification map; stale configuration on an un-restarted service remains a deployment risk.

## Session enforcement trace

- **Registration/login:** `service::register` and `service::login` verify/create the identity, then call `issue_pair` (`identity-api/src/service.rs:245-317`). `issue_pair` rechecks `is_active` under a user row lock, generates 32 random bytes for a 64-character lowercase-hex session ID, and inserts session, refresh hash, access-token allowlist row and `SESSION_ISSUED` event in one transaction (`:165-225`).
- **Access tokens:** `access_claims` creates a random UUID `jti` and binds user, session ID and revision (`:21-43`). `issue_access_in_tx` inserts the same values into `identity_access_tokens` before transaction commit (`:171-190`). The random session ID is opaque and satisfies the migration’s 64-hex constraint.
- **Refresh:** The submitted opaque refresh token is SHA-256 hashed; the matching row is locked. Session status/expiry, current user activity, and token non-revocation are checked before rotation (`:320-468`).
- **Logout/current-token revocation:** Logout verifies the bearer and current database session, then revokes the session, refresh tokens, access tokens, increments revision and appends an event (`:483-515`, `:579-597`). The current-token endpoint revokes the matching `jti` and appends an event (`:517-546`).
- **Expiry/inactive user:** `validate_access_token` joins user/session/token, locks session and token, checks exact ID/user/revisions and expiry, and lazily marks expired or inactive sessions revoked while revoking their tokens (`persistence/src/repos/identity_sessions.rs:7-74`).
- **Journal coverage:** Every registered route is added before `.route_layer(session_guard)` (`journal-api/src/main.rs:59-195`). The guard verifies the JWT and database state for all `/api/` paths; database failure returns 503, invalid session returns 401 (`:198-219`). `/healthz` and `/readyz` pass through without auth. Identity register/login/refresh are intentionally unauthenticated entry points; `/auth/me`, logout and access-token revoke explicitly verify current session state. No registered Journal application route bypasses the route layer.
- **Authorization separation:** `AccessClaims` contains no tenant, role, platform grant, resource permission, or step-up claim. Journal handlers still perform their existing authorization checks after the session guard. Session validity does not itself grant those permissions.

## Migration assessment

`20261114000001_identity_sessions_and_tokens.sql` is a new forward-only migration; no historical migration is edited. Under the workspace SQLx `Migrator`, it is run as a migration transaction (there are no explicit no-transaction directives or nontransactional index operations). The I1 report records the guarded 19-test migration suite passing, including `i1_upgrade_revokes_legacy_refresh_tokens_without_promoting_sessions` and migration-history/checksum tests. This audit did not apply or rerun it.

- Existing identity/user rows and refresh-token rows are retained. Legacy refresh rows receive `revoked_at` and `last_used_at`; no session is created. New checks require either a session binding or revocation. Existing users must sign in again after cutover.
- New session/token foreign keys use `RESTRICT`; session IDs, revisions, statuses, token times, and revocation fields have checks. Access and refresh tokens bind `(session_id,user_id)` to the same session. Event user/session correspondence is not composite-enforced, as noted above.
- The full-table refresh-token update, new constraints, and indexes take locks and may block traffic while the migration runs. Table size and lock duration were not measured. The migration should be rehearsed against a preserved clone, with a backup and a defined maintenance/cutover window.
- New binaries auto-run the migration before serving. Before migration, an old binary can continue its old contract; after it, an old Identity binary cannot create unbound refresh tokens, while a new Journal binary rejects old unbound JWTs. A mixed-version rolling state is unsupported. The “before and after” path is safe only as a coordinated version transition, not as transparent backward compatibility.
- There is no down migration. Database rollback means restoring a verified pre-migration backup or keeping the additive schema and rolling back application binaries only if doing so does not resume old writes that violate the new constraints.

## Acceptance evidence matrix

| I1 criterion | Result | Evidence and limit |
|---|---|---|
| Active, correctly signed token tied to an active session validates | PASS | Reported SQLx test `session_issue_refresh_replay_revokes_and_logout_invalidates_access`; source inserts session/access state and checks validation. |
| Expired JWT rejected with zero clock leeway | PASS | `expired_token_is_rejected`; code sets `Validation.leeway = 0`. |
| Expired session/access-token row rejected | PASS | Reported `access_token_requires_active_user_session_revision_and_unrevoked_allowlist_row` and `expired_access_token_allowlist_and_revoked_session_are_rejected`. |
| Revoked/stale-revision token rejected despite a signed JWT | PASS | Persistence test checks stale session revision, revoked access row, and revoked session. Journal test also marks a session revoked and expects 401. |
| Inactive user rejected | PASS | Persistence test seeds inactive user and expects false plus `REVOKED`. **Suspension followed by reactivation without a request remains FAIL** (High finding above). |
| Registration/login issues session-bound access and hashed refresh token | PASS | Implementation inserts all records transactionally; reported identity SQLx tests exercise registration. No separate login-flow assertion was found in the I1 SQLx test. |
| Refresh rotates transactionally | PASS | `session_issue_refresh_replay_revokes_and_logout_invalidates_access` checks old token rotation and new access validity. No fault injection at each transaction write. |
| Reuse/replay revokes the session/family | PASS | Same test checks reuse returns 401 and the rotated access token becomes invalid; concurrency test checks one 200/one 401 and session `REVOKED`. Family rows are not individually asserted. |
| Logout and explicit access-token revoke invalidate credentials | PASS | Identity SQLx test invokes both handlers and validates token rejection afterward. |
| Concurrent refresh/revoke outcomes deterministic | PARTIAL | Duplicate-token refresh race is tested. Refresh/logout, refresh/access-token revoke, suspension, deadlock and injected failure races are not tested. |
| Session IDs opaque and revisions enforced | PASS | 32 CSPRNG bytes encoded as 64 lowercase hex; persistence compares JWT revision to both session and token row. Tests cover stale revision. |
| No platform permissions are inferred from JWT | PASS | `AccessClaims` has no permission fields; authorization continues in Journal handlers. |
| Unknown `kid`, bad signature, wrong issuer/audience/type reject | PARTIAL | Direct code path checks are present, but no explicit negative tests for these cases were found. |
| Private/public mismatch fails startup | PASS | `invalid_key_material_duplicate_ids_and_unknown_kid_fail_closed` constructs a mismatched pair; startup invokes the same pair probe and exits on error. Process-exit behavior itself is not tested. |
| Private-key file ACL/mode check | PARTIAL | Unix code checks group/other mode bits; no file-loader test. Non-Unix is delegated to deployment ACLs. |
| Key overlap, retirement and rollback | PASS | Unit tests use distinct RSA fixture keypairs; tests run in-process, not as a service deployment/rotation exercise. |
| Emergency key removal behavior | PARTIAL | A ring without the old key rejects its token; operational removal/restart and incident response are documented, not exercised. |
| Append-only session events | PARTIAL | Update is tested as rejected; delete/truncate and role-based enforcement are not. |
| Legacy users are not promoted; legacy refresh tokens invalidated | PASS | Reported migration test confirms rows remain, refresh is revoked and no session is created. |
| Migration before/after application operation | PARTIAL | Fresh/upgrade tests are reported; they do not demonstrate old/new API mixed-version behavior. Source shows the mixed-version incompatibility. |
| Staging/production unchanged | PASS by audit action | No DB-capable command or connection was made during this review. |

## Repository preservation

Current repository: `/Users/olaoluwatunmise/event-driven-trading-rs`; branch `main`; HEAD `f6446dd33dc74a86ede9064f35aef2ec082a1c59`.

Current protected and comparison hashes:

| File | Current SHA-256 | Comparison |
|---|---|---|
| `.github/workflows/ci.yml` | `7eb6c98d3518e05e97497f2e8ec1163d6ca90488556af870098f3cd2cbcbd1f6` | Matches I1 preflight capture. |
| `migrations/20261106000002_identity.sql` | `98a38ce0388afa200579ac7db826295de18e1d12e9c45375c9bc6dd87590f76e` | Matches R2 protected hash. |
| `migrations/20261112000001_organization_tenant_foundation.sql` | `a8bf4f8c7b4a979c7aaee777a025ea6522112b4fbedc591c25bed28f6863e39b` | Matches R2 protected hash. |
| `migrations/20261113000001_broker_account_registry.sql` | `ee76686a784a3b62d0ae6743a78549ee4c4bbe9a548820115efd59f03c24b2be` | Matches R2 protected hash. |
| `crates/intent-bridge/src/mt5_runtime.rs` | `6799d6d47f5b497366aeb90ba945658e9b741138459e0ed42571120f0eff5ccc` | Matches R2 protected hash. |
| `crates/intent-bridge/src/mt5_transport.rs` | `a3343826b93724c0e0fc1de6722e2fc0b9b5d231b29557c8562e4b369d0a2ea0` | Matches R2 protected hash. |
| `Cargo.lock` staged index | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` | Matches recorded staged hash. |
| `Cargo.lock` worktree | `2dce46e257579b3f22728f582f08cfa61f5fd8f4922ce032601c4c2aa90f6d3e` | Differs from R2 pre-I1 hash `549d67…`; the worktree lock changed during authorized I1 dependency/workspace work. It remains unstaged. |
| `crates/identity-api/Cargo.toml` | `a4794cf52b2ebb70c7a4e86cea4cdbc7ffdbfff7f44a0f2f2cd4767df43683ed` | Expected I1 change from recorded pre-I1 hash `8d18f395…`. |
| `crates/identity-api/src/main.rs` | `484d5e97f1e0a7ca82f954db7c3cfc3a6a79c4ef39805ec89b4f6e359440712b` | Expected I1 change from recorded pre-I1 hash `0e49b722…`. |
| `crates/identity-api/src/service.rs` | `0c92987ed7e9b1615e5cc2ac4b16e09c96c908630f133a067406a5f399227fdc` | Expected I1 change from recorded pre-I1 hash `adfbb10c…`. |
| `crates/journal-api/src/main.rs` | `bc71e172607da5c3d3f5075f0bdd42610420ec4ea73f060f641c82bd36ea087d` | Expected I1 session-middleware/test change from recorded pre-I1 hash `4e54ee0c…`. |

Complete non-ignored `git status --short --untracked-files=all` before creating this requested audit report (96 entries):

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
 D crates/venue-connectors/tests/failure_injection.rs
 M crates/venue-connectors/tests/e2e_pipeline.rs
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
?? docs/QOS-020_TEST_INFRASTRUCTURE.md
?? docs/QOS-021A-E1-R1_GENERIC_VENUE_EXECUTION_BOUNDARY.md
?? docs/QOS-021A-E1_EXECUTION_AUTHORIZATION_FOUNDATION.md
?? docs/QOS-021A-E2_ACCOUNT_ENVIRONMENT_AND_MIGRATION_FOUNDATION.md
?? docs/QOS-021A-E3A-R1_ARCHITECTURE_DECISION_RESOLUTION.md
?? docs/QOS-021A-E3A-R2-F_VALIDATION_AND_COMPLETION.md
?? docs/QOS-021A-E3A-R2_ORGANIZATION_TENANT_FOUNDATION.md
?? docs/QOS-021A-E3A-R3A_BROKER_VERIFICATION_AUTHORITY_AND_EVIDENCE_ARCHITECTURE.md
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
?? scripts/qos020/docker-compose.qos020.yml
?? scripts/qos020/init/10-qos020-init.sql
?? scripts/qos020/provision.py
?? scripts/qos020/qos020_guard.py
?? scripts/qos020/test_guard.py
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

After creating this requested report, current non-ignored status has 97 entries: the 96 recorded above plus this new untracked audit report.

Current ignored status consists of 23 paths: `.conda/`, `.pytest_cache/`, `.venv/`, `.vscode/`, `adapters/mt5-python/.pytest_cache/`, `adapters/mt5-python/mt5_adapter/__pycache__/`, `adapters/mt5-python/quant_os_mt5_readonly_adapter.egg-info/`, `adapters/mt5-python/scripts/__pycache__/`, `adapters/mt5-python/tests/__pycache__/`, `frontend/.next/`, `frontend/.open-next/`, `frontend/.wrangler/`, `frontend/next-env.d.ts`, `frontend/node_modules/`, `frontend/tsconfig.tsbuildinfo`, `migration-report.json`, `migration-state.json`, `migration/`, `scripts/__pycache__/`, `scripts/qos020/__pycache__/`, `scripts/traderos-migration/__pycache__/`, `strategies/python/trading_strategies.egg-info/`, and `target/`. Their existence was recorded; their contents were not inspected. Historical ignored-state hashes/list were not present in the I1 report, so unchanged status of each ignored artifact cannot be independently established.

Staged paths remain `Cargo.lock` and `docker-compose.yml`; Cargo.lock is also modified in the worktree. Existing unrelated staged, unstaged, deleted, untracked and ignored changes were not normalized. The audit did not invoke any mutating Git command.

## Residual limitations and required follow-up

1. Resolve the three high-severity findings: transactional session invalidation on suspension, database-enforced append-only event storage, and coordinated old/new service migration cutover.
2. Add concurrency tests for refresh/logout, refresh/access-token revocation, account suspension, and transaction failure, only through QOS-020.
3. Add direct negative JWT tests and a file-based key permission/startup test. Enforce Windows ACL policy operationally; current non-Unix code does not verify it.
4. Select and document external private-key custody. The code supports restricted file-based keys but does not integrate a KMS/HSM, JWKS, or live reload.
5. Reconcile I1’s required “current platform grant check” with the existing R3B authority workstream before any sensitive step-up route is released. Session/JWT claims do not provide that grant.

## Read-only confirmation

This audit read repository state, source, documentation and existing tests only. No project file was changed except creation of this requested audit report. The original I1 report was not modified. No DB-capable tests, migrations, database writes, Docker/service lifecycle operations, deployments, commits, pushes, or I2 work were performed.

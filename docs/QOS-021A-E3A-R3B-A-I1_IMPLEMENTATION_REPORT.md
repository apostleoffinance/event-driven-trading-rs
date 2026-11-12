# QOS-021A-E3A-R3B-A-I1 — Identity Session Foundation

**Status:** I1 implementation and acceptance gates complete. I2–I5 have not started.  
**Scope:** Server-managed identity sessions, session-bound access tokens, refresh rotation and revocation, and JWT signing-key rollover support.  
**Runtime status:** Not deployed. No staging or production migration was run.

## Implemented

- Added the shared `identity-token` crate. Access claims carry the immutable user subject, issuer, audience, token type, token ID (`jti`), opaque session ID (`sid`), and session revision (`srv`). They carry no organization, role, platform permission, or resource authorization claims.
- Added a configured RSA key ring. The issuer loads exactly one active private key and a verification-key set keyed by `kid`; the Journal API loads public verification keys only. The active private/public pair is checked at startup. Missing, invalid, unknown, or mismatched key configuration fails closed. Unix private-key files must not grant group or other access. JWT expiration validation has zero leeway.
- Added forward-only migration `20261114000001_identity_sessions_and_tokens.sql`. It adds session, access-token allowlist, and append-only session-event tables; binds refresh tokens to sessions; and revokes legacy unbound refresh tokens without creating sessions for existing users. Those users must authenticate again after the migration is deployed.
- Login and registration create an active session and persist access and refresh token state in one transaction. Refresh rotates tokens transactionally. Concurrent reuse is serialized; reuse of a consumed refresh token revokes the active session and its token family. Logout revokes the current session and its tokens. A separate endpoint revokes only the current access token.
- Persistence validates user activity, session state and expiry, session revision, token revision, token expiry, and token revocation. Expired sessions are marked expired and their tokens revoked. An inactive user’s active session is revoked when next checked.
- Journal API checks the session against PostgreSQL for every `/api/` request. Health endpoints remain available as probes. Existing platform-permission, tenant, separation-of-duties, and resource-level checks remain in place.
- Updated Identity and Journal API deployment documentation with key-file configuration and a staged rotation, overlap, retirement, emergency removal, and rollback procedure. This implementation uses configured key files; it does not implement KMS/HSM retrieval, JWKS discovery, or hot reload.

## Verification

All SQLx tests below ran through QOS-020 run `20261110-230504-ffb59dfd0ab4` against its disposable integration lane (`127.0.0.1:51462`, database `qos020_test_master`). No staging or production database was used.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check -p identity-api -p journal-api -p persistence -p identity-token --tests` | PASS |
| `cargo test -p identity-token` | PASS — 5 tests, including expiration rejection, key mismatch, overlap, retirement, and rollback with distinct test RSA keypairs |
| Guarded `cargo test -p identity-api --bin identity-api i1_tests -- --test-threads=1` | PASS — 2 SQLx tests: refresh/replay, logout, access-token revocation, append-only events, and concurrent refresh replay |
| Guarded `cargo test -p persistence --test qos021a_i1_sessions -- --test-threads=1` | PASS — 2 SQLx tests for active, stale-revision, expired, revoked, and inactive identity state |
| Guarded `cargo test -p persistence --test qos021a_e2_migrations -- --test-threads=1` | PASS — 19 migration/history tests, including legacy refresh-token revocation without session promotion |
| Guarded `cargo test -p journal-api --bin journal-api qos021a_e3a_r2_api_tests -- --test-threads=1` | PASS — 2 SQLx API tests; includes rejection after session revocation |
| QOS-020 `--verify-sqlx-cleanup` | PASS — no SQLx-generated test databases remain |

The test keypairs are fixtures only and are not deployment credentials. Rotation was tested in-process with distinct RSA keys; no production key was read or rotated.

## Acceptance gate

**I1 code and test gate: PASS.** Active tokens validate; expired, revoked, stale-revision, and inactive-user cases reject; refresh rotation/replay and concurrency are exercised; logout and explicit access-token revocation invalidate tokens; session events reject mutation; and key overlap, retirement, and rollback are tested. JWT claims do not grant platform permissions.

**Production rollout gate: NOT APPROVED by this report.** Before deploying I1, choose and document the external private-key custody mechanism, provision protected key files/ACLs for each service, rehearse key rotation and rollback in a non-production environment, back up the target database, and apply the migration only under a separately authorized rollout. The migration has not been applied to shared staging or production.

## Open dependencies and risks

- The private signing key is read from a configured file. KMS/HSM integration and organizational custody/rotation operations remain external deployment responsibilities. Non-Unix ACL policy must be enforced by the deployment environment.
- Journal API performs a database session check on every application request. Database unavailability fails closed; capacity and availability must account for this dependency.
- Rotation configuration changes require service restart. Verifiers must receive the new public key before the issuer switches `kid`; old verification keys must remain configured through the access-token lifetime and rollout overlap.
- Applying the migration revokes all unbound legacy refresh tokens, requiring existing users to sign in again. There is no down migration; rollback is an application/configuration rollback after database backup, while schema changes remain forward-only.

## Repository preservation

- Repository root: `/Users/olaoluwatunmise/event-driven-trading-rs`
- Branch and HEAD: `main`, `f6446dd33dc74a86ede9064f35aef2ec082a1c59`
- Pre-existing staged, unstaged, deleted, untracked, and ignored work was preserved. No reset, stash, clean, checkout, commit, or push was performed. The previously staged `docker-compose.yml` and staged/worktree `Cargo.lock` states were not staged or discarded.
- Previously captured hashes for unrelated CI, historical migration, and MT5 source files remain unchanged. The identity/API, persistence, and new migration files listed above were modified only for the authorized I1 scope.
- No Docker lifecycle operation, shared database operation, credential inspection, deployment, broker execution, or MT5 dispatch change was performed.

## Phase boundary

I1 is ready for review and acceptance. I2 (WebAuthn credential infrastructure), I3 (operation-bound step-up), I4 (recovery/revocation/audit extensions), and I5 (security validation and acceptance) remain unimplemented. No later phase should begin until I1 is accepted.

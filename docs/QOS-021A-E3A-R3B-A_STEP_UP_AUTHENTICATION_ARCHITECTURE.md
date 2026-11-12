
















































































































































































# QOS-021A-E3A-R3B-A — Step-up Authentication Architecture Investigation

**Status: COMPLETE — repository investigation and architecture recommendation documented; no implementation authorized or performed.**  
**Repository:** `/Users/olaoluwatunmise/event-driven-trading-rs`  
**Branch / HEAD:** `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59`  
**Investigation date:** 2026-11-12

## 1. Repository preflight and preservation

```text
$ pwd
/Users/olaoluwatunmise/event-driven-trading-rs
$ git rev-parse --show-toplevel
/Users/olaoluwatunmise/event-driven-trading-rs
$ git branch --show-current
main
$ git rev-parse HEAD
f6446dd33dc74a86ede9064f35aef2ec082a1c59
```

The root, branch and HEAD match the expected repository. The active `origin` is `https://github.com/apostleoffinance/event-driven-trading-rs.git` for fetch and push.

### Pre-existing status

The complete pre-investigation `git status --short --untracked-files=all` is recorded below. The two leading columns are Git's staged and worktree status; no existing state was changed.

```text
 M .github/workflows/ci.yml
MM Cargo.lock
 M crates/account-engine/src/simulated.rs
 M crates/domain/src/account.rs
 M crates/domain/src/ids.rs
 M crates/domain/src/lib.rs
 M crates/domain/tests/phase1_architecture.rs
 M crates/event-trading/src/execution/fill.rs
 M crates/execution-engine/src/oms.rs
 M crates/intent-bridge/src/lib.rs
 M crates/intent-bridge/src/mt5_runtime.rs
 M crates/intent-bridge/src/mt5_transport.rs
 M crates/intent-bridge/src/mt5_v1.rs
 M crates/intent-bridge/tests/mt5_transport_mtls.rs
 M crates/journal-api/Cargo.toml
 M crates/journal-api/src/dashboard.rs
 M crates/journal-api/src/main.rs
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
?? crates/journal-api/src/broker_registry.rs
?? crates/journal-api/src/organizations_api.rs
?? crates/journal-api/tests/fixtures/r2_test_private.pem
?? crates/journal-api/tests/fixtures/r2_test_public.pem
?? crates/persistence/src/repos/broker_registry.rs
?? crates/persistence/src/repos/organizations.rs
?? crates/persistence/tests/qos021a_e2_migrations.rs
?? crates/persistence/tests/qos021a_e3a_r2_concurrency.rs
?? crates/persistence/tests/qos021a_e3a_r2_tenant.rs
?? crates/persistence/tests/qos021a_e3a_r3_broker_registry.rs
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
?? docs/QOS-021A-E3A-R3B-R_QOS020_RECOVERY_INVESTIGATION.md
?? docs/QOS-021A-E3A-R3_BROKER_SERVER_ACCOUNT_REGISTRY.md
?? docs/QOS-021A-E3A_TRUSTED_IDENTITY_AND_IMMUTABLE_RISK_ARCHITECTURE.md
?? docs/QOS-021A-S_ARCHITECTURE_AND_INFRASTRUCTURE_RECOVERY.md
?? migrations/20261111000001_journal_trading_account_fk_repair.sql
?? migrations/20261111000002_account_environment_provenance.sql
?? migrations/20261111000003_execution_authorizations_and_claims.sql
?? migrations/20261112000001_organization_tenant_foundation.sql
?? migrations/20261113000001_broker_account_registry.sql
?? scripts/qos020/docker-compose.qos020.yml
?? scripts/qos020/init/10-qos020-init.sql
?? scripts/qos020/provision.py
?? scripts/qos020/qos020_guard.py
?? scripts/qos020/test_guard.py
?? scripts/traderos-migration/migrate.py.before-http-debug
?? scripts/traderos-migration/migrate.py.debug-backup
```

`git status --short --ignored` also showed ignored Conda/Python caches and environments, VS Code state, MT5 adapter caches/egg-info, frontend build/cache/node_modules/generated metadata, migration reports/state/directory, strategy egg-info and Rust `target/`. No ignored files were opened or changed.

### Protected hashes

| Path | SHA-256 at preflight |
|---|---|
| `crates/identity-api/Cargo.toml` | `8d18f3956e35eff37e8f295121404cece7c9e9cf1a451dab5791e62d73d4a9fc` |
| `crates/identity-api/src/main.rs` | `0e49b7222e62a8e76153965c129083cffbb5f96d5f43ca610b15ae860d8c645a` |
| `crates/identity-api/src/service.rs` | `adfbb10cf373323a29f634370238109bed3b96491af6734d73dfa36b6d9c98c8` |
| `migrations/20261106000002_identity.sql` | `98a38ce0388afa200579ac7db826295de18e1d12e9c45375c9bc6dd87590f76e` |
| `crates/journal-api/src/main.rs` | `4e54ee0cb78c3def454ac7e181154002421f401a8e391c7287a2cffa004a2aa8` |
| `crates/journal-api/src/organizations_api.rs` | `10454eedc3d063056f5670807a154bfeaac7cccfb4415531c072f9aef405d142` |
| `crates/journal-api/src/broker_registry.rs` | `c762c522e2c29656e1c7c31e5387660fa640bbe86ea9fcd8d287cd88129633a9` |
| R1 decisions report | `3a8d8792788d37332bbb0b090d13cf57253c6dcd9fc5381d4a6144d9e6e4c3f6` |
| R2 foundation report | `1bd31dd6bdfac6c46a5205c549a2d0daf66b040b6483af6e1d5ca6ea138df890` |
| R2-F validation report | `736f0f530e2a042dfb6e1c73fe014fa9c39628ac8bbf492449a6379664be4518` |
| R3 registry report | `9dce6962b51b045fcbd0f41f7b85345ab6986de28443f9becb08ad92bfb7c7a1` |
| R3A architecture report | `85750fa64ea2f578db5a4bff81c359089619aa2eb2daeebbf904621f19b17627` |
| R3B-R recovery report | `ba0dfa29f5ee30e2af463415df04a75f76f12744f04af29f628f8ec3506dc8b2` |

The differing staged and worktree `Cargo.lock` state, staged `docker-compose.yml`, all prior edits/deletions/untracked artifacts and ignored directories were preserved.

## 2. Current identity and authentication architecture

### Identity and primary authentication

**VERIFIED from `crates/identity-api/src/service.rs`, `main.rs`, and `migrations/20261106000002_identity.sql`:**

- The local identity store is `identity_users`: stable text ID, unique email, display name, Argon2 password hash, active flag and timestamps. Passwords are not stored in plaintext.
- `/api/v1/auth/register` creates an identity with password length validation, hashes via Argon2, persists it, then issues an access/refresh token pair. `/api/v1/auth/login` looks up the user, checks `is_active`, verifies the Argon2 password hash and issues a pair.
- There is no MFA enrollment or second-factor check in either flow.
- There is no external OIDC, SAML or hosted identity-provider integration in the identity service's routes, dependencies or configuration found in this repository.

### JWT issuance and validation

- `AccessClaims` contains `sub`, `iss`, `aud`, `exp`, `iat` and `token_type`. It contains no `amr`, `acr`, `auth_time`, MFA flag, authenticator ID or authentication event ID.
- `JwtService::issue_access` creates an RS256 JWT with the configured issuer/audience and expiration. It does not set a `kid` or embed a step-up event.
- `JwtService::authenticate` validates bearer format, RS256 signature, issuer, audience and expiration, then requires `token_type == "access"` and a nonempty subject.
- `identity-api` reads one private PEM and one public PEM from environment configuration at process start. The inspected implementation constructs one encoding and one decoding key; no key ID/JWKS/multi-key rollover or in-process key rotation protocol is implemented. Operational rotation behavior beyond changing configuration and restarting a service is **NOT VERIFIED** and should not be assumed.
- Access token default TTL is 900 seconds and is clamped to 60–3600 seconds. Refresh token default TTL is 30 days and is clamped to 1–90 days.

### Refresh and session lifecycle

- Refresh tokens are 32 random bytes encoded as hex. Only SHA-256 hashes are persisted in `identity_refresh_tokens`.
- Refresh consumes/revokes the presented refresh token and issues a replacement within one DB transaction; expired, revoked, unknown or inactive-user refreshes fail.
- Logout revokes the supplied user's matching refresh token.
- Access JWTs are stateless and have no `jti` or server-side session row. Logout does not revoke already-issued access JWTs. They remain signature/expiration valid until expiry unless an endpoint independently checks current account/user state.
- `me` checks `identity_users.is_active`, and refresh checks active state, but `JwtService::authenticate` itself does not query active user status.

### Authorization boundaries and audit

- Journal tenant authorization extracts the JWT `sub`, reads `X-Organization-Id`, and loads current R2 organization membership from persistence. Organization OWNER/ADMIN/etc. roles are tenant capabilities. The broker registry OWNER-only route uses this organization context; no current organization role grants platform verifier permission.
- R3's intended platform approval metadata is not a proof system: current migration contains actor IDs and text authority/reference fields, but no platform-grant table or authenticated step-up event.
- R2/R3 maintain append-only domain audit/event tables for membership, entitlement and broker registry state. A centralized identity security-event stream for login, password failures, refresh replay, authenticator changes or assurance changes was **NOT FOUND** in inspected code/schema.
- `identity-api` dependencies include Argon2, `jsonwebtoken`, SHA-256, random generation, SQLx and Axum. No WebAuthn/passkey, TOTP or OIDC/SAML implementation dependency was found in `identity-api` or relevant API crates. Presence of JWT or cryptographic dependencies is not MFA support.

## 3. Capability inventory

| Capability | Finding |
|---|---|
| Password primary authentication | VERIFIED: Argon2 verification and active-user check on login |
| Short-lived RS256 access JWT | VERIFIED: issuer, audience, signature and expiry validation |
| Rotating hashed refresh tokens | VERIFIED: single-use rotation, expiry, revocation and hashed-at-rest token |
| Current organization membership check | VERIFIED: tenant membership is loaded from DB per request |
| Current platform permission grants | NOT IMPLEMENTED: no platform grant model/table/API found |
| TOTP enrollment/verification/replay defense | NOT IMPLEMENTED |
| WebAuthn/passkeys and user-verification policy | NOT IMPLEMENTED |
| External IdP MFA / signed `acr` or `amr` verification | NOT IMPLEMENTED / NOT FOUND |
| Operation-bound step-up proof | NOT IMPLEMENTED |
| Single-use proof-consumption store | NOT IMPLEMENTED |
| Access token revocation/session version | NOT IMPLEMENTED in the local JWT validator |
| Identity security-event audit stream | NOT FOUND; domain events are not a substitute |
| JWT signing key rollover with `kid` | NOT IMPLEMENTED in inspected code; operations unverified |

## 4. Step-up option comparison

| Option | Assurance and phishing | Enrollment / recovery | Server verification, replay and revocation | Fit here |
|---|---|---|---|---|
| **A. WebAuthn/passkeys** | Public-key challenge-response scoped to RP ID and origin; user verification can be required. Strong phishing resistance when correctly configured. Synced passkey properties depend on authenticator/provider policy; do not overclaim hardware-backed assurance. | Requires registration ceremony, credential public-key storage, authenticator naming, replacement, recovery, and account recovery policy. Recovery must not silently bypass privileged verification. | Server validates challenge, origin, RP ID hash, credential signature, user-presence/user-verification flags and credential status. One-time unpredictable challenge prevents replay. Credential removal/revocation is server-side. | Best fit for the accepted phishing-resistant requirement and existing human identity, but requires a new implementation/dependency and browser ceremony. No existing support. |
| **B. TOTP app** | Shared secret plus short code; not phishing-resistant and susceptible to real-time relay. A one-time time-step check reduces replay but does not make it phishing-resistant. | Broad authenticator compatibility, but seed provisioning/recovery is sensitive; seed must be encrypted with managed keys. Recovery channels often become the weakest factor. | Verify HMAC-based time code, allowed skew and replay counter/last accepted step transactionally. Revoke seed and sessions. | Technically implementable, but not recommended as the privileged-operation fallback because it fails the preferred phishing-resistant control and creates seed custody/recovery burden. |
| **C. Existing external IdP MFA** | Could inherit phishing-resistant assurance if provider and policy guarantee it. A generic `amr=mfa` does not establish phishing resistance. | Depends on provider enrollment and recovery. | Must validate issuer, audience, signature, token lifetime, `acr`/`amr`, fresh `auth_time`, assurance policy and provider session revocation. | No IdP integration or trust configuration was found. This is a future alternative only after provider and claim contract are explicitly adopted. |
| **D. Separate step-up service** | Can centralize WebAuthn and issue narrow proofs; security depends on its authenticators, proof signing and channel. | Adds a separate enrollment, recovery, availability and identity-linking system. | Needs its own challenge/proof store, key rotation, revocation API and relying-party/resource binding. | Not the smallest safe initial design. It duplicates identity responsibilities without an existing service to reuse. |

## 5. Recommended architecture — native WebAuthn step-up in the identity trust domain

**Recommendation (proposed, not accepted):** implement WebAuthn/passkey authentication inside or as a tightly coupled module of the existing identity service, with explicit platform grants remaining in the separate platform-permission domain. Do not trust tenant roles, client booleans, arbitrary headers, a JWT `mfa=true` claim or proof from an external IdP that the repository does not currently trust.

Use WebAuthn requiring user presence and user verification for all sensitive platform mutations. Configure exact RP ID and allowed origins; reject origin/RP mismatch and downgrade. The server stores credential IDs, public keys, supported algorithms, counters/backup metadata where meaningful, status/revision and user association; it never stores authenticator private keys. A credential's assurance label must reflect verified ceremony and policy, not simply the word “passkey.”

At the current implementation state this is a **future architecture**, not a working capability. The local authentication foundation is sufficient to identify a stable human `sub`, but not to verify an authenticator or attest phishing resistance.

### Sensitive operations

Require fresh step-up for:

- `broker_verification.approve`
- `broker_verification.suspend`
- `broker_registry.manage`
- `broker_reassignment.approve`
- HMAC key retirement approval
- Platform authority grant/revoke/suspend

Reads, ordinary tenant dashboard access and routine organization membership changes continue using standard authenticated authorization unless another policy requires elevation. A high-impact operation remains subject to its current platform permission check as well as step-up; step-up is authentication assurance, not authorization.

## 6. Operation-bound proof model

### Ceremony

1. User first authenticates with a valid identity session. Server reads current identity active state and current platform permission; permission check occurs again on final action.
2. Client requests a challenge for a specific operation. Server accepts only an allowlisted operation and a server-resolved resource ID/revision, not a caller-supplied free-form authorization scope.
3. Server creates a cryptographically random, single-use WebAuthn challenge and stores only a protected challenge representation with: user ID, credential/user scope, operation identifier, target case/resource ID, target revision, expected authorization-grant revision, authentication-event ID, issue/expiry times, browser session binding, RP/origin policy version, and nonce digest.
4. Challenge lifetime: **2 minutes proposed**. Require user verification. The WebAuthn signed challenge bytes bind the ceremony to server-held context. User-facing prompt displays the action and target name/reference to reduce transaction-confusion risk.
5. Server validates authenticator response, signature, challenge, origin, RP ID hash, credential status, presence and verification flags, and ceremony freshness. A client response or JWT claim alone is not enough.
6. Server records an authentication event and creates an operation proof row with: identity-user ID, method (`WEBAUTHN`), assurance-policy version, credential ID, event ID, operation, target resource/case ID, resource revision, authority grant ID/revision, issue time, expiry time and unique proof nonce.
7. Proof lifetime: **5 minutes proposed** and single-use. Bind it to the existing authenticated browser/session context (server-side session or a random session binding stored/validated by the API); do not bind only to a reusable refresh token. If a server-side session binding is unavailable at implementation time, that is an additional prerequisite, not a reason to weaken binding.
8. Final sensitive operation supplies the opaque proof ID. In one database transaction, lock proof, resource/case, current platform grant and relevant binding/entitlement rows in fixed order; revalidate user active state, current permission/revision, proof expiry/session binding, exact operation/resource/revision, no prior consumption, and separation-of-duties rules; consume proof and write decision/audit/outbox. A failed action rolls back proof consumption if the operation is rejected before committing; alternatively, an explicit attempted/consumed outcome must be recorded atomically to prevent retries from changing semantics.

Use one proof per operation invocation. A different operation, target or revision requires a new challenge and fresh authenticator ceremony. Consuming proof uses a row lock plus `consumed_at IS NULL` compare-and-set and unique event/operation idempotency. Two concurrent requests: at most one commits; the other gets a replay/conflict response and cannot execute its action. A consumed proof is never reset.

The proof is not a bearer permission by itself. It is a short-lived server record accepted only with the same authenticated user and session. It cannot replace platform grants or independently authorize a resource.

## 7. Session, revocation, recovery and audit

- Access JWT lifetime remains bounded by the current configured TTL; step-up proof is substantially shorter and stored server-side. If a verifier credential or platform grant is revoked, final action queries current DB status and rejects immediately even if the JWT remains valid.
- WebAuthn challenge is one-use and expires after 2 minutes; operation proof expires after 5 minutes. Both lifetimes are proposed defaults pending security review and configurable only within narrow validated bounds.
- Revoke individual credentials by status/revision; record actor, reason and timestamp. Credential replacement requires step-up with an existing credential or an independently reviewed recovery process.
- Recovery codes alone must not satisfy step-up for privileged approval, grant, transfer or key retirement. If all authenticators are lost, require separately approved identity recovery with a cooling-off/notification process; do not make email-only recovery equivalent to WebAuthn.
- Suspend identity: block new login, step-up, refresh and sensitive operations. Existing access JWTs may remain cryptographically valid until expiry, so sensitive routes must check current user/credential/grant state online. A future user/session revision can provide broader revocation, but none exists now.
- Signing-key compromise: rotate issuer keys under a defined `kid`/overlap policy, revoke refresh sessions, invalidate outstanding step-up challenges/proofs, and assess forged token interval. Existing single-key validation has no verified overlap/rotation process; add key lifecycle separately and test rollback. Do not claim simply changing PEM revokes JWTs accepted by other still-running instances.
- Audit identity login/failure, step-up challenge issuance/expiry, ceremony success/failure, credential enrollment/revocation/recovery, proof consumption/replay, permission changes, operation decision and denied authorization. Store actor ID, event/credential references, operation/resource/revision, assurance policy version, server timestamps, correlation/idempotency IDs and redacted reason. Never store assertion secrets, private keys, TOTP seeds or raw challenge secrets in logs. Evidence and authority event histories remain append-only with service-role restrictions and independent backups.
- If assurance cannot be established, is stale, revoked, mismatched or database state cannot be checked, deny the sensitive operation. Do not fall back silently to password-only, TOTP, email, arbitrary `amr`, or a long-lived JWT claim.

## 8. Proposed API and data model

These are design proposals only; no routes or tables were created.

### API sketches

- `POST /api/v1/auth/webauthn/registration/options` and `/registration/complete` — identity enrollment, requiring current authenticated identity and appropriate verification/recovery policy.
- `POST /api/v1/auth/step-up/options` — allowlisted operation and resource reference; server resolves revisions/grant context and returns WebAuthn options plus opaque challenge ID.
- `POST /api/v1/auth/step-up/complete` — signed assertion and challenge ID; server validates challenge and persists operation-bound proof.
- Sensitive platform API receives `step_up_proof_id` as an opaque reference or dedicated proof header. Server verifies session/user and consumes the proof transactionally; the header itself has no authority.
- `POST /api/v1/platform/authenticators/{credential_id}/revoke` — current platform/user policy with reauthentication and audited actor.

Do not include account identifier or sensitive evidence contents in WebAuthn challenge text, URLs, telemetry or browser logs. Display only a redacted case/broker label.

### Additive persistence concepts

- `identity_webauthn_credentials`: user FK, credential ID, public key, COSE algorithm, status, sign-counter/backup metadata, credential revision, created/last-used/revoked timestamps; unique credential ID and restrictive deletion.
- `identity_authentication_events`: user, method, credential, outcome, event time, session reference, policy version, redacted client metadata and correlation ID.
- `step_up_challenges`: challenge digest, user/session, operation, target resource/case, resource and grant revisions, RP/origin policy version, issue/expiry/consumption, attempt state and nonce uniqueness.
- `step_up_proofs`: authentication event, user/session, method/assurance policy, exact operation/resource/revisions, issue/expiry, proof nonce digest, consumed timestamp and outcome. Unique proof nonce and idempotency key.
- `identity_security_events`: append-only event taxonomy for failed/successful primary auth, refresh replay/revocation, authenticator lifecycle, step-up, key lifecycle and suspicious reuse.

All migrations would be forward-only. Use `ON DELETE RESTRICT` for audit-linked identity records; credential revocation and identity closure are state transitions. Database constraints and row locks enforce one-use challenge/proof semantics. Do not store private authenticator keys or use plain text challenge/proof as database bearer secrets.

## 9. Threat model

| Threat | Impact | Proposed controls | Residual risk / required test |
|---|---|---|---|
| Stolen access JWT | Attacker uses ordinary authenticated endpoints | Short access TTL; sensitive routes require fresh WebAuthn proof; current user/grant lookup | Attacker with victim browser session may invoke challenge; test JWT-only denial |
| Stolen refresh token | Attacker renews access session | Existing single-use rotating hashed refresh token; revoke family/session on replay in future | Current replay response behavior must be reviewed; test stolen/reused refresh and session revocation |
| Phishing | Password or code relayed to attacker | WebAuthn RP/origin-bound assertion with UV required | User may approve a confusing transaction; test operation display and origin mismatch |
| TOTP interception/replay | Attacker relays valid code | Do not use TOTP as sensitive-action fallback; if separately adopted, enforce one accepted time-step | TOTP remains phishable; negative test must show no privileged fallback |
| Passkey/session compromise | Attacker controls authenticated browser/device | Operation-bound challenge, short expiry, current grant check, session binding, UV | Malware can act through live session; test stolen session without fresh credential |
| Cross-operation proof reuse | Proof for approve used for key retirement | Bind operation string in DB and proof; compare exact operation at consume | Handler mapping errors; cross-operation matrix test |
| Cross-resource proof reuse | Proof for case A used on case B | Bind resource/case ID and revision | Resource aliasing bugs; same-proof/different-resource test |
| Concurrent proof consumption | Duplicate sensitive action | Transactional row lock/CAS, unique consumption, single use | Deadlocks/serialization; real PostgreSQL race test |
| Recovery-channel compromise | Attacker replaces passkey | No email-only privileged recovery; delay, notify, independent review; invalidate proofs | Human review can be socially engineered; recovery drill and notification tests |
| Privileged operator misuse | Improper grant/approval/retirement | Least privilege, step-up, separation of duties, append-only events, two-person high-risk actions | Collusion/DB owner risk; audit alert and access review |
| Revoked authenticator reuse | Old credential creates proof | Current credential status checked in final transaction; revision increment | Cache/stale replica risk; revoke-then-use test |
| IdP/signing-key compromise | Forged identity tokens or proof | Locally owned keys, `kid` overlap/rotation, emergency revoke, short tokens, current user/grant check | No current key rollover protocol; compromise and rollback exercise |

## 10. Migration and test implications

No database operation was performed. Proposed migrations must be additive and tested in the QOS-020 isolated PostgreSQL cluster after separate implementation authorization. Existing R2/R3 rows and pending verification state must not be auto-promoted. Test fresh install and upgrade from supported R2/R3 history.

Required tests before step-up can be called implemented:

1. WebAuthn registration/assertion: valid credential, wrong challenge, wrong RP ID/origin, invalid signature, missing user-verification flag, unknown/revoked credential, expired/future challenge, malformed/unsupported algorithm.
2. Operation binding: same challenge cannot authorize a changed operation, target, account/binding revision or grant revision.
3. Replay/races: nonce reuse, expired challenge, duplicate proof consumption, two concurrent proof consumers, proof/session mismatch, stale grant and revoked credential.
4. Separation of duties: initiator and evidence submitter cannot approve; platform permission checked separately from organization OWNER/ADMIN role; grant/revocation races.
5. Recovery and revocation: replacement, lost authenticators, user suspension, recovery notifications and old-session rejection for sensitive operations.
6. Token behavior: stale JWT plus revoked permission is denied; refresh rotation/replay; signing-key rollover and compromise behavior once designed.
7. Audit/privacy: events append-only, redacted logs contain no challenges/assertions/credentials, proof decisions include exact operation/resource/revisions.
8. Migration: QOS-020-guarded fresh install and upgrade; constraints and cleanup; no staging/production access.

Unit cryptography tests alone are insufficient. Challenge/proof consumption and authority races need actual PostgreSQL integration concurrency tests. Browser WebAuthn ceremony needs browser integration tests with a test authenticator; real user enrollment and deployment are separate operational gates.

## 11. Implementation sequence and open decisions

### Recommended sequence

1. Decide WebAuthn policy: supported authenticator classes, user verification requirement, allowed origins/RP ID, synced passkey acceptance, verifier enrollment eligibility and recovery policy.
2. Decide identity session binding and access-token revocation/key rollover design. Add current user-active checks for sensitive operations.
3. Implement WebAuthn credential lifecycle in identity service with replay-safe challenges, audit and step-up ceremony. Add forward-only migrations.
4. Implement operation-context challenge creation and proof issuance using server-resolved platform permission/resource revisions.
5. Integrate transactional proof consumption into each sensitive platform endpoint; recheck current platform grant, active identity, separation-of-duties and resource revisions.
6. Add recovery/revocation workflows and admin monitoring; no email or TOTP fallback for sensitive approvals unless separately risk-accepted.
7. Run guarded QOS-020 migrations/tests, browser authenticator tests, security review and recovery drill; verify no R2/R3 or MT5 execution boundary changes.

### Decisions still requiring approval

- Whether synced passkeys meet the required verifier assurance or whether hardware-bound authenticators are required.
- Whether privileged approvers must enroll two independent credentials.
- Exact challenge/proof expiry bounds and user reauthentication UX.
- Recovery adjudication authority, cooling-off period and user notification requirements.
- RP ID and production origin ownership, browser support and environment separation.
- Access JWT revocation mechanism and signing-key rotation/KID/JWKS contract.
- Audit-event retention and privacy jurisdiction.
- Whether step-up is re-required for every individual sensitive mutation (recommended) or can be grouped within a short, exact-operation session (not recommended for approvals).

## 12. Final confirmation

**COMPLETE for read-only investigation.** Existing behavior and missing capabilities were traced in source, migration and dependency configuration. No MFA/step-up implementation is claimed. The WebAuthn design is a recommendation only.

No repository source, migration, authentication configuration, database schema/state, identity-provider setting, user, credential, authenticator enrollment, secret, session, Docker resource or external service was changed. No tests/builds, database operations, deployment, commit or push occurred. The only repository change is this requested report.

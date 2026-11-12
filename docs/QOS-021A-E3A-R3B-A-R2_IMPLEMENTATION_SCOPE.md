# QOS-021A-E3A-R3B-A-R2 — Step-up Authentication Implementation Scope

**Status: SCOPE APPROVED — documentation only in this phase; implementation remains separately gated by R3B authorization.**  
**Governing decisions:** `docs/QOS-021A-E3A-R3B-A-R1_ARCHITECTURE_DECISION_RESOLUTION.md`  
**Repository baseline:** `/Users/olaoluwatunmise/event-driven-trading-rs`, `main`, `f6446dd33dc74a86ede9064f35aef2ec082a1c59`.

This document defines five proposed implementation phases. It does not authorize their execution. Each phase must preserve platform-permission checks, separation of duties, tenant/resource authorization, QOS-020 isolation and MT5 execution deny-all behavior.

## Invariants applying to every phase

- WebAuthn/passkey proof is authentication assurance; it is not a platform permission or resource grant.
- Check the current database-backed platform grant on each sensitive mutation. Organization OWNER/ADMIN roles never imply platform permission.
- Verify user, current server-managed session, resource, grant and relevant state revisions at action time.
- Require user verification and an exact operation/resource binding. No client boolean, JWT role, MFA claim or arbitrary header substitutes for server-verified proof.
- Sensitive proofs have a two-minute challenge lifetime and five-minute maximum proof lifetime; proof is single-use.
- Consume proof transactionally with the protected mutation and its append-only audit event. Concurrent replay yields at most one successful commit.
- Require fresh step-up for every sensitive mutation. Do not create reusable elevated sessions.
- Recovery requires an independent process, a minimum 24-hour cooling-off period and notifications. No TOTP or email-only privileged fallback.
- Migrations are additive and forward-only; no legacy identity is auto-promoted.
- Database tests/migrations run only under the verified QOS-020 guard. Never use staging/production.
- No broker execution capability, MT5 dispatch change, deployment, live operation, commit or push.

## R3B-A-I1 — Identity session foundation

### Scope

- Add server-managed session records with opaque stable session IDs, status, revision, issue/expiry/last-use/revocation metadata and identity-user FK.
- Bind access JWTs to session ID and session revision. Define token version/claims without treating claims as current authorization.
- Add online session and user-active-state validation for sensitive routes; implement explicit session/access-token revocation.
- Integrate existing hashed, rotating refresh tokens with session IDs and revision; define refresh-family replay behavior, logout, user suspension and global/session-specific revoke.
- Define current platform grant checks against database state for protected requests, separate from session assurance.
- Resolve the required JWT signing-key rollover contract (`kid`/JWKS or equivalent): provisioning, overlapping validation, rotation, rollback and incident response. The current single configured signing/verification key does not implement rollover.

### Dependencies

- Accepted ADR-6 and ADR-1.
- JWT issuer/verifier coordination and access to current identity/session state.
- Key custody and `kid`/JWKS (or equivalent) decision before completion; no key material in source.

### Acceptance gate

- Active session + valid JWT authenticates; revoked/expired session or inactive user is rejected for sensitive operations despite an otherwise unexpired JWT.
- Refresh rotates transactionally within the same session; logout/revocation invalidates session and pending proofs.
- Stale session revisions fail; concurrent refresh/revoke behavior has a deterministic safe outcome.
- Signing-key rollover/rollback is specified and tested before this phase is accepted as complete.
- No platform permission is inferred from session or JWT claims.

### Tests

- Unit token claim/issuer/audience/signature/expiry tests.
- PostgreSQL session creation, revision, expiry and revocation tests under QOS-020.
- Refresh rotation, reuse/replay, logout, user deactivation and revoke races.
- Old-token/new-key overlap and retired-key rejection tests for the accepted rollover design.
- API test with still-valid JWT but revoked session/grant.

## R3B-A-I2 — WebAuthn credential infrastructure

### Scope

- Implement WebAuthn registration and assertion validation in the existing identity trust domain or a narrowly coupled component.
- Store credential IDs, public keys, algorithm and policy metadata, status/revision and timestamps; never store authenticator private keys.
- Require user presence and mandatory user verification for sensitive step-up; accept synced passkeys and support optional hardware-bound authenticators without mislabeling assurance.
- Support two registered credentials for privileged approvers where feasible.
- Implement challenge creation/consumption, origin/RP ID validation, signature verification, supported algorithm policy, credential revocation and security notifications.
- Add authentication-event and append-only authenticator lifecycle auditing.

### Dependencies

- I1 sessions, session binding and key rollover design.
- Approved RP ID, origin allowlist, browser/environment configuration, authenticator metadata policy and WebAuthn library selection/security review.
- Recovery policy defined in I4 before privileged enrollment/replacement is released.

### Acceptance gate

- Correct assertion validates; wrong signature, challenge, origin, RP ID, algorithm, credential status, expiry or missing UV fails closed.
- Credential enrollment and revocation are revisioned/audited; revoked credential cannot authenticate even with an otherwise valid token.
- Synced and hardware-bound metadata are represented accurately; no unsupported attestation claim is made.
- Credential secrets are absent from logs and database; public keys/credential metadata access is restricted.

### Tests

- WebAuthn ceremony tests with test-only authenticator fixtures and browser integration.
- Registration/assertion success and malformed/unsupported data rejection.
- Challenge expiry/replay, origin/RP mismatch, UV=false and credential revocation.
- Two credentials, one credential lost/revoked, replacement and concurrent revocation tests.
- Privacy/log redaction and append-only audit tests.

## R3B-A-I3 — Operation-bound step-up

### Scope

- Add allowlisted step-up challenge APIs and server-side challenge/proof records.
- Bind proof to identity user, auth method/policy, auth event, credential, session ID/revision, exact operation, target case/resource ID, target revision, platform permission/grant revision, issue/expiry and unique nonce.
- Enforce a two-minute challenge TTL and five-minute maximum proof TTL.
- Require a new ceremony for every sensitive mutation; prohibit general-purpose elevation.
- Recheck current identity, session, platform grant, separation of duties, tenant/resource entitlement, evidence freshness and revisions at final action.
- Atomically consume proof with approval/suspension/catalogue/transfer/key-retirement/authority mutation and append-only audit event.

### Dependencies

- I1 session foundation and I2 verified WebAuthn credentials.
- Platform grants and named permissions from the R3B authority workstream.
- Stable operation names and server-resolved resource revision model for each protected endpoint.

### Acceptance gate

- JWT-only, organization OWNER/ADMIN, role claims, arbitrary headers and client MFA booleans cannot pass.
- Proof for another operation, resource, user, session, permission revision or resource revision is rejected.
- Expired, revoked, consumed, or malformed proof is rejected.
- Concurrent use of one proof yields at most one successful operation and leaves no partial audit/resource changes.
- Initiator/evidence submitter self-approval remains rejected independently of successful WebAuthn.

### Tests

- Full API authorization matrix for every sensitive operation.
- Cross-operation/resource/session/revision proof substitution tests.
- PostgreSQL proof consumption and concurrent replay tests under QOS-020.
- Stale JWT, revoked grant, changed entitlement, changed resource revision and session revocation races.
- Transaction rollback and append-only decision-event tests.

## R3B-A-I4 — Recovery, revocation and audit

### Scope

- Implement independent recovery/replacement authority, recovery request and decision history, at least 24-hour cooling-off, notifications, appeal/cancel and credential/session revocation.
- Prevent recovered credentials from authorizing sensitive operations during the cooling-off interval.
- Revoke outstanding challenges/proofs on credential/session/account revocation and relevant security events.
- Add append-only security audit for primary authentication events, session lifecycle, refresh reuse, credential lifecycle, recovery, step-up challenges/proofs, permission decisions and sensitive actions.
- Define privacy-minimized metadata, reviewer access policy and notification evidence without storing passwords, private keys, raw proof secrets or unnecessary personal data.
- Integrate security-triggered reverification with broker identity state without mutating orders/positions or bypassing reconciliation.

### Dependencies

- I1 session revocation, I2 credential state, I3 proof lifecycle.
- Independent recovery authority, notification channel, retention/legal-hold policy and incident ownership approved before enabling account recovery.

### Acceptance gate

- Recovery cannot be initiated and approved by the same person or by email-only verification.
- No recovered/replaced credential can step up before 24 hours and required notifications are recorded.
- Credential, user, session or verifier revocation immediately blocks future sensitive operations and invalidates outstanding proof.
- Audit events are append-only through application roles, contain actor/session/authority references and no secrets, and failed attempts are appropriately recorded.
- Association revocation causes no order, position or reconciliation mutation.

### Tests

- Recovery request, independent decision, cooling-off boundary, notifications and cancellation.
- Credential/session/user revocation with outstanding challenges and proofs.
- Audit immutability, actor attribution, redaction and retention policy tests.
- Recovery and revocation concurrency tests under QOS-020.
- Negative tests proving no TOTP/email privileged fallback.

## R3B-A-I5 — Security validation and acceptance

### Scope

- Conduct end-to-end security review across I1–I4, including browser ceremony, APIs, database invariants, logging, operations and recovery.
- Complete threat-model tests for replay, cross-scope substitution, stale grants, compromised/revoked credentials, token theft, session theft, recovery abuse, privilege escalation and signing-key rotation.
- Validate forward-only fresh-install and supported-upgrade migrations using the QOS-020 guard; verify SQLx cleanup.
- Review deployment secret/key handling and operational recovery/runbooks without deploying.
- Demonstrate that all sensitive actions require current platform authorization plus fresh operation-bound WebAuthn and separation-of-duties checks.

### Dependencies

- I1–I4 complete; JWT rollover mechanism resolved and exercised; supported browser/RP origins and recovery operations defined.
- QOS-020 identity verified before each database-capable command.

### Acceptance gate

- All required unit, API, WebAuthn/browser and real-PostgreSQL concurrency tests pass; failures and unsupported environments are explicitly reported.
- Fresh install and R2/R3 upgrade preserve existing rows without legacy promotion.
- QOS-020 guard passes for each DB-capable run and generated SQLx databases are cleaned.
- Security review confirms no fallback, proof replay, stale permission or self-approval path.
- E1-R1 sealed simulation boundary and MT5 `dispatch_submit` deny-all behavior remain unchanged.
- No production deployment or execution is implied by acceptance.

### Tests

- Full I1–I4 regression and security suites.
- QOS-020 guarded migration, integration and concurrency suites.
- Browser WebAuthn tests with supported test authenticators.
- Signing-key rollover, emergency revoke and rollback exercises.
- Independent security review and operational recovery tabletop.

## Cross-phase implementation order

`I1 Identity session foundation → I2 WebAuthn credential infrastructure → I3 Operation-bound step-up → I4 Recovery/revocation/audit → I5 Security validation`

I1 and the JWT rollover design are prerequisites to binding proof to revocable sessions. I2 depends on approved RP/origin policy and I1. I3 depends on current platform grants and I1/I2. I4 depends on all lifecycle records from I1–I3. I5 depends on all phases and the QOS-020 guard.

Each phase is accepted independently only when its gate passes. A failed security gate stops progression; do not weaken proof binding, user verification, current permission checks or recovery controls to make tests pass.

## Explicit exclusions and boundaries

This scope does not authorize implementation. When separately authorized, it still excludes:

- Execution authorization coordinator, risk-decision issuance/consumption, canonical-order authorization, broker sending or MT5 dispatch enablement.
- Changes to live/shadow execution, orders or positions.
- Production migration, deployment or external identity-provider changes.
- Broker credential ingestion/storage or automatic gateway failover.
- TOTP or email-only fallback for privileged platform operations.

All database tests/migrations must use QOS-020 only. Staging and production remain out of scope. Historical migrations are immutable; all schema changes are forward-only.

## Open technical dependencies

1. Define and validate JWT signing-key rollover using `kid`/JWKS or equivalent. No rollover behavior currently exists in the inspected identity service.
2. Decide RP ID, exact allowed origins, browser compatibility and authenticator policy.
3. Choose and security-review a maintained WebAuthn server library; no such library is currently used in the identity service.
4. Specify server-session storage, high availability, revocation consistency and failure behavior.
5. Establish independent recovery authority, notification delivery and audit retention/legal-hold policy.
6. Define access-token revocation and refresh-family reuse response across multiple API instances.
7. Define the platform-grant tables and current-state permission check from the R3B authority workstream before wiring privileged routes.

## Repository preservation record

Preflight confirmed the expected root, branch and HEAD. Existing staged `Cargo.lock`/`docker-compose.yml` state; unstaged changes; deleted tests; untracked R2/R3 work, reports, migrations, QOS-020 scripts, and migration backups; and ignored build/cache artifacts were preserved.

Protected SHA-256 values recorded before writing:

| Path | SHA-256 |
|---|---|
| `crates/identity-api/Cargo.toml` | `8d18f3956e35eff37e8f295121404cece7c9e9cf1a451dab5791e62d73d4a9fc` |
| `crates/identity-api/src/main.rs` | `0e49b7222e62a8e76153965c129083cffbb5f96d5f43ca610b15ae860d8c645a` |
| `crates/identity-api/src/service.rs` | `adfbb10cf373323a29f634370238109bed3b96491af6734d73dfa36b6d9c98c8` |
| `migrations/20261106000002_identity.sql` | `98a38ce0388afa200579ac7db826295de18e1d12e9c45375c9bc6dd87590f76e` |
| `crates/journal-api/src/main.rs` | `4e54ee0cb78c3def454ac7e181154002421f401a8e391c7287a2cffa004a2aa8` |
| `crates/journal-api/src/organizations_api.rs` | `10454eedc3d063056f5670807a154bfeaac7cccfb4415531c072f9aef405d142` |
| `crates/journal-api/src/broker_registry.rs` | `c762c522e2c29656e1c7c31e5387660fa640bbe86ea9fcd8d287cd88129633a9` |
| `Cargo.lock` staged index | `0114a3432961bae1989c479073e12ea84df6507c27debda84e832d950e35e0a0` |
| `Cargo.lock` worktree | `549d67b306e609fc6360a6f8d20281dcd33c17949904d510e2807d6eedd891d5` |

## Phase completion statement

Only documentation is produced in this authorization. No authentication, session, WebAuthn, database, API, test, Docker, infrastructure or external identity-provider change is authorized or performed. No database operation, build, test, migration, deployment, commit or push is permitted by this documentation phase.

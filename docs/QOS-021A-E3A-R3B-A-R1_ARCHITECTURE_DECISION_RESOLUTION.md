# QOS-021A-E3A-R3B-A-R1 — Step-up Authentication Architecture Decision Resolution

**Status: ACCEPTED — architecture decisions recorded; implementation is not authorized by this document.**  
**Repository baseline:** `/Users/olaoluwatunmise/event-driven-trading-rs`  
**Branch / HEAD:** `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59`  
**Authority:** explicit user acceptance in QOS-021A-E3A-R3B-A-R1.  
**Scope:** documentation only.

## Decision context

The identity service currently authenticates users using a password verified with Argon2, then issues a short-lived RS256 access token and a rotating, hashed refresh token. Existing access JWT claims contain the user subject and standard issuer, audience, issued-at and expiry fields; they contain no MFA or step-up proof. The local JWT validator uses one configured public key and the issuer uses one configured private key; a `kid`/JWKS rollover protocol has not been implemented.

There is no current WebAuthn, passkey, TOTP or external identity-provider MFA implementation. R3B introduces platform-level permissions separate from R2 organization membership and requires recent, stronger authentication for high-impact platform actions.

Authentication assurance and authorization remain separate controls:

- WebAuthn proves that an enrolled authenticator completed a fresh ceremony for the signed challenge under the configured RP/origin policy.
- Platform grants authorize a person to perform a named action. Current grants must be checked server-side.
- Separation of duties determines whether that authorized person may act on this particular case, such as whether they initiated the registration or submitted the evidence.
- Resource authorization checks current revisions, status, tenant, entitlement and case scope.

No passkey proof can substitute for platform permission, separation of duties or resource-level authorization.

## ADR-1 — Native WebAuthn/passkeys with mandatory user verification

**Status: ACCEPTED.**

**Decision:** Use native WebAuthn assertions for step-up on sensitive platform operations. Require authenticator user verification and user presence. Configure exact RP ID and allowed origins. Verify challenge, origin, RP ID hash, credential status, supported algorithm, signature, user-verification flags and freshness server-side.

WebAuthn is not implemented by the current repository. This decision authorizes a future implementation design, not current passkey enrollment or use.

**Consequences:** Add credential lifecycle, browser ceremony, server challenge/proof persistence, recovery, revocation and security event auditing. Do not claim a credential is hardware-backed unless the authenticator policy and evidence support that claim.

## ADR-2 — Synced passkeys are accepted; hardware-bound credentials are optional

**Status: ACCEPTED.**

**Decision:** Synced passkeys may satisfy the normal privileged step-up requirement when a valid WebAuthn assertion meets the configured user-verification policy. Hardware-bound authenticators are supported as an optional stronger choice. The UI and audit records must not mislabel synced credentials as hardware-bound.

**Consequences:** Assurance metadata records the method and policy version without inferring undisclosed authenticator properties. If a particular operation later requires hardware-backed credentials, that must be a separate explicit policy gate with a verifiable implementation.

## ADR-3 — Two registered credentials for privileged approvers where feasible

**Status: ACCEPTED.**

**Decision:** Privileged approvers should register two independent WebAuthn credentials where feasible to reduce lockout risk. Enrollment of a second credential does not weaken user verification or authorize recovery. One active credential may still perform an otherwise authorized step-up unless a future risk policy requires two authenticators for one ceremony.

**Consequences:** Provide credential inventory, naming, revoke/replace lifecycle and security notifications. Independent recovery remains necessary for users who lose all credentials. Do not imply that two credentials mean two-person approval; two-person approval requires two distinct authorized people.

## ADR-4 — Two-minute challenges and five-minute single-use operation-bound proofs

**Status: ACCEPTED.**

**Decision:** WebAuthn challenge lifetime is two minutes. A successfully verified step-up proof is valid for at most five minutes, is single-use and is bound to:

- identity-user ID;
- authentication method and assurance policy version;
- authentication event ID and credential reference;
- exact sensitive operation identifier;
- exact target case/resource ID;
- target resource revision and relevant platform-grant revision;
- authenticated session ID and session revision;
- issue and expiry times;
- unique replay-resistant nonce/proof ID.

The server creates the challenge context from an allowlisted operation and server-resolved resource state. The browser cannot widen or alter the scope. Final action consumes the proof atomically with the state transition and audit event. Concurrent replays must yield at most one successful commit.

**Consequences:** A proof for one action, resource, revision, user or session cannot authorize another. Any mismatch, expiry, revocation or inability to establish current assurance fails closed.

## ADR-5 — Fresh step-up for every sensitive mutation

**Status: ACCEPTED.**

**Decision:** Require a new WebAuthn ceremony for every sensitive platform mutation, including each verification approval, suspension, catalogue management mutation, reassignment approval, key-retirement approval, and platform authority grant/revocation. Do not reuse a proof across actions or create a general elevated session.

Ordinary authenticated reads and routine tenant membership operations do not become platform-sensitive operations merely because WebAuthn exists. Platform permission and resource checks remain independently mandatory.

## ADR-6 — Server-managed sessions and explicit access-token revocation

**Status: ACCEPTED.**

**Decision:** Introduce server-managed sessions with stable opaque session IDs and monotonically increasing revisions. Bind access tokens to a session ID/revision and check current session status for sensitive actions. Add explicit access-token/session revocation semantics. Integrate refresh-token rotation, logout, account suspension and authenticator revocation with the server session lifecycle.

Stateless JWT validity alone is not sufficient for a sensitive platform mutation. Do not rely solely on a client-side token or long-lived JWT claim to represent current assurance.

**Consequences:** Add forward-only session persistence and lookup; define how API authentication checks current user and session state; expire/revoke outstanding step-up challenges and proofs on session revocation. Network partitions or unavailable revocation state fail closed for sensitive actions.

## ADR-7 — Independent authenticator recovery, 24-hour cooling-off and notifications

**Status: ACCEPTED.**

**Decision:** Recovery and replacement of privileged authenticators must use an independent identity-recovery process, impose a minimum 24-hour cooling-off period before restored credentials can authorize sensitive platform operations, and notify the account owner through established channels. Recovery cannot be approved by the affected person alone and cannot silently bypass platform permission or separation-of-duties controls.

**Consequences:** Preserve recovery request, reviewer, evidence references, wait period, notifications, cancellation/appeal and completion as auditable lifecycle events. If the independent identity-recovery authority or notification channel is unavailable, recovery remains pending rather than falling back to email-only or TOTP approval.

## ADR-8 — No TOTP or email-only fallback for privileged approvals

**Status: ACCEPTED.**

**Decision:** TOTP and email-only verification must not authorize privileged platform approvals, grants, revocations, reassignment, registry management or key retirement. A failed or unavailable WebAuthn ceremony denies the sensitive operation. Account recovery is a separate governed process and does not convert email confirmation into step-up assurance.

**Consequences:** Availability/recovery procedures must accommodate WebAuthn loss without weakening the approval boundary. Any future alternative factor requires a new explicit architecture decision and assurance analysis.

## Sensitive operation policy

Fresh operation-bound WebAuthn proof is required for every mutation under:

- `broker_verification.approve`
- `broker_verification.suspend`
- `broker_registry.manage`
- `broker_reassignment.approve`
- HMAC key retirement approval
- platform authority grant, revoke or suspend

Each operation must also validate the current named platform permission, actor account/session state, current resource and authority revisions, tenant entitlement, evidence freshness and separation-of-duties rules. Synced passkeys are accepted for normal privileged use, but do not bypass two-person approval requirements for high-risk actions.

## Required implementation dependencies

1. **JWT signing-key rollover:** define `kid`/JWKS or an equivalent multi-key overlap and retirement contract before implementation is considered complete. The current service has no verified rollover mechanism. Define issuer key provisioning, rotation, validation overlap, rollback, revocation incident behavior and key custody without embedding secret values in code or documents.
2. **WebAuthn relying-party configuration:** establish production RP ID, allowed origins, environment separation and credential policy.
3. **Server sessions:** define session IDs, revisions, access-token binding, revocation and availability behavior.
4. **Independent recovery authority:** define permitted recovery reviewers, evidence standard, notification delivery and appeals; the 24-hour cooling-off is the minimum, not a reason to bypass identity checks.
5. **Security audit:** define append-only event storage, database role enforcement, access to credential metadata, privacy/retention and alert ownership.

## Rejected designs

- A caller-provided `mfa=true`, role claim, arbitrary header or JWT-only assurance indicator.
- Reusing organization OWNER/ADMIN membership as platform permission.
- A reusable elevated session or proof that authorizes multiple operations.
- TOTP or email-only privileged fallback.
- Passkey recovery that immediately authorizes high-impact operations.
- Treating authentication assurance as authorization or as evidence of broker-account ownership.

## Acceptance criteria for future implementation

- Every sensitive mutation requires a valid, unexpired, unused WebAuthn proof scoped to exact user, session, operation, resource and revisions.
- Mandatory user verification is checked cryptographically and tested; UI intent alone is insufficient.
- Current platform grant and resource authorization are rechecked at proof consumption.
- Proof consumption and sensitive mutation are atomic; concurrent replay permits no more than one commit.
- Session or access-token revocation prevents sensitive use even when a previously issued JWT remains within `exp`.
- Refresh rotation and logout are integrated with sessions; recovery requires independent review, at least 24 hours and notifications.
- No TOTP/email fallback authorizes privileged approval.
- Key rollover is specified and verified before release completion.
- Append-only audit covers authenticator, session, recovery, challenge, proof, permission and sensitive-action lifecycle.
- No authentication decision grants broker execution or changes MT5 deny-all behavior.

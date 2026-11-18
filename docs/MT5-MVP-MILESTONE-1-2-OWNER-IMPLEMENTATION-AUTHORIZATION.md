# MT5-MVP Milestones 1–2 — Owner Implementation Authorization

**Decision date:** 2026-11-18  
**Decision authority:** Project owner  
**Decision source:** Explicit owner instruction in the QuantOS engineering conversation dated 2026-11-18  
**Decision type:** Bounded engineering implementation authorization  
**Status:** AUTHORIZED FOR REPOSITORY ENGINEERING AND ISOLATED TESTING

## 1. Authorized scope

The project owner has explicitly authorized implementation work for:

### Milestone 1 — R5-02 and database admission
- Correct the SQLx migration/test concurrency issue without weakening migration identity checks.
- Complete database-backed admission tests for active and denied states.
- Verify organization membership, account entitlement, broker/server identity, evidence authentication/expiry/revocation, verifier authority, worker assignment and session readiness.
- Run the QOS-020 isolated integration-test lane and preserve its identity/cleanup guard.
- Prepare the implementation authorization and independent-review evidence package.

### Milestone 2 — MT5-MVP-01A single-account DEMO vertical slice
- Complete repository-side Windows worker, daemon, identity, transport, persistence and recovery implementation.
- Enforce the current broker registry binding and account entitlement before accepting or persisting MT5 frames.
- Verify the observed broker login, exact server and DEMO environment against the approved external-account binding.
- Persist fresh account, market, open-order and position snapshots into an isolated approved database.
- Demonstrate worker-only restart, monotonic sequence continuation and full reconciliation before readiness.
- Keep order submission, modification, cancellation and close unavailable.

## 2. Authorization boundaries

This is an owner authorization for engineering work. It is **not** an independent reviewer approval, security certification, production release approval, or broker-account ownership determination.

The following remain prohibited unless separately authorized through the applicable governance process:

- Production or shared-staging database migrations.
- Production credentials, production deployment or service restarts.
- Live broker accounts or live trading.
- Order submission, modification, cancellation, close-position or copy-trading execution.
- Public exposure of the bridge listener.
- Use of a non-isolated database for integration tests.
- Treating a successful MT5 login as independent proof of legal account ownership.

Only the exact disposable QOS-020 integration-test instance may be used for migration and database integration tests. The existing guard must pass unchanged and cleanup must be verified.

## 3. Independent review remains a separate gate

The implementer must not approve their own work. The independent reviewer must be a qualified person who is not the project owner or implementation author and must receive the complete R5-02 evidence package.

Until that reviewer records a decision and the designated decision-maker accepts it:

- R5-02 remains **PENDING INDEPENDENT REVIEW**.
- QOS-015C remains **NO-GO**.
- Production admission remains **NOT AUTHORIZED**.

A reviewer recommendation, successful CI run, or owner implementation authorization must not be represented as an independent approval.

## 4. External prerequisites for the real Windows DEMO acceptance

Repository work and simulated/contract tests may proceed. Real-host acceptance cannot be marked complete until the owner supplies or approves:

1. A named Windows VM/VPS provider, region, size and cost ceiling.
2. The managed Windows host and dedicated non-admin service identity.
3. A dedicated broker DEMO login and exact server name, delivered through an approved secure channel.
4. A protected credential store and certificate/PKI owner.
5. The expected account currency and approved symbol allowlist.
6. The Quant account, organization, entitlement, broker/server catalogue entries and pending binding references.
7. A separate authorized verifier/operator to inspect broker evidence and approve the binding.
8. An isolated database endpoint provisioned through the QOS-020 guard.

No host, broker login, secret, certificate, paid cloud resource or production service is created by this authorization alone.

## 5. Acceptance and evidence

Each milestone is complete only when:
- Required automated checks pass on the final commit.
- Database migration identity and cleanup checks pass.
- Negative admission tests prove fail-closed behavior.
- Real Windows DEMO evidence proves exact identity, fresh persistence and restart recovery.
- All evidence is redacted and tied to immutable source/artifact hashes.
- Independent review and accountable governance decisions are recorded separately.

**Owner decision:** AUTHORIZE BOUNDED ENGINEERING IMPLEMENTATION OF MILESTONES 1 AND 2, SUBJECT TO THE BOUNDARIES ABOVE.

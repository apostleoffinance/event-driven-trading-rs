# R5-02 Independent Review Packet

**Review status:** NOT YET REVIEWED  
**Requested disposition:** Approve, approve with conditions, defer, or reject  
**Related implementation:** PR #143 — `feat/r5-02-db-backed-admission`  
**Implementation branch head at packet creation:** `168ac178b3b3d26334dfe7223c5349c648036d54`  
**Owner implementation authorization:** `docs/MT5-MVP-MILESTONE-1-2-OWNER-IMPLEMENTATION-AUTHORIZATION.md`

> This packet is evidence for a qualified reviewer. It is not itself an approval, certification, or authorization to operate the bridge.

## 1. Review objective

Determine whether registry-backed admission is correctly enforced before MT5 data is accepted, sequence-checkpointed, or persisted, and whether the R5-02 implementation satisfies the approved architecture and security boundaries.

The reviewer must be independent of the project owner and implementation author. The reviewer must disclose relevant conflicts and identify the authority under which they are making the decision.

## 2. Scope for review

1. Registry-backed broker/account/server/binding resolution.
2. Organization membership and active account entitlement.
3. Evidence authenticity, supersession, expiry and revocation.
4. Verification-authority validity and revocation.
5. Worker assignment uniqueness, scope and revision.
6. Worker-session readiness and expiry.
7. mTLS certificate-to-worker identity binding.
8. Exact QuantOS account, terminal and broker-server binding.
9. Fail-closed behavior when the database or any required fact is unavailable.
10. Admission enforcement before every incoming frame is sequence-checkpointed or persisted.
11. Preservation of read-only capability boundaries.

## 3. Implementation evidence to inspect

- `crates/domain/src/broker_registry.rs`: pure admission decision model and opaque permit.
- `crates/persistence/src/broker_admission.rs`: repeatable-read persisted fact resolution.
- `crates/persistence/tests/r5_02_db_admission.rs`: integration tests for admitted, revoked, expired, suspended and not-ready states.
- `crates/intent-bridge/src/mt5_runtime.rs`: per-frame persisted admission guard.
- `crates/intent-bridge/src/mt5_session.rs`: authenticated terminal/server identity and immutable session binding.
- `crates/intent-bridge/src/bin/mt5-bridge-daemon.rs`: production daemon wiring to `new_with_admission`.
- `.github/workflows/ci.yml`: QOS-020 guarded database integration test lane and serialized workspace test execution.
- `docs/MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md`: first real-host acceptance prerequisites.

The reviewer must inspect the actual PR diff and final commit, not rely solely on this file's summaries.

## 4. Required independent verification

| Check | Required evidence | Reviewer result |
|---|---|---|
| Migration concurrency | Full workspace Rust test job passes with QOS-020 guard and cleanup | Pending |
| Admission allow path | Current verified binding and active entitlement produce a scoped permit | Pending |
| Evidence revocation | Append-only revocation decision denies the next evaluation | Pending |
| Evidence expiry | Expired authenticated evidence denies admission | Pending |
| Authority revocation | Revoked verifier grant denies admission | Pending |
| Entitlement suspension | Suspended entitlement denies admission | Pending |
| Session/assignment failure | Missing, expired, ambiguous or non-READY worker state denies admission | Pending |
| Database outage | Runtime refuses to checkpoint or persist frames | Pending |
| Certificate mismatch | Unknown or mismatched mTLS worker identity is rejected | Pending |
| Account/terminal/server mismatch | Frame identity cannot create or mutate a stream outside the authenticated principal scope | Pending |
| Read-only boundary | No order submit/modify/cancel/close operation is reachable through the bridge daemon | Pending |

The reviewer should request additional tests where an assertion is not directly demonstrated.

## 5. Required reviewer declarations

The reviewer must complete the following before recording a decision:

- Name and professional role:
- Organization:
- Relevant security/architecture qualifications:
- Relationship to project owner:
- Relationship to implementation author:
- Conflicts of interest:
- Authority to approve R5-02, or confirmation that the report is advisory:
- Date and review commit SHA:

A reviewer who is the project owner or implementation author cannot satisfy the independence requirement.

## 6. Decision record

Select exactly one:

- [ ] **APPROVED** — all required controls and evidence are accepted; no outstanding conditions.
- [ ] **APPROVED WITH CONDITIONS** — implementation may proceed only within the explicitly listed conditions.
- [ ] **DEFERRED** — evidence or verification is insufficient; no new authorization follows.
- [ ] **REJECTED** — material design or control failure requires remediation and resubmission.

### Findings and conditions

For each finding, record:
- Severity and affected control.
- Exact file, function, migration, test or evidence reference.
- Reproduction method.
- Required remediation.
- Owner and closure evidence.
- Whether the finding blocks implementation, data ingestion or release.

### Reviewer rationale

[Reviewer to complete.]

### Reviewer signature / authenticated approval reference

[Reviewer to complete.]

## 7. Governance effect

Until a qualified independent reviewer completes this packet and the designated decision-maker records the formal decision:

- R5-02 remains **PENDING INDEPENDENT REVIEW**.
- The owner's bounded engineering authorization remains limited to repository implementation and isolated testing.
- Production admission, production migrations, broker connectivity and live execution remain unauthorized.
- Passing CI does not constitute independent approval.

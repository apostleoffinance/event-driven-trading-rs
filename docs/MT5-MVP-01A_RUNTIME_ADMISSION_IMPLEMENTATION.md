# MT5-MVP-01A — Managed Windows vertical-slice implementation increment

**Status:** repository-side runtime admission increment implemented; real Windows DEMO acceptance remains blocked pending separately controlled infrastructure and account prerequisites.

## Implemented in this increment

1. Added `validate_ready_mt5_terminal_slot` in `crates/persistence/src/mt5_terminal_slot_admission.rs`.
   - Requires the exact slot and its managed gateway to be `READY`.
   - Requires a linked `ACTIVE` worker assignment matching organization, Quant account, binding, external broker account, broker, broker server, terminal ID and worker certificate fingerprint.
   - Requires the latest worker session to be `READY` and unexpired.
   - Database errors propagate; callers must deny ingestion.
2. Extended `Mt5AdmissionGuard` with an immutable terminal-slot reference.
3. Made the production bridge runtime check broker admission and terminal-slot admission before every incoming frame. Slot lookup failure, missing slot, non-READY host/slot/session, expired assignment/session or identity mismatch degrades the stream and refuses ingestion.
4. Made `MT5_TERMINAL_SLOT_ID` mandatory in the managed daemon configuration.
5. Updated the operator README and registry/data-contract documentation.
6. Added CI source-contract assertions for the runtime slot admission query and fail-closed behavior.

The check is deliberately separate from broker verification: a VERIFIED broker binding does not prove that a specific Windows worker is the assigned publisher.

## Runtime identity chain

```text
mTLS worker certificate
  -> provisioned worker service principal
  -> active organization membership
  -> active worker assignment
  -> exact READY terminal slot
  -> READY EdgeNest-managed gateway host
  -> VERIFIED broker binding + active account entitlement
  -> broker/server identity match
  -> read-only MT5 Bridge v2 frame ingestion
```

Every telemetry frame revalidates persisted admission and the exact slot/session state. The daemon continues to expose no order-submit, modify, cancel or close command. Execution capabilities remain disabled.

## Managed-service boundary

EdgeNest owns the Windows host, terminal installation/profile, service identities, worker, daemon, certificates, monitoring, patching and recovery. Customers do not install an EA, run a terminal, maintain a VPS, or operate a worker. The TenTrade template remains disabled and its canonical DEMO server remains blank until copied from an authoritative TenTrade source and independently verified.

## Not represented as complete

Repository changes do not create cloud resources or establish a real broker connection. The following release gates remain external or require guarded verification:

- Select and approve the Windows host/provider, Windows build, terminal session supervisor, network path and operating owner. No paid host has been provisioned by this change.
- Approve a central secret manager/workload identity and CA/PKI lifecycle, including issuance, ACLs, rotation, revocation and audit. CurrentUser DPAPI is only a local protected cache and is not accepted as the production secret-management system.
- Obtain the exact TenTrade DEMO server identifier, approved DEMO login, expected account currency and exact broker symbol allowlist through the secure operator process. Never guess these values.
- Complete independent platform verification and approval for the broker-account evidence and DEMO-to-Quant PAPER eligibility. mTLS is workload authentication, not ownership proof.
- Verify the exact disposable QOS-020 database identity guard and cleanup lane before running database-capable acceptance tests. The 2026-11-19 QOS-020 review records that the new provisioner/guard is implemented but had no successful workflow run evidenced on its reviewed head.
- Run the Windows acceptance matrix and worker-only restart/reconnect smoke on the approved host, attach redacted timestamped evidence, and obtain independent review.

## Acceptance boundary

The implementation increment is ready for automated CI review. MT5-MVP-01A as an operational milestone is **not accepted** until the real Windows DEMO host, approved credentials/certificates, guarded isolated database, 21 acceptance checks, fresh data persistence, worker restart recovery and execution-disabled evidence have all passed. No staging/production database, live account, customer self-service credential intake, or broker order execution is authorized by this increment.

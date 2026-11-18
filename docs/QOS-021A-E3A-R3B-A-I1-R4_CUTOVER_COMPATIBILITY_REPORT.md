# QOS-021A-E3A-R3B-A-I1-R4 Cutover Compatibility Report

**Date:** 2026-11-12  
**Recommendation: BLOCKED for cutover readiness.** The current application code has useful fail-closed startup checks and the isolated R3 rehearsal passed for the current API pair. Exact historical Identity/Journal binaries and an operational deployment gate that drains and prevents legacy instances from serving traffic were not established. Docker access was denied in this inspection, so no new isolated checks were attempted.

## 1. Scope and authorization

This is the read-only I1-R4 artifact, compatibility, and deployment-control review authorized by the user. No build, migration, test, database connection, deployment, service lifecycle operation, or MT5 action was performed in this phase. The previous R3 rehearsal is cited as prior evidence and was not rerun.

## 2. Initial worktree and environment preservation

| Item | Evidence |
|---|---|
| Repository | `/Users/olaoluwatunmise/event-driven-trading-rs` |
| Branch / HEAD | `main` / `f6446dd33dc74a86ede9064f35aef2ec082a1c59` |
| Initial status | `git status --short --untracked-files=all`: 105 entries. Captured at `/private/tmp/qos021a-i1-r4-status-before.txt`, SHA-256 `0c302c4084cc9e00489ea54b441b6297bf46697e9616ac6a407f603bc7659257`. |
| After read-only inspection | Same 105 entries and identical status-output hash. |
| Ignored inventory | Before/after snapshots compared byte-for-byte (`cmp` exit 0); both SHA-256 `4f65a9fead550333c028319347ae845a3e4ab95a7fd39fcded1df7b86502f673`. This inventory has 222,804 lines. |
| Docker context | `docker context show` returned `desktop-linux`. Container listing attempts failed with exit 1 due to `connect: operation not permitted` on `~/.docker/run/docker.sock`. No alternate context or access path was tried. |

All pre-existing staged, unstaged, deleted, untracked, and ignored work remains preserved. This report is the only intended repository change. No protected Docker resource was inspected or changed because the socket denied access. Hashes of the unchanged QOS-020 guard and manifest were verified as `08698f0c013d223886720010a47c5e70248221832134368820f855a178633e1b` and `f79bfdab4262f6d47a19205e7823c052f2a8778c967a1c579e97bfc6dbbeea9e`, respectively.

## 3. Artifact inventory, identity, and provenance

`find target -maxdepth 4 -type f \( -name identity-api -o -name journal-api -o -name persistence-migrate \)` found only the current local debug artifacts. A bounded search of `/Users/olaoluwatunmise` to depth four found the same three files and no historical copies. `target/release` contains no release Identity API, Journal API, or persistence migration executable. The release MT5 daemon is unrelated and is not a substitute.

| Artifact | Identity evidence | Provenance / support classification |
|---|---|---|
| Current `target/debug/identity-api` | SHA-256 `9b7df1fb4ed1fbd486952b8c4f8d36ac6dafcf0e74943eba9f602ac17ddb30d7`; Mach-O arm64; 30,138,432 bytes | **PARTIALLY VERIFIED.** Exact local file identity is verified. No embedded source commit or build attestation was found; the R3 report records this binary was exercised, but does not establish a reproducible artifact-to-commit chain. |
| Current `target/debug/journal-api` | SHA-256 `45b0668d0f2cb62be1ad8b7d9c1205d144d677493eb9863e9e707da2eea538ef`; Mach-O arm64; 36,833,536 bytes | **PARTIALLY VERIFIED**, for the same reason. |
| Current `target/debug/persistence-migrate` | SHA-256 `434efce559459cabb12b3ec5ea92e708697da38190686cc1f0dbe75827976f0d`; Mach-O arm64; 16,728,152 bytes | **PARTIALLY VERIFIED.** Identity is verified; no release manifest or source/build provenance is attached. The APIs apply migrations through their configured migrator connections at startup; this standalone artifact is not shown in a deployment path. |
| Historical pre-I1-R1 Identity and Journal executables | None found in the bounded search or current release target | **UNAVAILABLE.** No exact historical artifact, checksum, embedded version, source commit, or release manifest could be verified. No old binary was manufactured or inferred from a timestamp. |
| `target/release/mt5-bridge-daemon` | SHA-256 `14ef1871da8f67869f87bb3c2fdd92abe439304bd0faa192cdacb9945091535b`; Mach-O arm64 | **VERIFIED as an unrelated MT5 artifact only.** It is not an Identity, Journal, or migration artifact and is excluded from compatibility claims. |

No Identity/Journal release tag was found. The local MT5 v2 tag is outside this scope. A filename, local modification time, or debug build does not establish a supported release or source commit.

## 4. Old/new application-schema compatibility matrix

| Application / schema combination | Evidence and result | Classification |
|---|---|---|
| Current Identity + current Journal debug artifacts, schema at `20261115000001`, restricted runtime role | R3 report states both exact-hash local binaries started against the isolated upgraded database, returned `/readyz` HTTP 200, and completed synthetic Identity registration and authenticated Journal access. | **VERIFIED for that isolated rehearsal pair and database.** It does not establish a release artifact or production deployment. |
| Current API binaries with unsafe table-owner runtime role | R3 report: each API exited 2 before listening with `database runtime role is not sufficiently restricted`. | **VERIFIED in R3** for tested setup. Current code in `validate_identity_runtime_role` checks migration checksum, required FK/triggers, role ownership and privileges. |
| Current binaries with runtime database behind at `20261114000001` while migrator URL targeted upgraded DB | R3 report: each API exited 2 before listening after the runtime-role/schema validator rejected the runtime DB. | **VERIFIED in R3** for current binaries and tested split-URL configuration. |
| Old migration file set through `20261114000001` against database upgraded through `20261115000001` | R3 ran SQLx with the old migration set; exit 1: `migration 20261115000001 was previously applied but is missing in the resolved migrations`. | **VERIFIED for the SQLx migration-set mismatch only.** It is explicitly not a test of an exact historical application executable. |
| Exact historical Identity/Journal binaries after the migration | No artifacts available; no binary-level test. | **NOT VERIFIED.** |
| Old application process already running before migration, then reconnecting or continuing against upgraded schema | No exact old binary and no process/deployment environment available. | **NOT VERIFIED.** The SQLx migration-set result does not prove how a legacy process handles reconnects or requests. |

Relevant source: `crates/identity-api/src/main.rs` and `crates/journal-api/src/main.rs` both open a migrator connection, apply the workspace migrations, close that pool, open `DATABASE_URL`, call `persistence::validate_identity_runtime_role`, and bind only after those steps succeed. `crates/persistence/src/pool.rs`, `validate_identity_runtime_role`, checks the exact successful `20261115000001` checksum, schema hardening objects, and restricted runtime-role conditions. `crates/identity-api/src/service.rs::ready` and `crates/journal-api/src/http.rs::ready` only perform `SELECT 1`; readiness itself does not inspect schema version or artifact compatibility. Current binaries validate before binding, but an already-running incompatible process is not thereby removed from traffic.

Migration hashes rechecked in this phase:

- `20261114000001_identity_sessions_and_tokens.sql`: `3a2b1d9915838975bd7f02466dd4fa7e4da066335ac720f8e1aa2aafc9dd1ed8`
- `20261115000001_identity_suspension_and_audit_hardening.sql`: `740c7f7074381c84f1aa9f326ecbb00cc844705d6f2f2e28611bfa8e6c28fef9`

## 5. Deployment control evidence and gaps

The repository search covered `.github/workflows`, `scripts`, deployment/config manifests, API crates, and LaunchAgents/LaunchDaemons matching Identity/Journal paths. The only application deployment workflow found is for the Cloudflare frontend/worker. `.github/workflows/ci.yml` runs checks/tests; `traderos-migration-tests.yml` is for the separate TraderOS migration. No Identity/Journal service deployment target, rolling-update policy, load-balancer routing control, service manager definition, termination grace, release-pair manifest, or old-instance restart suppression was found. A read-only local LaunchAgent search returned no matching Identity/Journal API service.

| Required control | Evidence / classification |
|---|---|
| Stop admitting requests to old instances | **Missing/unverified.** No API load balancer or routing deployment configuration found. README procedure does not drain traffic. |
| Drain in-flight requests and terminate old instances | **Documented operational procedure only / incomplete.** READMEs require coordinated rollout. API main functions call `with_graceful_shutdown` using `tokio::signal::ctrl_c()`, but no supervisor configuration, signal contract, drain deadline, or orchestrated termination was found. The source does not establish handling of a deployment manager's SIGTERM. |
| Apply migration once using dedicated migrator identity | **Application-enforced separation partially; deployment sequencing missing.** Both APIs require `MIGRATOR_DATABASE_URL`, use it for migration at startup, close it, then use `DATABASE_URL`. No deployment workflow or single migration job ensures exactly one migration run before the pair rollout. |
| Start only a mutually compatible Identity/Journal pair | **Documented operational procedure only.** Both READMEs say the services must be rolled together, but there is no pair manifest, deployment unit, version gate, or automated rollout controller. |
| Require schema/runtime-role compatibility before readiness | **Application-enforced at startup for current artifacts.** The current API binds only after migration and runtime-role validation. `/readyz` later checks DB reachability only (`SELECT 1`), not binary/schema compatibility independently. R3 tested current binary startup failures for selected mismatches. |
| Prevent old instances from reconnecting after migration | **Missing/unverified.** No deployment ownership or restart policy was found; exact old binaries are unavailable. |
| Keep incompatible instances out if restarted | **Partially enforced only for current implementation.** Current startup fails closed for an unsupported schema/role. There is no evidence that legacy versions perform an equivalent check or that the deployment system prevents their restart. |
| Restore service only after both APIs pass readiness | **Documented operational procedure only.** No traffic-promotion gate waits on both services. R3 tested both current APIs locally, not a deployment gate. |

The API READMEs document coordinated rollout, restricted runtime role, reauthentication, and forward-only recovery. Those instructions are not an operationally enforceable gate. Further, because each API currently runs the migration set during startup, a future cutover must explicitly manage the migrator principal and sequencing rather than assume a separate migration job is already implemented.

## 6. Isolated verification results and commands

No new database-capable command was run. Docker context and access checks were:

```text
$ docker context show
desktop-linux
$ docker ps --no-trunc --filter id=f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82 --format 'qos020={{.ID}} {{.Names}} {{.Status}}'
permission denied ... connect: operation not permitted
exit 1
$ docker ps --no-trunc --filter id=dc9abdf6c9ae2a7560f95fb1ce5e5903e1ab4def23627bbd0d48c4bdff03a2e7 --format 'staging={{.ID}} {{.Names}} {{.Status}}'
permission denied ... connect: operation not permitted
exit 1
```

No attempt was made to access either database or to provision a replacement environment. The previous R3 report is the only isolated verification evidence referenced here. It records the current API pair passing startup/readiness and restricted-role checks, plus SQLx rejecting the pre-I1 migration set against the upgraded schema. R3 expressly did not test an exact historical binary.

## 7. Readiness and incompatible-instance rejection

**PARTIALLY VERIFIED.** Current application startup is fail-closed before opening a listener if the migration fails or the runtime role/schema validator rejects the database. R3 directly exercised current binaries against selected role/schema mismatches. However, `/readyz` itself tests only database availability. No system-level readiness configuration removes an instance from routing, no coordinated pair check exists, and no mechanism prevents a pre-existing or automatically restarted old process from serving/reconnecting after migration. Therefore “incompatible instances cannot serve traffic” is not demonstrated.

## 8. Production-sized operational planning requirements

No production systems, databases, data, or credentials were accessed. The values below remain **NOT MEASURED** and must be agreed and tested on a production-sized isolated rehearsal before rollout:

- Maximum acceptable backup age and approved RPO.
- Backup format, integrity verification, restore procedure, and measured restore duration into a separate isolated database.
- Migration lock duration and maximum acceptable lock threshold for the production-sized `identity_session_events` table; define lock timeout and abort criteria.
- Maximum request drain time, service recovery objective/RTO, and supervisor termination deadline, including verified SIGTERM behavior.
- Named incident owner, database operator, application release approver, escalation contacts, and decision authority for forward repair versus restore.
- Post-migration rollback limitation: there is no down migration; do not start old binaries against the upgraded schema. A restore must account for writes after the backup and any required reconciliation.
- Reauthentication/user communication plan because the existing migration invalidates legacy unbound refresh tokens.

R3 validated backup restoration and migration behavior on a synthetic local PostgreSQL environment, but did not measure production-sized lock, drain, backup-age, restore, or recovery objectives.

## 9. Cleanup and protected-resource preservation

This R4 phase created no Docker resource, test database, or build artifact. It performed no Docker lifecycle operation and no database access. Socket denial prevented checking live protected-container status in this phase; this is an inspection limitation, not evidence of resource absence. Protected container IDs were used only as read-only filters. The QOS-020 manifest and guard hashes match their expected values. No MT5 staging resource, staging database, production resource, service, or broker execution path was touched.

The two local diagnostic status snapshots under `/private/tmp` are phase evidence only and are outside the repository; they do not contain credentials. No repository cleanup was performed.

## 10. Final findings and recommendation

1. **BLOCKER — historical artifacts unavailable.** Exact old Identity/Journal binaries and their provenance cannot be established. The R3 SQLx migration-set rejection is useful boundary evidence but is not a historical binary test.
2. **BLOCKER — deployment gate absent.** No enforced drain, old-instance suppression, compatible-pair rollout, or dual-readiness traffic promotion was found. README guidance alone does not prevent an old instance from serving traffic.
3. **PARTIAL — current startup guard is sound for the verified path.** Current binaries check migrations and runtime-role/schema before listener bind; this was exercised in the isolated R3 rehearsal. Readiness endpoint itself checks DB connectivity only.
4. **PARTIAL — isolated evidence exists but was not repeated.** R3 used pre-existing local debug artifacts and a disposable database; exact production release artifacts and production-sized operational metrics remain outstanding.
5. **BLOCKED — this phase could not inspect live Docker identity.** `desktop-linux` was selected, but the daemon socket returned operation-not-permitted. No workaround was attempted, and no database checks were run.

**Overall: BLOCKED for I1-R4 cutover acceptance.** Do not perform the I1-R1 production cutover on this evidence. A separately authorized next phase should first establish immutable versioned Identity/Journal release artifacts with source/build attestations, then implement or identify an enforceable deployment controller that drains and suppresses legacy instances, performs the migration as a controlled single step, deploys a tested compatible pair, and promotes traffic only after both versions pass schema-aware readiness. After those gates are available, repeat bounded compatibility tests in a newly identified disposable environment and record operationally relevant timing measurements. No implementation or MT5-MVP-01A work began.

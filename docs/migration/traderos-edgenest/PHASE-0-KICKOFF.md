# TraderOS → EdgeNest Migration — Phase 0 Kickoff

**Status:** Phase 0 baseline established; remaining live-access/sign-off gates open  
**Owner:** EdgeNest / TraderOS project owner  
**Target repository:** `apostleoffinance/event-driven-trading-rs`  
**Source repository:** `apostleoffinance/traders-os`  
**Branch:** `migration/traderos-edgenest-phase-0`  
**Created:** 2026-11-06

> This document initiates the TraderOS capability migration. It is a scope, governance, and readiness artifact—not authorization to alter production databases, deploy code, change broker connectivity, or delete source data.

## 1. Goal

Integrate the selected TraderOS capabilities into EdgeNest while retaining EdgeNest as the authoritative platform for identity, account ownership, trading connectivity, execution, risk authorization, persistence, and reconciliation.

### In scope

- TraderOS Journal and historical journal records
- Manual journal entry and editing
- MT5-derived journal synchronization through EdgeNest-owned connectivity
- Risk Calculator
- Analytics Lab
- Related journal notes, setups, psychology, checklists, and screenshots
- Migration of historical TraderOS PostgreSQL data and S3-backed screenshot objects

### Explicitly out of scope for this migration unless separately approved

- Wholesale restoration/copy of TraderOS database
- TraderOS authentication, JWT/session infrastructure, password hashes, or active user credentials
- Running TraderOS MT5 integration alongside EdgeNest MT5 bridge
- TraderOS Quant Lab
- Replacing EdgeNest Rust risk/OMS/execution authority
- A broad EdgeNest redesign
- Live broker execution changes or MT5 bridge release-gate bypasses

## 2. Source and destination systems

| System | Current known state | Migration role |
|---|---|---|
| TraderOS repository | GitHub: `apostleoffinance/traders-os` | Source application and migration logic reference |
| TraderOS PostgreSQL | Neon PostgreSQL (Render hosts the TraderOS backend service) | Source historical journal database |
| TraderOS screenshot storage | S3 | Source image objects |
| EdgeNest repository | GitHub: `apostleoffinance/event-driven-trading-rs` | Destination application/platform |
| EdgeNest database | PostgreSQL; provider not yet verified | Destination data store |
| EdgeNest authentication | Implementation not yet verified | Canonical identity and authorization |
| EdgeNest MT5 bridge | Existing project, release gates apply | Sole intended MT5 connectivity source |

**Known unknowns:** exact deployed commit/version, live schema version, source row counts, S3 bucket/object counts, destination DB host/provider, EdgeNest auth readiness, tenant model, staging environment, and production access procedure.

These must be verified rather than inferred from repository code.

## 3. Architecture guardrails

1. EdgeNest owns canonical user/tenant identity and account ownership.
2. EdgeNest owns MT5 connectivity and normalized broker events.
3. Journal is a separate domain from OMS orders and positions.
4. Journal is read-only with respect to execution authority.
5. Calculator outputs are advisory/planning results; Rust risk authorization remains authoritative.
6. Analytics reads canonical journal data rather than maintaining a second trade ledger.
7. Manual and MT5-originated trades coexist in one canonical journal model.
8. Broker facts and user annotations are distinct. Broker facts are not silently overwritten.
9. Financial values use exact decimal/NUMERIC semantics; do not introduce floating-point financial state.
10. Historical facts are auditable and corrections are represented explicitly.
11. Migration must be idempotent, resumable, observable, and validated.
12. Source databases and S3 objects remain untouched during initial migration.
13. No production write, deployment, broker operation, or destructive action without an explicit change window and approval.
14. Never place database passwords, S3 keys, JWT secrets, tokens, or other credentials in chat, source control, logs, or migration reports.

## 4. Migration ownership boundaries

| Capability | Authority after integration |
|---|---|
| Authentication / user identity | EdgeNest |
| Tenant / account authorization | EdgeNest |
| MT5 terminal and bridge lifecycle | EdgeNest |
| Broker event normalization | EdgeNest |
| Order / execution / risk authorization | EdgeNest Rust platform |
| Canonical Journal | EdgeNest Journal domain |
| User annotations and manual journal entries | EdgeNest Journal |
| Calculator UI / preview | EdgeNest Trader Tools |
| Execution risk decision | EdgeNest Rust risk engine |
| Analytics | EdgeNest Analytics Lab over canonical Journal |
| Legacy TraderOS data | Read-only source/archive during transition |

## 5. Phase 0 work checklist

### 0.1 Scope and project controls

- [x] Confirm source and destination repositories.
- [x] Confirm desired capabilities: Journal, Risk Calculator, Analytics Lab.
- [x] Confirm Journal needs manual and MT5-originated trades.
- [x] Confirm historical TraderOS journal data must be preserved.
- [x] Confirm source DB provider is Neon PostgreSQL (backend service is hosted on Render).
- [x] Confirm screenshot source storage is S3.
- [x] Record that EdgeNest database is PostgreSQL.
- [ ] Verify destination PostgreSQL provider/host and environment separation.
- [ ] Confirm staging environments for both systems.
- [ ] Confirm migration owner and approvers.
- [ ] Confirm maintenance/read-only window expectations.
- [ ] Confirm desired rollback window and source-retention period.

### 0.2 Repository and deployment baseline

- [x] Inspect EdgeNest README and current migration audit.
- [x] Inspect EdgeNest engineering constitution and platform guide.
- [x] Confirm EdgeNest README describes the frontend as mock-first/operator console.
- [x] Confirm EdgeNest platform guide treats gateway/API capabilities as target architecture until proven deployed.
- [x] Record current EdgeNest main commit: `ed2baa42ddf958802182050978960fcf3315a7fa` (source tree audit; deployment commit remains unverified).
- [x] Record TraderOS main/deployed source commit from prior audit: `c20958479f8053936c5eeeee4352754a4acb0ff0`; deployment health/API remains unverified.
- [x] Confirm TraderOS backend service metadata on Render and frontend deployment metadata on Vercel; EdgeNest production service remains unidentified.
- [ ] Confirm current database migration versions for both systems.
- [ ] Confirm existing CI checks and required branch protections.

### 0.3 Data and access readiness

- [ ] Obtain a secure, temporary, least-privilege, read-only TraderOS DB credential.
- [ ] Verify source schema and migration version without modifying source.
- [ ] Capture baseline table counts and relevant row counts.
- [ ] Verify S3 access using a least-privilege role/policy.
- [ ] Inventory screenshot objects and metadata; do not expose object contents or credentials.
- [ ] Obtain destination staging DB access through approved secret management.
- [ ] Verify backup and restore procedures for both databases.
- [ ] Verify S3 destination/ownership/retention strategy.
- [ ] Confirm how identity and account mappings will be approved.

### 0.4 Change safety

- [x] Create dedicated migration branch.
- [ ] Confirm branch protections / PR review requirements.
- [ ] Define a migration-only environment and test accounts.
- [ ] Confirm no live accounts or broker execution are used for migration tests.
- [ ] Confirm backup is restorable before any destination schema/data change.
- [ ] Confirm migration scripts default to dry-run.
- [ ] Confirm logs redact PII and secrets.
- [ ] Confirm migration operations have an audit trail.

## 6. Repository baseline findings (initial)

Based on repository documentation—not live deployment verification:

- EdgeNest is a Rust modular trading platform with PostgreSQL persistence and a Next.js frontend.
- EdgeNest documentation establishes shared platform services for tenant identity, account registry, connectivity, risk, OMS, persistence, and reconciliation.
- EdgeNest MT5 connectivity has explicit real-terminal conformance and release gates; the migration must not bypass them.
- EdgeNest frontend is documented as mock-first. Any new Journal UI must clearly distinguish mock/demo data from real API-backed data.
- EdgeNest's engineering constitution requires exact financial values, immutable/auditable financial events, explicit uncertainty, and no silent state correction.
- TraderOS is a separate source application and must be selectively migrated, not wholesale restored.

Phase 0 repository/deployment metadata baseline has been established. Live DB/schema access, EdgeNest production deployment, identity authorization behavior, staging and sign-off remain open.

## 7. Data preservation requirements

- Preserve original source IDs in a migration mapping/provenance record.
- Preserve exact decimal values, timestamps, timezone semantics, commission, swap, and realized P&L.
- Preserve trade-to-account/user relationships through approved identity mapping.
- Preserve setups, notes, psychology, checklist relationships, and screenshot associations.
- Preserve MT5 deal/ticket provenance for deduplication and audit.
- Do not recalculate and overwrite historical financial results using current logic.
- Do not treat one MT5 deal as one complete trade by default; partial fills/closes and deal aggregation must be modeled.
- Do not import active MT5 credentials or source auth secrets.
- Do not delete, mutate, or expire source data during the migration/rollback period.

## 8. Phase 0 deliverables

- [x] Migration scope and exclusions
- [x] Source/destination system register
- [x] Architecture guardrails
- [x] Ownership boundaries
- [x] Initial repository baseline findings
- [x] Phase 0 readiness checklist
- [ ] Environment and access inventory
- [ ] Live deployment/version inventory
- [ ] Approved migration and rollback window
- [ ] Phase 0 sign-off

## 9. Phase 0 exit criteria

Phase 0 is complete only when:

1. Scope and non-goals are approved.
2. Production and staging environments are documented.
3. Source and destination versions are identified.
4. Destination PostgreSQL provider/host and backup/restore are verified.
5. EdgeNest identity, tenant, and account ownership implementation is understood.
6. Temporary read-only source DB access and S3 inventory access are available through secure channels.
7. Migration owner, reviewers, rollback window, and source-retention policy are named.
8. No unresolved issue could cause data to be imported under the wrong user/account.

**Phase 1 must not begin with production writes.** Phase 1 is a read-only technical audit and identity/account ownership investigation.

## 10. Open decisions / blockers

| ID | Decision / unknown | Required owner | Status |
|---|---|---|---|
| P0-01 | EdgeNest auth and tenant model | EdgeNest engineering | Open |
| P0-02 | EdgeNest DB provider/host and staging | Platform owner | Open |
| P0-03 | TraderOS deployed version and DB schema version | TraderOS owner | Open |
| P0-04 | EdgeNest deployed version and current frontend API mode | EdgeNest engineering | Open |
| P0-05 | Identity and account mapping policy | Product + engineering | Open |
| P0-06 | S3 bucket inventory, access, and destination storage | Infrastructure owner | Open |
| P0-07 | Backup/restore evidence | Infrastructure owner | Open |
| P0-08 | Migration window, rollback period, source retention | Product owner | Open |
| P0-09 | Staging environments and pilot users/accounts | Product + engineering | Open |

## 11. Next phase

**Phase 1 — Repository, deployment, identity, and account ownership audit.**

Read-only investigation first. No production DB mutation, no source deletion, no broker connectivity change, and no deployment until the appropriate later phase and approval.

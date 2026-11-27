# M6 Stage 1 — Production Database Topology & Migration Integrity Audit

**Audit date:** 2026-11-27  
**Repository:** `apostleoffinance/event-driven-trading-rs`  
**Audited source revision:** `a58ab577b1ec0d7c5197fa021298163159b0f863` (main; merge of PR #207)  
**Scope:** Read-only production topology, migration-ledger and schema inspection.

## Executive result

**Status: AUDIT COMPLETE WITH CONFIGURATION MAPPING OPEN.**

The live evidence strongly indicates that the EdgeNest API is using the Neon project `edgenest-production` as its active PostgreSQL target:

- The Neon database has active connections under the restricted `edgenest_runtime` role.
- The Render-hosted PostgreSQL instance named `edgenest` had no application connections at the time of inspection.
- The Neon database contains the organization and identity schema expected by the current EdgeNest application.

The exact values/targets of Render's `DATABASE_URL` and `MIGRATOR_DATABASE_URL` could not be read through the connected Render service interface. Therefore the mapping is strongly inferred from live connection evidence, not conclusively verified from the service's environment configuration. The target of `TRADEROS_DATABASE_URL` also remains unverified.

No production data, migration ledger, database role, environment variable, service configuration or deployment was changed during this audit.

## 1. Runtime and database topology

### Render application

| Property | Observed value |
|---|---|
| Workspace | EdgeNest |
| Service | `edgenest-api` |
| Service ID | `srv-dbn4bjm7bikc73crpo8g` |
| Repository | `apostleoffinance/event-driven-trading-rs` |
| Branch | `main` |
| Service URL | `https://edgenest-api.onrender.com` |
| Auto deploy | Enabled |
| Region | Oregon |
| Current live deploy at audit time | `dep-dc2vj2d9fdbs73ff6vi0` |
| Deployed commit | `eeec636ebaf4293f98527a44f5852ea958139979` (PR #186) |
| Deployed commit date | 2026-11-24 |

The audited main revision is newer than the live deployment: main is at `a58ab577b1ec0d7c5197fa021298163159b0f863`, which merges PR #207 on 2026-11-27. The deployed service was still reported at the PR #186 commit during this inspection.

### Neon EdgeNest database

| Property | Observed value |
|---|---|
| Project | `edgenest-production` |
| Project ID | `crimson-pine-85819973` |
| Branch | `main` |
| Branch ID | `br-weathered-night-ar08bgoj` |
| Database | `edgenest` |
| PostgreSQL | 18.7 |
| Region | AWS US West 2 |
| Branch protected | No |
| Project plan | Free |
| Configured history retention | 21,600 seconds (6 hours) |
| Manual snapshots returned by snapshot listing | None |

Live connection inspection returned multiple application connections under `edgenest_runtime`. The role is non-superuser, cannot create roles or databases, and cannot create objects in the public schema. Its privileges on the inspected protected tables were:

- `_sqlx_migrations`: SELECT only.
- `identity_session_events`: SELECT and INSERT only.
- No UPDATE, DELETE, TRUNCATE or TRIGGER privilege on either table.

This is consistent with the runtime-role restrictions required by the current Identity and Journal API startup checks.

The Neon owner role `edgenest_owner` is non-superuser but can create roles and databases. It is a separate, elevated role and should remain restricted to migration/administrative use. The inspected role membership does not show the runtime role inheriting the owner role.

### Render-hosted PostgreSQL instance

| Property | Observed value |
|---|---|
| Instance | `edgenest` |
| Instance ID | `dpg-dbn2k8hsrm7s73d6u8fg-a` |
| Database | `edgenest` |
| Database user | `edgenest_user` |
| PostgreSQL | 18.6 |
| Region | Oregon |
| Plan | Free |
| Status | Available |
| Expiry shown by Render | 2026-12-06 |

The Render database had no application connection visible in `pg_stat_activity` at the time of inspection; only the audit query connection was present. Its database user is not a superuser, but it has CREATE ROLE and CREATE DATABASE attributes and full privileges on `_sqlx_migrations`. This looks like an administrative/master connection, not the restricted runtime role required by the current API.

**Interpretation:** This Render database appears to be a legacy or inactive EdgeNest database rather than the currently connected application database. Do not delete it or repoint services until its ownership, contents and retention requirements are confirmed. Its displayed free-plan expiry makes that confirmation time-sensitive.

## 2. Connection-variable mapping

The current API code and service documentation require:

| Variable | Intended responsibility | Audit result |
|---|---|---|
| `DATABASE_URL` | EdgeNest runtime PostgreSQL connection | Active Neon runtime-role connections strongly indicate Neon `edgenest-production/main/edgenest`; exact environment value not visible |
| `MIGRATOR_DATABASE_URL` | Separate schema-owner/migration connection, closed before runtime pool opens | Neon has a separate elevated `edgenest_owner` role; exact Render environment value and mapping not visible |
| `TRADEROS_DATABASE_URL` | Read-only TraderOS source PostgreSQL connection for the extraction utility | Not verified; the connected Neon organization listing did not expose a project identified as TraderOS |

The migration runbook documents TraderOS as a separate Neon PostgreSQL source and requires a read-only source role. The EdgeNest Neon database inspected here contains EdgeNest tables, not the legacy TraderOS source tables.

**Required confirmation before Stage 2:** inspect the Render service's Environment page without copying secret values into chat. Confirm only the non-secret destination metadata (provider/project or host label, branch, database name and role name):

1. `DATABASE_URL` → Neon `edgenest-production`, branch `main`, database `edgenest`, role `edgenest_runtime`.
2. `MIGRATOR_DATABASE_URL` → the same Neon project/branch/database, using the separate `edgenest_owner` migration principal.
3. `TRADEROS_DATABASE_URL` → a distinct TraderOS source database and read-only role.

Do not paste connection strings, passwords, query parameters or tokens into chat.

## 3. Migration inventory and ledger integrity

The current main branch contains **26 SQL migration files**.

### Neon `edgenest-production/main/edgenest`

The live ledger contains **24 rows; all 24 are successful; zero failed rows**. The latest recorded migration is `20261119000002` (`mt5 gateway terminal registry`). Every inspected SQLx checksum is 48 bytes.

The current main branch contains two later migrations not yet recorded in this database:

| Version | Migration | Live ledger |
|---|---|---|
| `20261125000001` | Workspace persistence | Not applied |
| `20261125000002` | Workspace access grants | Not applied |

The live schema inspection agrees with the ledger: organization and membership tables exist, while `workspaces`, `workspace_events`, `workspace_account_assignments` and `workspace_access_grants` do not exist yet.

This is the expected pre-workspace migration prefix in the current migration classifier. It is not evidence of an arbitrary partial migration history. The current migration code recognizes this complete prefix and checks the recorded checksums against the compiled migration set before applying pending migrations.

**Important:** the two pending migrations are not applied by this audit. The next application startup using the current main code may apply them through `MIGRATOR_DATABASE_URL`. Do not deploy that code until Stage 2 backup and restore readiness is established and the target mapping is confirmed.

### Render PostgreSQL `edgenest`

The live ledger contains **14 successful rows; zero failed rows**. Its version sequence exactly matches the 14-version `KNOWN_RECOVERY_HISTORY` allowlist in `crates/persistence/src/pool.rs`. Every inspected SQLx checksum is 48 bytes.

The production read-only query reproducing the code's `validate_recovery_schema` preconditions returned `true`. The database therefore matches the specifically recognized legacy predecessor shape in the current code; this is not a general allowance for arbitrary partial histories.

The Render database does not contain the later organization, execution-authorization or workspace schema. It must not be mistaken for the current Neon schema.

### Checksum qualification

The audit inspected the live checksum lengths and migration versions. It did not independently recompute every SHA-384 checksum from the migration source files. The application migration path compares each recorded checksum byte-for-byte with the current migration checksum and fails closed on mismatch. No ledger entry was fabricated, edited or bypassed.

## 4. Production-readiness findings

| Finding | Severity | Evidence / impact |
|---|---|---|
| Active EdgeNest database is strongly indicated as Neon, but Render secret-to-target mapping remains unverified | High | Runtime connections observed on Neon; service environment values are not exposed by the connected Render read interface |
| Two workspace migrations are pending on Neon | High | 24 successful live migrations versus 26 migration files on current main |
| Neon main branch is not protected | High | Branch metadata reports `protected=false` |
| No manual Neon snapshots were returned | High | Snapshot listing returned an empty result |
| Neon project is on the Free plan with six-hour history retention | High | Project metadata |
| Legacy Render PostgreSQL is on Free plan and has a displayed expiry of 2026-12-06 | High | Render Postgres metadata |
| TraderOS source database target not verified | Medium | No TraderOS project was visible in the connected Neon project listing |
| Live Render deployment trails current main | Medium | Latest live deploy is PR #186; current main includes PR #207 |

## 5. Safety decisions

1. **Do not run or repair migrations manually.**
2. **Do not edit `_sqlx_migrations` or fabricate migration history.**
3. **Do not deploy the current main revision yet.** It includes two workspace migrations that are pending on the observed Neon database.
4. **Do not repoint the API to the Render PostgreSQL instance.** It has a different, older schema and was not observed serving application connections.
5. **Do not delete the Render PostgreSQL instance.** Confirm whether it is a retained legacy database or required rollback source before its displayed expiry.
6. Keep TraderOS extraction read-only and keep its source database separate from EdgeNest.
7. Keep all broker, execution, risk and tenant-isolation behavior unchanged during the database-readiness work.

## 6. Stage 2 entry criteria

Proceed to M6 Stage 2 only after the following are confirmed:

- [ ] Render `DATABASE_URL` maps to Neon `edgenest-production/main/edgenest` using `edgenest_runtime`.
- [ ] Render `MIGRATOR_DATABASE_URL` maps to the same Neon database using the separate `edgenest_owner` migration principal.
- [ ] `TRADEROS_DATABASE_URL` maps to a separate TraderOS source database with a read-only role.
- [ ] A verified backup/snapshot of the Neon production branch exists.
- [ ] A restore rehearsal has completed and the restored schema/ledger has been checked.
- [ ] The Render PostgreSQL legacy instance's retention or retirement decision is documented before 2026-12-06.
- [ ] The exact 24 recorded migration checksums are confirmed against the current migration source set before the pending migrations are run.

## 7. Audit method and limits

Read-only operations used:

- GitHub repository metadata, current migration inventory, migration code and API startup configuration.
- Render workspace, service, deployment, PostgreSQL metadata and read-only SQL query interface.
- Neon project, branch, snapshot listing, schema inspection and read-only SQL queries.
- PostgreSQL catalogs, `pg_stat_activity`, `_sqlx_migrations`, role attributes and table privileges.

No database DDL/DML, role changes, migration execution, environment updates, deployment, backup creation or restore was performed.

The connected Render interface did not expose the service's secret environment-variable values. This audit therefore does not claim direct proof of the exact `DATABASE_URL` / `MIGRATOR_DATABASE_URL` values. The active Neon runtime sessions provide strong operational evidence, but the non-secret target metadata still needs confirmation before Stage 2.

---

**Stage 1 conclusion:** The active EdgeNest database is strongly indicated to be Neon `edgenest-production/main/edgenest`, with 24 successful migrations and two workspace migrations pending. The Render PostgreSQL instance is an older 14-migration recovery-state database and appears inactive. Stage 1 is complete as a read-only evidence audit, with environment mapping and backup readiness explicitly carried into Stage 2.

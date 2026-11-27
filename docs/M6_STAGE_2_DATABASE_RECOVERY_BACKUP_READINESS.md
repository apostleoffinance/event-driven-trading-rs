# M6 Stage 2 — Database Recovery & Backup Readiness

**Date:** 2026-11-27  
**Repository revision audited:** `2491e573886d38d62929f18eccb6e167153f837b` (main, after PR #208)  
**Production database:** Neon project `edgenest-production`, branch `main`, database `edgenest`  
**Scope:** Snapshot creation, isolated restore rehearsal, restore verification, backup capability assessment and recovery runbook.

## Executive result

**Status: RESTORE REHEARSAL PASSED; CONTINUOUS / OFFSITE BACKUP READINESS BLOCKED.**

A manual Neon snapshot was created from the production branch and restored into a separate non-default branch. The restored branch reached `ready`. Catalog signatures, all 63 public table row counts, migration history and key data counts were compared between source and restore; all comparisons matched.

The snapshot is a provider-local recovery point. It is **not** an independently stored offsite backup. The Neon project currently reports a Free plan, six-hour history retention, no configured scheduled snapshot job, and no available manual snapshot history before this stage. The connected Neon API reports that scheduled backup creation is not enabled for this project. Branch protection could not be enabled because the current plan/account has reached its protected-branch limit.

No production migration was applied, no production row or schema was modified, and no application deployment or connection-string change was made.

## 1. Backup created

| Property | Value |
|---|---|
| Provider | Neon |
| Project | `edgenest-production` |
| Project ID | `crimson-pine-85819973` |
| Source branch | `main` (`br-weathered-night-ar08bgoj`) |
| Database | `edgenest` |
| Snapshot name | `m6-stage-2-pre-workspace-migration-2026-11-27` |
| Snapshot ID | `snap-rapid-feather-ar4fvokh` |
| Created | 2026-11-27 18:39:38 UTC |
| Expiry | 2026-12-27 23:59:59 UTC |
| Snapshot type | Manual |

This snapshot was taken before applying the two pending workspace migrations (`20261125000001`, `20261125000002`). It therefore preserves the exact 24-migration pre-workspace state identified in Stage 1.

## 2. Isolated restore rehearsal

The snapshot was restored with `finalize=false` to avoid replacing the production branch.

| Property | Value |
|---|---|
| Restored branch | `m6-stage-2-restore-verify-20261127` |
| Branch ID | `br-cold-bird-arghb834` |
| Source snapshot | `snap-rapid-feather-ar4fvokh` |
| Restore status | `restored` |
| Branch state | `ready` |
| Production branch replaced | No |
| Default branch changed | No |
| Expiry | 2026-12-04 23:59:59 UTC |

The temporary restored branch is not protected. The project rejected branch protection because the current plan/account has reached its protected-branch limit. A seven-day expiry was set instead. Do not use this temporary branch for application traffic or as a production connection target.

## 3. Restore verification results

The source and restored branch were queried independently. All comparisons returned identical results.

### Schema and migration integrity

| Check | Production source | Restored branch | Result |
|---|---:|---:|---|
| SQLx migrations | 24 | 24 | Match |
| Successful migrations | 24 | 24 | Match |
| Failed migrations | 0 | 0 | Match |
| Public tables | 63 | 63 | Match |
| Public columns | 685 | 685 | Match |
| Public constraints | 967 | 967 | Match |
| Public indexes | 171 | 171 | Match |
| Public sequences | 14 | 14 | Match |

The following independently computed digests matched exactly:

- Complete ordered SQLx migration ledger, including version, description, success flag and full recorded checksum.
- Public column definitions (table, column, ordinal position, data type and nullability).
- Public constraint definitions.
- Public index definitions.
- Non-internal trigger definitions.

### Data verification

Exact row counts were queried for **all 63 public tables**. No table-count differences were found.

Key table counts also matched:

| Table | Rows on source and restore |
|---|---:|
| `identity_users` | 2 |
| `identity_sessions` | 73 |
| `identity_access_tokens` | 143 |
| `identity_refresh_tokens` | 998 |
| `journal_accounts` | 3 |
| `journal_trades` | 89 |
| `organizations` | 0 |
| `organization_memberships` | 0 |
| `execution_authorizations` | 0 |
| `execution_claims` | 0 |

Sequence definitions and visible sequence state also matched. The source and restored branch both reported the same last value for the one sequence whose last value was visible; other sequence last values were NULL in both catalog results.

**Restore verification conclusion:** the provider snapshot restored a schema and database state that matched the source across all tested public tables, migration history, schema signatures and row counts. This is a successful provider-level restore rehearsal, not a full application-level disaster-recovery exercise.

## 4. Backup automation and protection capability

### Automatic snapshots

The branch's automatic snapshot schedule was inspected and returned an empty schedule. An attempt to configure a daily snapshot at 02:00 UTC with 14-day retention was rejected by Neon:

`backup schedule creation is not enabled for this project`.

No schedule was installed.

### Branch protection

The production `main` branch is currently unprotected. An attempt to protect the isolated restore branch was rejected because the current plan/account has reached its protected-branch limit. No protection setting was changed.

### Point-in-time recovery

The project reports `history_retention_seconds = 21600` (six hours). This is not a substitute for a longer-retention backup policy.

### Offsite backup

No external backup archive was created in this stage. The connected tools do not provide a supported, verified `pg_dump` export workflow for the Neon source or the legacy Render PostgreSQL instance. A provider-local snapshot does not protect against provider/account-wide loss or accidental deletion of the project.

**Required upgrade/decision:** choose an appropriate Neon plan or a separately operated backup destination that supports scheduled backups, retention and export. Do not assume a schedule exists until a schedule object or backup job is visible and a new backup has been verified.

## 5. Legacy Render PostgreSQL retention

Stage 1 identified a separate Render PostgreSQL instance named `edgenest`, with an expiry shown as **2026-12-06**. It has a distinct 14-migration recovery history and was not observed serving application connections.

It was not modified in Stage 2. No backup or export of that legacy database has yet been verified. Before its expiry, the owner must decide whether to:

1. Retain it by moving to an appropriate paid plan, or
2. Export it to a verified independent backup location and validate the export, or
3. Explicitly approve its retirement after confirming that it contains no required data.

Do not delete or allow expiry to pass on the assumption that the Neon snapshot contains the Render database; they are different database instances and their migration histories differ.

## 6. Recovery runbook

### A. Recover into a new branch first

1. Identify the desired snapshot and confirm its source branch, timestamp and retention.
2. Restore with `finalize=false` and a new, clearly named recovery branch.
3. Wait until the branch state is `ready`.
4. Confirm the database name and PostgreSQL version.
5. Compare the SQLx ledger, table inventory, schema signatures, critical table row counts and sequence state.
6. Keep the recovered branch isolated from production application traffic while investigating.
7. Do not finalize/swap the restored branch until an explicitly approved recovery decision has been made.

### B. Production replacement

Production branch replacement is **not authorized by this runbook**. Any future finalize/swap must be separately approved after:
- Identifying the incident and recovery point.
- Confirming data-loss window and snapshot timestamp.
- Confirming the restore has passed verification.
- Coordinating application maintenance/connection handling.
- Documenting rollback and post-recovery acceptance checks.

### C. Migration recovery

- Do not delete, rewrite or fabricate rows in `_sqlx_migrations`.
- Do not manually apply the two pending workspace migrations as part of a recovery test.
- Use a disposable restored branch for migration rehearsal after Stage 2 backup readiness is resolved.
- Keep the production database at its 24-migration state until the migration target and backup prerequisites are confirmed.

## 7. Stage 2 acceptance checklist

| Criterion | Status |
|---|---|
| Manual pre-migration snapshot created | PASS |
| Snapshot restored into a separate branch | PASS |
| Restore branch reached ready state | PASS |
| All 63 public table row counts match | PASS |
| Migration ledger and checksums match | PASS |
| Column, constraint, index and trigger signatures match | PASS |
| Sequence definitions/state match | PASS |
| Automated snapshot schedule configured | BLOCKED — not enabled for project |
| Production branch protection enabled | BLOCKED — protected-branch limit reached |
| Independent offsite backup created and verified | NOT DONE |
| Legacy Render database retention/backup decision | OPEN |
| Restore test through application services | NOT DONE |
| Exact Render connection-variable target mapping | STILL OPEN FROM STAGE 1 |

## 8. What must happen before Stage 3 / production migration work

1. Confirm the Neon plan or external backup destination that will support automated, retained and independently stored backups.
2. Configure and verify a scheduled backup or documented external backup job.
3. Create and test an independent export/restore path; retain evidence of the restore.
4. Decide the legacy Render database's fate before 2026-12-06.
5. Confirm `DATABASE_URL`, `MIGRATOR_DATABASE_URL` and `TRADEROS_DATABASE_URL` target metadata as required by Stage 1.
6. Re-run the migration source/ledger checksum preflight on the intended production target.
7. Only then rehearse the two workspace migrations on a disposable restore branch.

---

**Stage 2 conclusion:** a manual Neon snapshot was created and successfully restored with exact equality across the tested schema signatures, migration ledger and all 63 public table row counts. The restore rehearsal passes. However, scheduled backup creation, branch protection and offsite backup are unavailable or unconfigured under the current project capability. The database is not yet considered fully disaster-recovery ready, and production migrations remain gated.

# M6 — Zero-Cost Neon Offsite Backup Implementation

**Status:** DEFERRED — repository workflow and implementation notes are preserved; R2 provisioning and activation are intentionally postponed while the Journal MVP is the delivery priority.
**Target when resumed:** Neon `edgenest-production/main/edgenest` → encrypted Cloudflare R2 object storage.
**Current production impact:** None. No R2 bucket, credentials, database role, production environment configuration, migration, or deployment is being created as part of this deferred work.

> **Priority decision (2026-11-27):** The active delivery focus is Workspace & Organization, with the Journal as the MVP. The R2 offsite-backup rollout is a separate infrastructure-hardening workstream. It must not block Workspace & Organization implementation or the Journal MVP rollout, provided the existing Neon recovery point is verified and retained before any production migration.

## 1. Cost and capacity assessment

The live Neon project previously reported a logical database size of **38,543,360 bytes (about 36.8 MiB)** on the production branch. This is a point-in-time measurement; backup archives will vary with data growth and compression. Re-measure database size and current Cloudflare account usage when this work is resumed.

Cloudflare R2 Standard's assessed monthly free allowance was:

- 10 GB-month storage
- 1 million Class A operations
- 10 million Class B operations
- No egress bandwidth charge

GitHub Actions on this repository can use standard hosted runners without minute charges while the repository remains public. The workflow does not use Actions artifacts for backup storage.

At the measured logical database size, 30 uncompressed daily copies would be about 1.1 GB before custom-format compression and age encryption overhead. This is an estimate, not a measured archive size. The workflow refuses an upload when the existing `edgenest/production/` prefix exceeds 8,000,000,000 bytes. This safety ceiling leaves headroom, but it cannot account for other R2 buckets or workloads consuming the account-wide free allowance.

**Zero-cost condition:** When activated, keep total account usage within the R2 free allowance, use Standard storage, configure a 30-day lifecycle expiration rule, and monitor account usage. Cloudflare can charge for usage above the free allowance. Do not treat the 8 GB prefix guard as a guarantee that the entire Cloudflare account will remain free. Recheck Cloudflare's current pricing and allowances before provisioning.

## 2. Work already implemented

File: `.github/workflows/neon-r2-backup.yml`

The workflow was added in PR #210 and merged to `main`. It:

- Is configured for a daily 02:30 UTC run on `main`, or manual dispatch from `main`.
- Uses PostgreSQL 18's `pg_dump` custom format with compression.
- Encrypts the archive client-side using age before upload.
- Decrypts the archive in the same run and checks its PostgreSQL archive directory with `pg_restore --list`.
- Computes a SHA-256 checksum for the encrypted archive and uploads the archive plus checksum sidecar.
- Uploads to a timestamped key under `edgenest/production/YYYY/MM/DD/`.
- Checks the remote object size against the local encrypted archive size.
- Prevents overlapping backup runs.
- Fails closed when required secrets/variables are missing or the backup prefix exceeds its safety ceiling.

The workflow does not delete old objects. Retention is intended to be configured on the R2 bucket with a lifecycle rule, avoiding custom deletion logic and accidental deletion of unrelated objects.

**Important operational note:** The workflow file is already on `main`, including its scheduled trigger. Until the required configuration exists, scheduled attempts may fail closed because required secrets/variables are absent. Before resuming the backup rollout, review the workflow's activation state and decide whether to keep it manual-only until provisioning is complete. Do not add credentials merely to silence scheduled failures.

## 3. Deferred Cloudflare setup — do not perform during the Journal MVP milestone

When the backup work is explicitly resumed:

1. In the Cloudflare dashboard, open **Storage & databases → R2** and create a **private** Standard bucket dedicated to EdgeNest backups.
2. Create an R2 API token restricted to that bucket with **Object Read & Write** permissions. Do not use a Global API Key.
3. Configure an object lifecycle rule on the bucket to expire objects under `edgenest/production/` after **30 days**. Verify the rule is active.
4. Keep the bucket private; do not enable public access or attach a public `r2.dev` URL.
5. Record the Cloudflare account ID and bucket name. Keep access key ID and secret access key out of repository files, issues and chat.

Cloudflare's R2 S3 endpoint is `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`; the workflow uses region `auto`.

## 4. Deferred age encryption identity

When this work is resumed, generate the age identity locally on a trusted machine:

```bash
age-keygen -o edgenest-backup-age-identity.txt
```

The file contains a private identity and a public recipient line. Protect the file and do not commit it.

- Put the public recipient (the `age1...` value) in the GitHub repository variable `BACKUP_AGE_RECIPIENT`.
- Put the complete private identity file contents in the GitHub repository secret `BACKUP_AGE_IDENTITY`.
- Keep a separate secure copy of the private identity offline. Without it, encrypted backups cannot be decrypted.

## 5. Deferred GitHub repository configuration

When the R2 destination and encryption identity are ready, open **Settings → Secrets and variables → Actions**.

### Repository variables

| Name | Value |
|---|---|
| `R2_ACCOUNT_ID` | Cloudflare account ID (32-character hex) |
| `R2_BUCKET` | Dedicated private R2 bucket name |
| `BACKUP_AGE_RECIPIENT` | Public age recipient beginning `age1` |

### Repository secrets

| Name | Value |
|---|---|
| `NEON_DATABASE_URL` | Neon production connection string for a backup-only database principal |
| `R2_ACCESS_KEY_ID` | R2 bucket-scoped S3 access key ID |
| `R2_SECRET_ACCESS_KEY` | R2 bucket-scoped S3 secret access key |
| `BACKUP_AGE_IDENTITY` | Complete private age identity file contents |

Use a dedicated database principal with only the permissions needed to read the complete database for backup. Do not reuse the application runtime credential or migration-owner credential by default. PostgreSQL row-level security and object privileges must be evaluated so that the dump includes every required row. Creating this principal or granting privileges requires a separate review and explicit approval.

Never paste connection strings, API keys or encryption identities into source files, pull requests, workflow logs or chat.

## 6. Activation and verification — future milestone

Only after the R2 bucket, lifecycle rule, GitHub variables and GitHub secrets are configured:

1. Review the workflow activation state and enable the intended schedule or manual dispatch.
2. Open **Actions → Neon encrypted offsite backup**.
3. Run **Run workflow** against `main`.
4. Confirm the job completes successfully.
5. Confirm the timestamped encrypted object and its `.sha256` sidecar exist in the private R2 bucket.
6. Confirm the reported remote object size matches the local archive size.
7. Record the object key, timestamp, archive size and workflow run URL in the operational recovery record.
8. Separately restore a downloaded archive into a disposable Neon branch and validate schema, migration ledger and critical row counts before calling the independent restore path proven.

A successful `pg_restore --list` check verifies that the encrypted archive can be decrypted and is structurally readable. It is **not** a full database restore test.

## 7. Recovery outline

1. Download the encrypted object and checksum sidecar from the private R2 bucket.
2. Verify the encrypted file's SHA-256 checksum.
3. Decrypt with the offline age identity.
4. Restore into a new isolated Neon branch/database using PostgreSQL 18 tooling.
5. Verify migration ledger, schema signatures, key table row counts, indexes and sequences.
6. Keep the recovered branch isolated until an explicitly approved recovery decision is made.

Do not restore over or finalize the production branch as part of a routine backup test.

## 8. Deferred completion checklist

- [ ] Reassess database size, account-wide R2 usage and current R2 pricing/free allowances.
- [ ] Decide whether the existing scheduled workflow should remain disabled/manual-only until provisioning is complete.
- [ ] Cloudflare private R2 bucket created.
- [ ] Bucket-scoped Object Read & Write token created.
- [ ] 30-day lifecycle expiration rule configured and verified.
- [ ] Age identity generated; private identity stored offline.
- [ ] Backup-only database principal reviewed and explicitly approved.
- [ ] GitHub variables and secrets configured.
- [ ] First manual workflow run passes.
- [ ] Encrypted archive and checksum sidecar verified in R2.
- [ ] Full isolated restore from R2 archive completed and documented.
- [ ] Legacy Render PostgreSQL retention/export decision resolved before its previously recorded 2026-12-06 expiry; recheck the current instance status before acting.

## 9. Relationship to Workspace & Organization and Journal MVP

This backup rollout is **not a feature dependency** for Workspace & Organization. It is a separate disaster-recovery enhancement.

The active product sequence is:

1. Complete the Workspace & Organization persistence and access-control implementation.
2. Verify organization membership, workspace authorization, account placement and trading-resource isolation.
3. Complete Journal MVP integration and regression tests while preserving user-private Journal ownership and access boundaries.
4. Prepare the Journal MVP for rollout after production readiness checks and explicit deployment approval.

Before applying any production Workspace & Organization migration, verify that the existing Neon provider snapshot and isolated restore rehearsal are still available and that the recovery point is suitable for the planned change. The previously recorded snapshot was named `m6-stage-2-pre-workspace-migration-2026-11-27`; verify its current availability and expiry before relying on it. The unfinished Cloudflare R2 offsite-backup rollout does **not** independently block Workspace & Organization work or the Journal MVP, but each production migration still requires an approved, verified recovery point. The QOS-021A-E3A-R2 organization/tenant-foundation milestone and QOS-020-R2 test-infrastructure diagnosis use the label “R2” for unrelated engineering work; those labels do not refer to Cloudflare R2 object storage. Deferring Cloudflare R2 does not waive the separate QOS-020 target-database verification, workspace release-preflight, or production security gates.

**Scope boundary:** Deferring R2 means no Cloudflare bucket provisioning, token creation, age identity setup, GitHub secret configuration, backup database-role creation, or first backup run during the current Journal MVP milestone. Preserve PR #210 and this document so the backup rollout can be resumed later without redoing the design work.

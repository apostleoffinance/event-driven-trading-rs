# M6 — Zero-Cost Neon Offsite Backup Implementation

**Status:** Repository implementation prepared; Cloudflare and GitHub secret configuration required before the first backup can run.  
**Target:** Neon `edgenest-production/main/edgenest` → encrypted Cloudflare R2 object storage.  
**Production impact:** None. The workflow uses `pg_dump` and does not apply migrations or modify database rows/schema.

## 1. Cost and capacity assessment

The live Neon project currently reports a logical database size of **38,543,360 bytes (about 36.8 MiB)** on the production branch. This is a point-in-time measurement; backup archives will vary with data growth and compression.

Cloudflare R2 Standard's current monthly free allowance is:

- 10 GB-month storage
- 1 million Class A operations
- 10 million Class B operations
- No egress bandwidth charge

GitHub Actions on this repository can use standard hosted runners without minute charges while the repository remains public. The workflow does not use Actions artifacts for backup storage.

At the current logical database size, 30 uncompressed daily copies would be about 1.1 GB before custom-format compression and age encryption overhead. This is an estimate, not a measured archive size. The workflow refuses an upload when the existing `edgenest/production/` prefix exceeds 8,000,000,000 bytes. This safety ceiling leaves headroom, but it cannot account for other R2 buckets or workloads consuming the account-wide free allowance.

**Zero-cost condition:** Keep total account usage within the R2 free allowance, use Standard storage, configure a 30-day lifecycle expiration rule, and monitor account usage. Cloudflare can charge for usage above the free allowance. Do not treat the 8 GB prefix guard as a guarantee that the entire Cloudflare account will remain free.

## 2. What the workflow does

File: `.github/workflows/neon-r2-backup.yml`

- Runs daily at 02:30 UTC on `main`, or by manual dispatch from `main`.
- Uses PostgreSQL 18's `pg_dump` custom format with compression.
- Encrypts the archive client-side using age before upload.
- Decrypts the archive in the same run and checks its PostgreSQL archive directory with `pg_restore --list`.
- Computes a SHA-256 checksum for the encrypted archive and uploads the archive plus checksum sidecar.
- Uploads to a timestamped key under `edgenest/production/YYYY/MM/DD/`.
- Checks the remote object size against the local encrypted archive size.
- Prevents overlapping backup runs.
- Fails closed when required secrets/variables are missing or the backup prefix exceeds its safety ceiling.

The workflow does not delete old objects. Retention is configured on the R2 bucket with a lifecycle rule, avoiding custom deletion logic and accidental deletion of unrelated objects.

## 3. Cloudflare setup — required before activation

1. In the Cloudflare dashboard, open **Storage & databases → R2** and create a **private** Standard bucket dedicated to EdgeNest backups.
2. Create an R2 API token restricted to that bucket with **Object Read & Write** permissions. Do not use a Global API Key.
3. Configure an object lifecycle rule on the bucket to expire objects under `edgenest/production/` after **30 days**. Verify the rule is active.
4. Keep the bucket private; do not enable public access or attach a public `r2.dev` URL.
5. Record the Cloudflare account ID, bucket name, S3 access key ID and secret access key. Do not put credentials in repository files, issues or chat.

Cloudflare's R2 S3 endpoint is `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`; the workflow uses region `auto`.

## 4. Generate the age encryption identity

Run locally on a trusted machine:

```bash
age-keygen -o edgenest-backup-age-identity.txt
```

The file contains a private identity and a public recipient line. Protect the file and do not commit it.

- Put the public recipient (the `age1...` value) in the GitHub repository variable `BACKUP_AGE_RECIPIENT`.
- Put the complete private identity file contents in the GitHub repository secret `BACKUP_AGE_IDENTITY`.
- Keep a separate secure copy of the private identity offline. Without it, encrypted backups cannot be decrypted.

## 5. GitHub repository configuration

Open **Settings → Secrets and variables → Actions**.

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

Use a dedicated database principal with only the permissions needed to read the complete database for backup. Do not reuse the application runtime credential or migration-owner credential by default. PostgreSQL row-level security and object privileges must be evaluated so that the dump includes every required row. Do not create or grant this principal as part of this PR.

Never paste connection strings, API keys or encryption identities into source files, pull requests, workflow logs or chat.

## 6. Activation and verification

After the R2 bucket, lifecycle rule, GitHub variables and GitHub secrets are configured:

1. Open **Actions → Neon encrypted offsite backup**.
2. Run **Run workflow** against `main`.
3. Confirm the job completes successfully.
4. Confirm the timestamped encrypted object and its `.sha256` sidecar exist in the private R2 bucket.
5. Confirm the reported remote object size matches the local archive size.
6. Record the object key, timestamp, archive size and workflow run URL in the operational recovery record.
7. Separately restore a downloaded archive into a disposable Neon branch and validate schema, migration ledger and critical row counts before calling the independent restore path proven.

A successful `pg_restore --list` check verifies that the encrypted archive can be decrypted and is structurally readable. It is **not** a full database restore test.

## 7. Recovery outline

1. Download the encrypted object and checksum sidecar from the private R2 bucket.
2. Verify the encrypted file's SHA-256 checksum.
3. Decrypt with the offline age identity.
4. Restore into a new isolated Neon branch/database using PostgreSQL 18 tooling.
5. Verify migration ledger, schema signatures, key table row counts, indexes and sequences.
6. Keep the recovered branch isolated until an explicitly approved recovery decision is made.

Do not restore over or finalize the production branch as part of a routine backup test.

## 8. Remaining gates

- [ ] Cloudflare private R2 bucket created.
- [ ] Bucket-scoped Object Read & Write token created.
- [ ] 30-day lifecycle expiration rule configured and verified.
- [ ] Age identity generated; private identity stored offline.
- [ ] GitHub variables and secrets configured.
- [ ] First manual workflow run passes.
- [ ] Encrypted archive and checksum sidecar verified in R2.
- [ ] Full isolated restore from R2 archive completed and documented.
- [ ] Legacy Render PostgreSQL retention/export decision resolved before its 2026-12-06 expiry.

**Important:** Until the first backup and independent restore test pass, Stage 2 remains **partially complete** and production migrations remain gated. This implementation does not configure Neon, Render, database roles, production environment variables, or production deployments.

# TraderOS → EdgeNest Journal migration runbook

## Safety

- Use a dedicated read-only TraderOS PostgreSQL role.
- Use a disposable EdgeNest staging database/API first.
- Keep credentials in private config files or a secret manager. Never commit them.
- Create one snapshot per TraderOS user.
- Preserve source DB backups and source object storage through the rollback window.
- No user credentials, passwords, sessions, API tokens, MT5 token hashes or live execution state are migrated.
- No production import until backups, staging tests and source-to-target reconciliation are signed off.

## Install

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r scripts/traderos-migration/requirements.txt
```

## Extract

```bash
export TRADEROS_DATABASE_URL='postgresql://READ_ONLY_USER:...@.../traderos'
python scripts/traderos-migration/migrate.py extract --user-id '<TRADEROS_USER_UUID>' --out ./migration/user-<id>/snapshot.json
```

If TraderOS stores screenshots in S3/R2 rather than the database, create a private source-storage config JSON containing `endpoint_url`, `region`, `bucket`, `access_key_id`, and `secret_access_key`, then run:

```bash
python scripts/traderos-migration/migrate.py extract --user-id '<TRADEROS_USER_UUID>' --out ./migration/user-<id>/snapshot.json --source-storage-config /secure/path/traderos-source-storage.json
```

The extractor is read-only. It writes the manifest, one JSON file per selected user/account table, and screenshot binaries to a sibling directory. It includes risk events, AI data, market analyses, annotations and MT5 historical evidence; it omits authentication tables, credential columns and the global shared market-candle cache.

## Configure destination

Copy `scripts/traderos-migration/config.example.json` to a private path and configure:

- EdgeNest Journal API URL.
- Explicit TraderOS user UUID → EdgeNest user JWT mapping.
- Private destination R2/S3-compatible bucket settings. The full archive is required for apply and is uploaded with a content-addressed key.
- Keep all credentials outside Git and outside the migration report.

The migration utility creates destination journal accounts and setups idempotently using source identifiers. Account mappings and report files are kept in the private state file.

## Plan / dry-run

```bash
python scripts/traderos-migration/migrate.py plan --snapshot ./migration/user-<id>/snapshot.json --config ./migration-config.json
python scripts/traderos-migration/migrate.py apply --snapshot ./migration/user-<id>/snapshot.json --config ./migration-config.json --dry-run
```

Dry-run must not make destination API calls or change checkpoint state. Resolve validation errors and review every warning before apply. Missing screenshot bytes must be resolved; metadata-only rows do not count as migrated screenshots.

## Apply to staging

```bash
python scripts/traderos-migration/migrate.py apply --snapshot ./migration/user-<id>/snapshot.json --config ./migration-config.json
```

Apply first verifies the source snapshot, uploads a complete ZIP archive to private object storage and verifies its SHA-256/size. It then creates/resumes accounts and setups, links the archive reference to every mapped EdgeNest account, imports trades and psychology, and transfers screenshot binaries to destination storage with attachment metadata.

Other source datasets are retained in the archive rather than claimed as native Journal UI records; see MIGRATION-COVERAGE.md.

## Verify

- Reconcile every source table count against manifest.
- Confirm the archive object's SHA-256 and byte size.
- Confirm each EdgeNest account exposes `migration_metadata.traderos_source_archive`.
- Compare trade counts and gross/net P&L, commissions, swaps and currency per account.
- Check open/closed/cancelled status totals and timestamp ranges.
- Verify psychology links and screenshot checksums/attachments.
- Confirm all native import counts and all archive-only dataset counts in the report.
- Recompute EdgeNest analytics from canonical imported trades.

## Rollback

Do not delete TraderOS data. For staging, restore the disposable EdgeNest database snapshot. For production corrections, use an approved owner-scoped compensating operation or restore process—never hand-edit production rows. Retain the source archive and migration report through the agreed rollback period.

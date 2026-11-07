# TraderOS → EdgeNest migration runbook

## Safety
- Use a dedicated read-only TraderOS PostgreSQL role.
- Use a disposable EdgeNest staging database/API first.
- Keep credentials in environment variables or a secret manager; never commit config with credentials.
- Make one snapshot per TraderOS user.
- Use a separate working directory for each migration.
- Preserve original snapshots, source DB backup and source object storage through the rollback period.
- Never run this against production until the staging reconciliation is signed off.

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

The extractor creates one JSON file per selected source table plus a manifest and a separate screenshot binary directory. It never queries auth/session/MT5 credential tables.

## Configure
Copy `scripts/traderos-migration/config.example.json` to a private working directory. Configure:
- EdgeNest Journal API URL.
- Explicit source-user → EdgeNest JWT mapping.
- Per-source-account → destination-account mapping only after review (the tool may create accounts idempotently from source external IDs).
- S3-compatible destination bucket credentials only when screenshot transfer is approved.

The configuration, JWT, snapshot, state file and report are sensitive. Keep them out of Git.

## Plan / dry run
```bash
python scripts/traderos-migration/migrate.py plan --snapshot ./migration/user-<id>/snapshot.json --config ./migration-config.json
python scripts/traderos-migration/migrate.py apply --snapshot ./migration/user-<id>/snapshot.json --config ./migration-config.json --dry-run
```

Review source counts, relationship checks, unmapped fields, missing screenshot bytes, missing timezone, invalid trade lifecycle and account currency/balance assumptions.

## Apply to staging
```bash
python scripts/traderos-migration/migrate.py apply --snapshot ./migration/user-<id>/snapshot.json --config ./migration-config.json
```

Re-run is safe only with the same source user, source IDs and mapping state. Review the report after every run. The source remains untouched.

## Verify
- Compare source vs EdgeNest counts by account.
- Compare gross/net P&L, commission, swap and currencies by account.
- Compare open/closed/cancelled counts and min/max timestamps.
- Validate screenshot SHA-256 from source binary to destination object.
- Validate psychology and checklist associations.
- Review every unsupported source table/field in MIGRATION-COVERAGE.md.
- Recompute EdgeNest analytics after imported rows have been reconciled.

## Rollback
Do not delete the source. For a staging rollback, restore the disposable target DB snapshot. For a production correction, use an explicitly approved owner-scoped compensating operation or restore procedure; do not hand-edit production rows. Keep the source archive and migration report for the full rollback window.

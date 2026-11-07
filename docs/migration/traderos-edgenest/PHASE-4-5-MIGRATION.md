# TraderOS → EdgeNest migration (Phases 4–5)

This is a historical Journal migration only. It never copies TraderOS users, password hashes,
session/refresh tokens, MT5 connection tokens, API credentials, or other secrets. Source
PostgreSQL is opened read-only. EdgeNest is written only through its authenticated Journal API.

See [RUNBOOK.md](RUNBOOK.md) for the step-by-step procedure and
[MIGRATION-COVERAGE.md](MIGRATION-COVERAGE.md) for the complete coverage matrix.

## Capabilities

- Read-only, per-user PostgreSQL extraction of accounts, risk profiles, setups, trades,
  psychology, checklist templates/items/responses, and screenshot metadata/binaries.
- Exact Decimal serialization and timezone-aware timestamp preservation.
- Screenshot binary extraction to separate files with SHA-256 and byte-size manifest.
- Account/setup creation with stable TraderOS external identity.
- Retry-safe trade import using TraderOS source IDs, scoped to destination account.
- Psychology upsert and checklist response preservation in trade JSON.
- Optional S3-compatible object transfer with remote metadata checksum verification.
- Dry-run planning that does not call EdgeNest and does not mutate checkpoint state.
- Explicit warnings/errors for missing objects, incomplete screenshot transfer, unsupported data,
  invalid source relationships, invalid financial values, and unmapped records.

## What is not automatically first-class in the current Journal API

Risk-profile attributes and selected TraderOS trade facts are preserved in source payload or account
migration metadata. Checklist template definitions, market analyses, AI memories/analyses,
chart annotations and raw MT5 source snapshots are not all exposed as first-class Journal resources.
See MIGRATION-COVERAGE.md. Their presence in the source archive does not imply that EdgeNest UI
currently renders them.

## Install

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r scripts/traderos-migration/requirements.txt
```

## Configure

Copy `scripts/traderos-migration/config.example.json` to a private directory and set the EdgeNest
API URL and explicit TraderOS-user-to-EdgeNest-JWT mapping. Store secrets in a secret manager or
environment variables; never commit the private config, snapshot, checkpoint, report or JWT.

## Extract

```bash
export TRADEROS_DATABASE_URL='postgresql://READ_ONLY_USER:...@.../traderos'
python scripts/traderos-migration/migrate.py extract --user-id '<TRADEROS_USER_UUID>' --out ./snapshot.json
```

The extractor produces table JSON files and a separate screenshot binary directory beside the
manifest. It queries only the allow-listed journal tables, never authentication or MT5 credential
tables.

## Plan and dry-run

```bash
python scripts/traderos-migration/migrate.py plan --snapshot ./snapshot.json --config ./migration-config.json
python scripts/traderos-migration/migrate.py apply --snapshot ./snapshot.json --config ./migration-config.json --dry-run
```

Dry-run performs no target API requests and does not write migration checkpoint state. Resolve all
validation errors before applying.

## Apply to staging

```bash
python scripts/traderos-migration/migrate.py apply --snapshot ./snapshot.json --config ./migration-config.json
```

The source remains untouched. Use a disposable EdgeNest staging database first. The report will
not mark complete if warnings or errors remain. It is not safe to cut over until the reconciliation
and screenshot verification are signed off.

## Tests

```bash
python -m unittest discover -s scripts/traderos-migration -p 'test_*.py' -v
```

No production migration or deployment is performed by these repository changes.

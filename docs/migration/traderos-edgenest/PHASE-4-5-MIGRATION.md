# TraderOS → EdgeNest Journal migration (Phases 4–5)

This is a historical Journal migration. TraderOS PostgreSQL is queried in read-only mode. EdgeNest writes happen only through authenticated, owner-scoped Journal APIs. No users, passwords, session/API tokens, MT5 connection tokens or execution state are migrated.

See [RUNBOOK.md](RUNBOOK.md) for operational steps and [MIGRATION-COVERAGE.md](MIGRATION-COVERAGE.md) for the source/native/archive-only coverage matrix.

## Extraction

The per-user source snapshot includes accounts, risk profiles, setups, trades, psychology, checklists, screenshots, risk events, AI analyses/memories, market analyses, chart annotations, safe MT5 connection metadata, processed deals, sync snapshots, source deals and account-scoped broker candles. A separate global shared `market_candles` cache is intentionally excluded.

When screenshot binaries are stored in TraderOS S3/R2, extraction accepts `--source-storage-config` to retrieve them. The private config is not written into the export.

## Archive preservation

Before apply, the tool requires private destination object storage. It packages the manifest, all per-table JSON files and exported screenshot binaries into a ZIP, derives a SHA-256 content-addressed key, uploads to R2/S3-compatible storage and verifies object metadata/size. The archive reference and checksum are attached to each EdgeNest Journal account through its owner-scoped migration metadata API. This also runs for accounts resumed from a previous checkpoint.

## Native EdgeNest import

- Accounts and setups.
- Trades with source provenance, exact decimal strings, timestamps, risk facts, notes and checklist answers.
- Psychology linked to the imported trade.
- Screenshot binaries and attachment metadata when source bytes are available and destination object storage is configured.

Risk events, AI records, market analyses, annotations, MT5 historical raw evidence and full checklist template definitions remain preserved in the account-linked source archive until EdgeNest introduces dedicated first-class resource models/screens. This distinction is explicit in the migration report and coverage matrix.

## Validation

The utility validates source owner scope and relationships, rejects naive timestamps and invalid trade financial/lifecycle values, checks screenshot hashes, preserves source precision, and uses a dry-run that makes no API requests and changes no checkpoint state. Apply is gated on the full source archive being uploaded and verified. See the runbook for the reconciliation checklist.

## Tests

```bash
python -m unittest discover -s scripts/traderos-migration -p 'test_*.py' -v
```

This repository change does not perform a production data migration or production deployment. Use a disposable staging API/database first and only cut over after the source/target reconciliation is approved.

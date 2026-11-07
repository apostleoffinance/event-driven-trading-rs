# TraderOS → EdgeNest migration coverage

## Extracted and preserved in the immutable source archive

The per-user archive contains JSON snapshots for:
- Accounts and account risk profiles.
- Setups, trades, psychology records and checklist templates/items/responses.
- Screenshot metadata and binary files where available from PostgreSQL or the configured source object store.
- Risk events.
- AI analyses and AI memories.
- Market analyses and chart annotations.
- MT5 connection metadata (only safe columns; no token hash/prefix), processed deals, sync snapshots, source deals and account-scoped broker candles.

The archive is uploaded to private R2/S3-compatible destination storage with a SHA-256 content-addressed object key, checked by HEAD metadata and size, and linked from every matching EdgeNest Journal account's `migration_metadata.traderos_source_archive`. It is a source-preservation archive, not a claim that every record has a first-class EdgeNest UI.

The global shared `market_candles` reference cache is excluded because it is not user-owned. The `users` table and all credentials, session data, password hashes and MT5 connection tokens are excluded.

## Imported as native EdgeNest Journal resources

- Journal accounts, including currency, starting balance and source identity.
- Setups.
- Trades with financial values, timestamps, risk facts, notes, tags, checklist answers and provenance.
- Psychology records, linked to their imported trades.
- Screenshots when their binary is exported, uploaded to destination object storage, and associated through the trade attachment metadata API.

## Archive-only datasets (not yet native Journal screens/resources)

- Account risk-profile settings not represented in current Journal account contracts.
- Risk events.
- AI analyses and memories.
- Market analyses and chart annotations.
- Raw MT5 connection metadata, processed deals, source deals, sync snapshots and broker candle history.
- Checklist template/item definitions.

These datasets are fully retained in the linked source archive. They are not silently discarded and can be normalized into first-class EdgeNest features in later work. Do not claim EdgeNest analytics or UI currently renders them.

## Screenshot integrity

- PostgreSQL `file_data` is extracted to binary files and hashed.
- When bytes are not in PostgreSQL, the extractor can fetch from the TraderOS source S3-compatible object store using a private config file.
- Missing binaries are warnings and prevent a complete migration.
- Destination screenshot upload verifies SHA-256 metadata and byte size before storing the owner-scoped trade attachment reference.
- The complete source archive separately retains the original screenshot files and row metadata.

## Definition of complete

A migration is complete only when:
1. Every listed source table is present in the snapshot manifest and reconciled.
2. The complete archive is uploaded and its checksum and size verified.
3. Every account has the archive reference in EdgeNest migration metadata.
4. Native imported account, setup, trade, psychology and screenshot counts reconcile.
5. Financial totals reconcile per account and currency using exact decimal arithmetic.
6. Every screenshot binary is linked or explicitly identified as missing (missing blocks completion).
7. The destination identity/account mapping is reviewed.
8. Dry-run and staging validation succeed.

Until all required conditions pass, the tool must report an incomplete migration.

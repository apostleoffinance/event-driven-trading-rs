# EdgeNest Journal — Phase 1 Data Foundation Plan

## Design goals
- Extend the existing Journal API/model; do not create a parallel journal subsystem.
- Preserve backward compatibility with current account/setup/trade endpoints.
- Add normalized tables for trader annotations, checklist responses, executions, and import lineage.
- Use JSONB only for evolving source-specific payloads and extensible checklist metadata, not as a replacement for core queryable fields.
- Keep Analytics Lab read-only over journal facts.

## Target record groups

### Journal trade core
Retain the current trade row and add nullable, backward-compatible metadata:
- venue/provider/account external IDs and raw symbol
- instrument class and timeframe
- trading session, timezone, preferred-session flag
- planned risk percentage, planned reward, planned RR, stop/target distance
- setup-valid/rules-followed/emotional-trade/mistake flags, mistake notes, notes, discipline score
- acknowledged warnings
- MFE/MAE price and R values, capture timestamps, source and precision
- import batch/provenance JSON for raw external fields (not credentials)

### Normalized related entities
- `journal_trade_executions`: partial fills/close legs, prices, quantity, fees, timestamps, venue deal IDs
- `journal_trade_psychology`: pre/during/post emotion, confidence, fear, FOMO, frustration, revenge, boredom (bounded 0–10)
- `journal_checklist_templates` and `journal_checklist_items`: user-owned, optionally setup-linked templates and rules
- `journal_trade_checklist_responses`: per-trade item outcome and note
- `journal_trade_attachments`: screenshot/file metadata (storage provider/path only; no binary blobs in DB)
- `journal_import_batches`: owner/account/source, totals, reconciliation state and timestamps
- `journal_trade_source_records`: original source identifiers and raw normalized facts, with unique source identity

## Constraints
- All new tables include owner scope and foreign keys to the journal trade/account where applicable.
- Unique source identity prevents duplicate ingestion.
- Partial close records must not automatically mark a position closed without authoritative close evidence.
- External credentials/secrets must never be stored in journal raw payloads.
- Preserve compatibility with the current API by adding nullable/defaulted columns only.

## Analytics-ready indexes
Index by owner/account + opened/closed time, setup, symbol, session, and source identity. Do not precompute analytics prematurely; add cache/snapshot tables only after profiling.

## Phase 1 acceptance criteria
- Existing API payloads continue working unchanged.
- Migration applies to existing installations without destructive rewrites.
- Ownership and source identity constraints are enforced by PostgreSQL and API.
- Domain validation rejects invalid timestamps, negative sizes, invalid currencies and out-of-range ratings.
- Unit/integration tests cover source dedupe, tenant isolation, optional new fields, and annotation bounds.

# TraderOS → EdgeNest migration coverage

## Migrated into native EdgeNest Journal records
- Journal accounts (starting balance and account identity).
- Personal setups (name, description and source metadata).
- Trades (direction, lifecycle, prices, size, stop/target, P&L, commission, swap, currency, risk, R, sessions, timeframe, timezone, notes, discipline and mistake flags).
- Psychology records through the owner-scoped psychology endpoint.
- Checklist responses embedded in each trade's checklist JSON.
- Source provenance and source-only fields retained in the trade's source payload.

## Screenshot handling
- PostgreSQL `file_data` is extracted to separate binary files with SHA-256 and size metadata.
- If a screenshot row already contains a remote object reference, its metadata is retained.
- Destination object transfer is an explicit operation requiring configured S3-compatible storage.
- Object upload is not considered complete until the metadata endpoint accepts the association and the object can be read back and its checksum verified.
- The current attachment API stores metadata only; it is not a binary upload endpoint.
- Do not report complete screenshot parity merely because local bytes were extracted.

## Preserved as source-only snapshots / audit payloads
- Account risk-profile fields not yet represented in native EdgeNest account/risk contracts.
- TraderOS-only MT5 provider/position/deal identifiers and any unmapped raw broker facts.
- Advanced market analyses, AI analysis/memory records and chart annotations not included in the current Journal schema/API.
- Full checklist template/item definitions where no dedicated EdgeNest checklist CRUD contract exists.

## Deliberately excluded
- TraderOS users and credentials, passwords, sessions, API tokens, MT5 connection secrets and refresh tokens.
- TraderOS authentication infrastructure.
- OMS, execution, broker account credentials or live positions/orders.
- TraderOS analytics chart snapshots: analytics should be recomputed from canonical imported journal rows after reconciliation.

## Definition of complete
A migration is complete only when:
1. Every supported source record is mapped or explicitly listed as excluded/preserved in the report.
2. Imported and skipped counts reconcile to source counts.
3. Financial totals reconcile by account/currency, using exact decimal arithmetic.
4. Every screenshot binary is checksum-verified in destination storage and linked to its destination trade, or is explicitly marked pending.
5. Every psychology record is associated with the correct trade.
6. All imported records have source provenance and retry-safe identities.
7. The destination user/account mapping is reviewed and approved.
8. Dry-run and staging validation succeed before production execution.

Until all required conditions pass, the tool reports a partial migration rather than claiming complete parity.

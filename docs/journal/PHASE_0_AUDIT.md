# EdgeNest Journal Expansion — Phase 0 Repository Audit

**Date:** 2026-11-07  
**Scope:** Phase 0 audit for extending Journal and Analytics Lab using TraderOS as capability reference.  
**Repository:** `apostleoffinance/event-driven-trading-rs`

## Current implementation found

### Frontend
- Next.js 15 / React 19 / TypeScript frontend under `frontend/`.
- Journal route exists at `frontend/src/app/journal/page.tsx`.
- It is an authenticated, API-mode page using React Query.
- Current screen lists accounts and the first 100 trades; it is a basic records view, not the full Journal workspace.
- Journal API client: `frontend/src/lib/api/journal.ts`.
- Existing app style system is token-driven CSS (`frontend/src/styles/globals.css`) with dark/light themes, panels, metric cards, and data tables. Keep these conventions rather than introducing a separate UI framework.
- ECharts is already installed in frontend dependencies.

### Backend
- Rust workspace, with `crates/domain`, `crates/persistence`, and `crates/journal-api`.
- Journal HTTP routes are implemented in `crates/journal-api/src/main.rs`.
- Authenticated endpoints already include account CRUD/archive, setup CRUD/archive, and trade CRUD.
- Trade ownership comes from JWT claims, not a client-provided owner identifier.
- Persistence is PostgreSQL / SQLx and uses Decimal/Numeric for financial values.
- Journal domain model is in `crates/domain/src/journal.rs`.
- Journal SQL persistence is in `crates/persistence/src/repos/journal.rs`.
- Journal account/setup/trade HTTP handlers are in `crates/journal-api/src/resources.rs` and `crates/journal-api/src/http.rs`.
- SQLx migrations are loaded from the workspace-root `migrations/` directory by `crates/persistence/src/pool.rs`.
- Existing domain analytics helpers are in `crates/domain/src/journal_analytics.rs`; they provide basic P&L summary and position-sizing calculations only.

## Existing trade core (verified)
- owner/account/setup linkage
- symbol, long/short, open/closed/cancelled status
- source, source system and source trade ID
- opened/closed timestamps
- quantity, entry/exit, stop loss/take profit
- gross P&L, commission, swap, other fees, net P&L and currency
- risk amount and R multiple

## Gaps versus TraderOS Journal reference
- no first-class psychology/emotional context
- no rule adherence, mistake annotations, discipline score, checklist responses
- no screenshot attachments
- no first-class session/timezone/timeframe/holding duration
- no planned RR / target risk context separate from realized results
- no MFE/MAE values, timestamps, source/precision provenance
- no multi-deal/partial-close execution and fill-level records
- limited source provenance / import-batch reconciliation
- no comprehensive Journal UI (calendar, filtering, detailed trade view, import workflow)
- no complete Analytics Lab modules (execution, costs, behavior, setup edge, statistical confidence, etc.)

## Security and architecture constraints
1. Keep Journal data owner-scoped at every read/write.
2. Never trust owner/account ownership from the request body; validate account/setup ownership in the API.
3. Journal annotations and historical analytics must not bypass or mutate the OMS/risk engine.
4. Financial values remain Decimal in Rust and NUMERIC in PostgreSQL; serialize monetary values as decimal strings.
5. Preserve source facts on import; do not recompute/overwrite broker-reported historical P&L.
6. Use stable source identity and idempotent import/reconciliation.
7. Support MT5 and crypto venue-specific details without forcing exchange semantics into generic fields.
8. No fabricated data in empty/loading/error states.

## Phase 0 conclusion
The current Journal is a real API-backed CRUD foundation, but it is not yet the complete TraderOS-level Journal or Analytics Lab. Phase 1 should extend the data foundation without disrupting existing trade CRUD, identity, account connectivity, or OMS/risk boundaries.

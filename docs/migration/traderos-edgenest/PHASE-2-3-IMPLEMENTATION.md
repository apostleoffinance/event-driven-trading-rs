# Phase 2/3 Implementation — Journal, Calculator, Analytics Lab

**Branch:** `feat/journal-phase-2-3-api-calculator-analytics`  
**Base:** EdgeNest main after merged Phase 1 PR #64  
**Status:** Implementation in progress; CI pending

## Phase 2 — Journal persistence foundation

- [x] Owner-scoped JournalTrade insert/get/list/delete persistence functions.
- [x] Input validation through the domain model before insert.
- [x] Owner ID and optional journal account filter on read paths.
- [x] List pagination capped at 500 records per call.
- [x] Legacy source identity retained for import deduplication.
- [x] Journal remains separate from OMS/order/fill/position storage.
- [x] Journal page with honest empty state: no mock trade history or claim of connected production data.
- [ ] Authenticated HTTP API route/controller integration. The Rust workspace currently does not include a Journal REST service; the repository methods must be wrapped by an authenticated service before app access.
- [ ] Create/edit trade form and setup/account CRUD UI.
- [ ] Source DB/TraderOS import pipeline and reconciliation.
- [ ] Screenshot upload/storage migration.

## Phase 3 — Calculator and Analytics

- [x] Decimal-based position sizing core with risk budget, stop distance, quantity step and estimated risk.
- [x] Domain tests for risk sizing and edge cases.
- [x] Net P&L analytics core: wins/losses/breakeven, win rate, gross profit/loss, net profit, profit factor, expectancy, average win/loss, max drawdown.
- [x] Analytics domain tests for empty input and P&L series.
- [x] Calculator UI for manual estimates (client-side preview only).
- [x] Analytics Lab UI for user-entered closed trade P&L (client-side sandbox only).
- [ ] Live data integration from authenticated Journal APIs.
- [ ] Instrument contract specification integration (FX/CFD/Crypto contract-value conventions).
- [ ] Timezone/session, strategy/setup grouping, MFE/MAE and equity curve from imported journal rows.

## Important safety limitations

- Calculator is a sizing estimate and does not place orders or mutate execution accounts.
- The UI explicitly states contract value must be verified for instrument/broker/currency.
- Analytics Lab is explicitly labeled manual sandbox; it does not display fabricated user trades.
- Production auth is not yet wired into Journal routes. Do not expose owner-scoped repository functions directly to an unauthenticated HTTP route.
- No production DB writes, migrations or deployment were made.

## Verification

CI is the gate for Rust format/check/tests/Clippy, frontend lint/typecheck/build, MT5 contract tests, and portable MT5 preflight. SQL migration/application is exercised against CI disposable Postgres, not production.

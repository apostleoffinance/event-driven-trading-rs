# TraderOS → EdgeNest — Phase 1 Journal Domain Baseline

**Status:** Journal domain/schema slice implemented; CI green on code commit `a163318d8e0f3260ab811f6a214a18a6d300e1ef` (2026-11-06)  
**Branch:** `migration/traderos-edgenest-phase-1-journal`  
**Date:** 2026-11-06

## Scope of this implementation

This first slice is deliberately limited to the canonical Journal persistence/domain foundation. It does not implement Journal REST endpoints, the frontend, Calculator, Analytics Lab, or the TraderOS importer yet.

### Phase 0 — audit baseline
- Source repository: `apostleoffinance/traders-os`
- Destination repository: `apostleoffinance/event-driven-trading-rs`
- TraderOS backend service: Render; source PostgreSQL provider: Neon (these are distinct).
- TraderOS source/deployed code baseline from prior audit: `c20958479f8053936c5eeeee4352754a4acb0ff0`.
- EdgeNest main code baseline at branch creation: `ed2baa42ddf958802182050978960fcf3315a7fa`.
- Neon connector currently exposes the Analytic Sages organization but not the TraderOS project. Live schema and source counts remain unverified.
- No production DB access, source reads, DB writes, deployment, MT5 action, or trading operation occurred during this implementation.

## Journal data model

Migration: `migrations/20261106000001_journal_domain.sql`

### Tables
- `journal_accounts`: user-owned journal accounts; optional explicit link to an EdgeNest execution account.
- `journal_setups`: personal journal setup definitions, not executable strategy deployments.
- `journal_trades`: journal lifecycle, entry/exit, position quantity, prices, fees, swap, P&L, risk, R, session, tags, psychology, checklist, notes and source provenance.
- `journal_screenshots`: metadata/object references only; image bytes are not copied into PostgreSQL by this migration.
- `journal_trade_events`: append-oriented event/audit history.

### Safety and integrity
- All monetary/price/quantity values are PostgreSQL `NUMERIC(38,18)`.
- Database owner-scope composite FKs protect account/setup/trade/screenshot/event relationships from cross-owner references.
- Imported records can be deduplicated using a partial unique index on `(journal_account_id, source_system, source_trade_id)`, avoiding false collisions when broker tickets repeat across accounts.
- The source system + source trade ID must be provided together.
- Closed trades require a close timestamp and exit price; timestamps cannot close before open.
- Journal accounts are separate from EdgeNest execution accounts. A link is optional and explicit.
- Journal trade rows are not OMS order/fill/position rows and cannot authorize execution.
- No secrets/MT5 credentials are represented in the schema.

### Rust domain model
- `crates/domain/src/journal.rs`
- `JournalTrade`, `JournalDirection`, `JournalTradeStatus`, `JournalTradeSource`
- Validates non-empty IDs/symbol, positive quantity/entry price, positive optional price fields, valid lifecycle timestamps, close requirements, paired provenance IDs, and uppercase 3-character currency.
- Does not recalculate historical P&L. Imported historical values must remain source facts.

## Validation plan / acceptance

- [x] Rust formatter, workspace check, tests and Clippy pass in CI (CI run `42222958026`, conclusion `success`).
- [ ] SQL migration reviewed against supported PostgreSQL version.
- [ ] Migration applied against disposable PostgreSQL and rollback/restore strategy reviewed.
- [x] Journal domain tests pass as part of the workspace test suite.
- [ ] Identity/tenant integration reviewed before exposing APIs.
- [ ] Journal API repository/service implementation follows in next increment.
- [ ] No production migration or deployment performed.

## Known limitations / next work

1. `owner_id` is a required identity string but is not yet FK-bound to a canonical EdgeNest user/tenant table. The actual auth/tenant model is not verified. Do not expose journal routes until authz binds owner_id to the authenticated principal.
2. Setup/account ownership is enforced by composite DB FKs, but API handlers must still scope every query to the principal.
3. The schema supports basic screenshots metadata. Secure object upload/download, signed URL authorization and migration from legacy S3 remain future work.
4. Need TraderOS export data/Neon project access to build and reconcile the source-to-target importer.
5. Need staging DB and backup/restore evidence before any migration is run against production.
6. This is a first domain slice, not a full Phase 1 sign-off or production-ready Journal application.


## CI evidence

- Workflow run: [Quant OS CI #555](https://github.com/apostleoffinance/event-driven-trading-rs/actions/runs/42222958026) — **success**
- Rust: format, workspace check, workspace tests (including journal schema migration/integrity test), Clippy — **success**
- Frontend: lint, typecheck, production build — **success**
- Python MT5 contract tests — **success**
- macOS portable MT5 preflight — **success**

CI runs against a disposable PostgreSQL 16 service. It does **not** establish that production Neon or EdgeNest databases are backed up, correctly configured, or safe to migrate.

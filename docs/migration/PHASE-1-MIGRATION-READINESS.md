# Phase 1 — Migration Readiness Audit

**Status:** INCOMPLETE / BLOCKED FOR PRODUCTION MIGRATION  
**Audit date:** 2026-11-06  
**Reviewed repositories:**  
- EdgeNest: `apostleoffinance/event-driven-trading-rs` — main `5fbc2ad52c3de1797c6b986598847754578f9211`
- TraderOS: `apostleoffinance/traders-os` — main `c20958479f8053936c5eeeee4352754a4acb0ff0`

> This report is a source-code and deployment-metadata audit. It is not proof of production database state. No production database migration, trading action, or write was performed.

## Executive summary

**The migration is not ready to execute.** Repository-level mapping is substantially understood, but production verification remains blocked. The highest-risk gaps are:

1. EdgeNest does not yet have a production-ready tenant/user authorization boundary in its core SQL schema or public API. Its frontend is explicitly mock-backed, and requesting API mode still returns mock repositories.
2. TraderOS is the existing user/account/trade journal application with owner-scoped APIs and mature journal/MT5 data. It should remain the system of record for journal entities during the initial integration.
3. TraderOS frontend deployment metadata shows `NEXT_PUBLIC_API_URL` as blank. Its Next config fallback is loopback (`127.0.0.1:8000`); the actual production API target and successful browser-to-backend path must be confirmed before cutover.
4. Neither live PostgreSQL catalog/schema nor backend deployment configuration was accessible in this audit. GitHub migrations are not evidence of deployed DB state.
5. EdgeNest MT5 integration is explicitly read-only in its current adapter contract. Do not assume TraderOS MT5 importer and EdgeNest MT5 bridge are interchangeable or allow both to mutate trading state.

## 1. Source and deployment evidence

| System | Evidence inspected | What it establishes | What it does not establish |
|---|---|---|---|
| EdgeNest | GitHub main source, architecture docs, frontend implementation note, SQLx migrations, MT5 adapter docs, Phase-0 audit/task docs | Source design, tracked schema, documented current/target boundaries | Running service version, deployed DB schema, live MT5 terminal conformance |
| TraderOS | GitHub main source, FastAPI models/services/routes, Alembic chain, canonical MT5 data docs | Source models, API owner-scoping patterns, migration history | Production API deployment, live DB schema/data, current MT5 connection health |
| TraderOS Vercel | Production deployment metadata: READY, commit `c20958479f8053936c5eeeee4352754a4acb0ff0`; project env metadata | Vercel deployment is built from that commit; `NEXT_PUBLIC_API_URL` appears empty in project env metadata | Whether API is served by another host, actual runtime env at invocation, backend readiness |
| Render/backend hosting | Not connected/inspected | No conclusions | Runtime logs, backend health, deployment revision, secrets, live database |

## 2. EdgeNest audit

### Auth, user, org, tenant
- Core account identity is trading/account-centric; accounts use text identifiers.
- No user/org/tenant foreign key or membership/role schema was found in the reviewed core persistence schema.
- Tenant-scoped API auth is described as future/target work in the frontend API contract/Phase-0 readiness material.
- **Critical:** do not expose existing account/order/position APIs as multi-user APIs until an authorization policy maps authenticated principal → organization/workspace → account and every read/write is scoped server-side.

### PostgreSQL / SQLx schema
Tracked migration chain at the audited head:
- `20260306000001_init.sql`
- `20260906000001_instrument_spec.sql`
- `20261027000001_sizing_decision_audits.sql`
- `20261027000002_mt5_bridge_idempotency.sql`
- `20261027000003_mt5_reconciliation_snapshots.sql`
- `20261027000004_mt5_market_data_snapshots.sql`

Core domains include venues, instruments, accounts/account states/snapshots, strategies/versions/deployments, risk profiles, trade intents, risk decisions, orders, fills, positions, performance snapshots, audit events and execution events. Account relationships are trading-domain relationships; there is no proven mapping to TraderOS User IDs.

The migration executable explicitly runs SQLx migrations when invoked; do not assume the daemon automatically applies migrations.

### Runtime / API / frontend
- Rust platform is organized around domain, strategy runtime, risk, account, execution, venue, persistence, trading runtime and reconciliation modules.
- Strategy output is intended to be a TradeIntent; Rust risk/OMS remain authoritative.
- REST API and developer/public API are described as planned contracts, not an implemented authenticated API surface.
- Frontend routes include Command Center, Accounts, Portfolio, Strategies, Positions, Orders, Risk, Events, Reconciliation and System.
- Frontend defaults to PAPER/mock. Repository selection currently returns mock repositories even when API mode is requested (warning then fallback).
- Event stream URL/auth/reconnect/sequence behavior is proposed rather than verified as production behavior.

### MT5 bridge
- EdgeNest MT5 adapter is documented as read-only: snapshots/market data/account evidence; no order submit/modify/cancel.
- Bridge contracts include idempotency keys, stream sequencing/checkpoints and reconciliation evidence.
- MT5 host conformance and demo smoke evidence are still pending in project tracking. Treat production execution as disabled until the operator evidence gates are satisfied.
- The adapter is host-dependent; Linux Render is not a supported Windows terminal host and cannot replace the user’s actual MT5 terminal runtime.

## 3. TraderOS audit

### Identity and ownership
- TraderOS contains the existing user/auth model and JWT login/refresh/me flows.
- SQLAlchemy services include owner-scoped account and trade access; trades carry both user and account relationships.
- Setups and checklist templates are user-scoped; trade/checklist responses relate back to trade and checklist item.
- Existing account/user/trade IDs must be preserved during migration. Do not remap IDs without a durable crosswalk and referential validation.
- JWT refresh flow source does not demonstrate a persistent refresh-token revocation ledger; confirm auth hardening requirements before using TraderOS tokens as EdgeNest platform-wide identity.

### Journal entities and mapping
| TraderOS source domain | EdgeNest target / integration strategy | Migration notes |
|---|---|---|
| User | Identity/tenant principal (new shared identity boundary) | Preserve user ID; require organization membership/roles before shared accounts |
| Account | EdgeNest account only if explicitly mapped | Distinguish journal account vs execution account; no automatic merge |
| Trade | Journal/analytics domain | Keep original trade UUID/ID and user/account ownership; avoid duplicate order/fill records |
| Setup | Strategy/setup metadata | TraderOS Setup is a user journal concept, not necessarily executable EdgeNest Strategy |
| Checklist template/item/response | Journal workflow domain | Preserve item/template IDs, historical responses, and optional trade linkage |
| Psychology fields | Trade journal extension | Keep as journal-specific; not an OMS domain |
| Screenshot | Object/file evidence domain | Preserve metadata, ownership, object key and checksum; never migrate raw image payload blindly |
| MT5 connection/token | Integration credentials domain | Rotate/revoke tokens as needed; do not copy bearer secrets in cleartext |
| MT5 source deal / processed deal / snapshot | Immutable MT5 evidence + journal projection | Preserve provenance and idempotency keys; do not flatten evidence into a single trade row |

### MT5 and trade lifecycle
- TraderOS MT5 ingestion has a separate connection/deal ledger, processed-deal idempotency and source-deal evidence.
- Canonical trade documentation distinguishes immutable source deals from normalized Trade projection.
- Import/reconciliation behavior is owner-scoped and documented as read-only.
- Known source limitations include position-ID granularity, INOUT reversal handling, concurrent duplicate sync uniqueness conflicts/retry behavior, older historical rows without provenance, and non-trade balance changes not inferable from trade PnL.
- EdgeNest’s MT5 bridge and TraderOS importer have different responsibilities and evidence contracts. A migration must retain both provenance trails and avoid duplicate importer ownership.

### Screenshots / object storage
- TraderOS can store screenshot bytes in database (BYTEA) or use storage backends such as S3-compatible services/R2/MinIO; local disk is unsuitable as durable production storage.
- Migration must move payloads separately from relational records, validate checksums/size/content type, preserve owner/trade links, and use private access with short-lived signed URLs.
- S3 key layout and live backend configuration were not verified against production.

### Alembic
TraderOS tracked Alembic chain includes `0001_initial` through `0015_broker_candles`, including screenshot byte storage, MT5 sync, deal economics, source-deal evidence, timestamp normalization and candle data.
**The Alembic head in source is not verified against the deployed database.** Obtain `alembic current`, `alembic heads`, and catalog inspection from the live backend before any schema change.

## 4. Integration architecture decision

**Recommended initial topology: shared platform identity + separate bounded domains, not a single merged database.**

- Shared identity/tenant layer: principal, organization/workspace, membership, roles, account grants and audit identity.
- EdgeNest owns execution domain: strategies/deployments, risk profiles/decisions, intents, OMS orders/fills/positions, venue connectivity, execution/reconciliation.
- TraderOS owns journal domain: user journal, manual/MT5 imported trades, setup/checklist/psychology, journal analytics and screenshot metadata.
- Integration uses explicit account linking and stable IDs (external IDs plus mapping table), events/API with versioned contracts and idempotency keys.
- Do not let journal edits change OMS orders/positions or bypass EdgeNest risk/OMS.
- Keep source deal evidence and execution venue fills separate; link by provenance where possible, never deduce a match solely by symbol/time/PnL.
- Avoid cross-database foreign keys. Use integration mapping tables and reconciled async sync state.

## 5. Migration order (gated)

1. **Read-only evidence capture:** verify live EdgeNest and TraderOS DB endpoints, versions, migration heads, schema catalogs, deployment commit and health. No writes.
2. **Identity/ownership design:** define shared user/org/membership/account grant policy; approve migration of IDs and access semantics.
3. **Crosswalk:** create source→target ID mapping and entity counts, duplicate detection, orphan checks, null/invalid ownership checks.
4. **Backfill dry-run:** copy only to staging, preserving IDs and provenance; test idempotent rerun and rollback.
5. **Validation:** compare counts, checksums, balances/PnL and MT5 source-deal totals; validate screenshots and private object permissions.
6. **Dual-read/shadow:** no execution authority change; reconcile data discrepancies.
7. **Cutover:** require explicit owner approval, backup/restore proof, tested rollback and maintenance window.
8. **Post-cutover:** verify API authorization isolation, event replay, data counts, account/trade lifecycle and observability.

## 6. Blocking checklist (must pass before production migration)

| Gate | Required evidence | Status |
|---|---|---|
| Actual EdgeNest deployment commit/version | Host deployment metadata + health response | BLOCKED |
| EdgeNest production PostgreSQL schema | Read-only catalog + SQLx migration status | BLOCKED |
| TraderOS backend host/commit/health | Backend host access/logs/health endpoint | BLOCKED |
| TraderOS production PostgreSQL schema | Read-only catalog + Alembic current/head | BLOCKED |
| Vercel frontend API routing | Verify deployed API URL and browser network/API smoke test | BLOCKED (env metadata blank) |
| Auth/tenant authorization design | Approved user/org/account grant matrix and tests | BLOCKED |
| MT5 host/operator conformance | Signed/demo evidence and current bridge version/manifest | BLOCKED |
| Data counts/crosswalk | Exported source counts, ownership, duplicate/orphan checks | BLOCKED |
| Screenshot/object storage inventory | Actual backend, key format, bytes, ownership, checksum | BLOCKED |
| Backups/restore/rollback | Recent verified backup and restore drill | BLOCKED |

## 7. Safe production evidence commands

Run these from the respective backend/container with **read-only credentials** and share sanitized outputs only. Never share DB URLs, JWTs, MT5 tokens, private keys, passwords, or screenshot payloads.

### PostgreSQL catalog / migration evidence
```sql
SELECT current_database(), current_user, version();
SELECT schemaname, tablename
FROM pg_tables
WHERE schemaname NOT IN ('pg_catalog','information_schema')
ORDER BY schemaname, tablename;
SELECT table_name, column_name, data_type, is_nullable
FROM information_schema.columns
WHERE table_schema = 'public'
ORDER BY table_name, ordinal_position;
```

### TraderOS Alembic
```bash
cd backend
alembic current
alembic heads
```

### EdgeNest SQLx
- Run the project’s migration-status/verification procedure against the intended DB using the exact deployed binary/commit.
- Do not invoke migration apply against production during Phase 1.

### Deployment evidence
- Capture deployed commit SHA, release/build ID, health endpoint status, runtime version, and migration head.
- Redact all environment values; show variable names and whether configured only.

## 8. Exit criteria

Phase 1 can be marked **PASS** only when:
- all production versions and DB migration heads are verified;
- all unknown critical dependencies are resolved;
- ownership/auth model is approved and tenant isolation tested;
- entity/ID crosswalk is complete and reconciliation checks pass;
- deployment/API connectivity and MT5 operating boundaries are verified;
- rollback/backup proof is available.

**Current disposition: NOT PASS / NO-GO for production migration.** Repository source mapping is complete enough to design the integration, but live DB/deployment/auth evidence remains critical. This report intentionally does not claim otherwise.

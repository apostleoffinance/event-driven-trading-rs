# QOS-021A-E3A-R2 — Organization and Tenant Foundation

**Status: PARTIAL — implementation and non-database checks are present; database integration and concurrency acceptance gates remain unverified.**

## Scope and source state

Implemented on the authorized source tree at `/Users/olaoluwatunmise/event-driven-trading-rs`, branch `main`, HEAD `f6446dd33dc74a86ede9064f35aef2ec082a1c59`. No commit or push was made. R2 adds organization tenancy to Quant-account dashboard access. It does not add broker registries, risk issuance, execution authorization coordination, broker adapters, or live execution.

The persistence hierarchy is:

`identity_users → organizations → organization_memberships → organization_account_entitlements → accounts`

`organization_account_entitlements` is the sole Quant-account tenant assignment. An account without an ACTIVE entitlement is unassigned and absent from tenant dashboard queries. No migration backfill or owner-ID inference is performed.

## Entity, revision, and lifecycle semantics

- Organizations have a validated stable text ID, display name, ACTIVE/SUSPENDED/CLOSED status, positive revision, and timestamps. Authenticated creation inserts the organization, first OWNER membership, and initial audit events in one transaction. Organization updates use an expected revision and append an event. SUSPENDED organizations can be reactivated by an active OWNER; CLOSED organizations are terminal through this API.
- Membership is unique per organization/user, has OWNER/ADMIN/TRADER/RISK_MANAGER/VIEWER and ACTIVE/SUSPENDED/REVOKED values, and uses optimistic revisions. Changes append `organization_membership_events`. Revocation and reinstatement are updates, not deletion. Owners cannot be changed through ordinary membership PATCH; a separate atomic owner-transfer operation promotes the active target then demotes the current owner. The database trigger serializes owner removal/demotion on the organization row and rejects removal of the last active owner. Membership and entitlement rows cannot be deleted by ordinary SQL.
- Entitlement has one row per Quant account, ACTIVE/SUSPENDED/REVOKED state, positive revision, actor, reason, timestamps, and append-only event history. The account row revision is checked while locked. Assignment requires an active OWNER, the expected account revision, and entitlement revision zero for a new row. Reassignment requires OWNER membership in both organizations and an expected existing revision. A suspended or revoked entitlement is excluded from dashboard queries.
- Existing account environment and environment-verification fields are not changed by the R2 migration. `account_revision` starts at one and increments when account identity, status, environment, or verification inputs change.

The events are protected from UPDATE/DELETE by database triggers. This is an integrity guard, not a claim of immutable storage against a sufficiently privileged database owner. The application still uses the existing broad database role; R2 does not introduce PostgreSQL RLS or a separate runtime/migration role.

## Role and operation matrix

| Role | Organization/dashboard reads | Membership operations | Organization lifecycle | Quant entitlement | Paper account creation | Execution |
| --- | --- | --- | --- | --- | --- | --- |
| OWNER | Yes | Full ordinary membership administration; explicit owner transfer | Yes | Assign, suspend, revoke, and explicitly reassign | Yes | None |
| ADMIN | Yes | Ordinary membership administration; cannot grant/manage OWNER or ADMIN or modify self | Not exposed in R2 | No | No | None |
| TRADER | Yes | No | No | No | No in this phase | None |
| RISK_MANAGER | Yes | No | No | No | No | None |
| VIEWER | Yes | No | No | No | No | None |

Where the current API cannot safely support the broader role description, R2 is intentionally narrower. There is no organization-scoped risk-configuration endpoint yet; existing Journal risk policy remains private to its authenticated user. No role grants execution. These rules are enforced in the request layer and revalidated against current membership on every request; the membership revision is not cached as authorization.

## Tenant authorization flow

Every `/api/v1/trading/*` request requires the existing verified JWT and `X-Organization-Id`. The server resolves the JWT subject, reads the current membership, rejects non-ACTIVE organization or membership, and applies the route permission. The organization header selects the requested tenant but does not prove access. Trading data SQL joins `organization_account_entitlements` and constrains `organization_id` and `status='ACTIVE'` in the database query. An account ID supplied in a path is included in that scoped query; a cross-tenant or unassigned ID returns the existing not-found response.

Organization routes are under `/api/v1/organizations`: authenticated list/create; owner update; admin member list/add/update; owner transfer; entitlement list/assignment/status update. Path organization IDs must equal the selected header and are independently checked against current membership. Entitlement management is OWNER-only. The CORS allow-list includes `x-organization-id`.

## Legacy and Journal compatibility

- No `organizations`, memberships, or entitlements are inferred during migration. Existing `dashboard_account_access` rows are neither rewritten nor deleted. The trading dashboard no longer reads that table. Its grants do not authorize organization access.
- Existing Quant accounts remain unchanged and unassigned until an OWNER explicitly assigns them with account revision, entitlement revision, and an audit reason. Manual pre-assignment review is required to establish that the organization is entitled to the account; a legacy `owner_id` or dashboard grant is insufficient.
- Paper-account creation remains OWNER-only in this phase. It writes the account and then its explicit entitlement. If entitlement assignment fails, the account remains unassigned and invisible to tenant queries; this failure can leave an orphaned account for manual review.
- Journal resources (`journal_accounts`, setups, trades, imports, attachments, psychology, analytics, and journal risk policies) remain user-private and are scoped by the authenticated JWT subject (`owner_id`). They are not Quant trading accounts or organization entitlements. The Journal account creation API does not accept a linked Quant-account ID. Cross-organization Journal sharing is not part of R2.

## Migration and upgrade behavior

Added migration: `20261112000002_organization_tenant_foundation.sql`. Historical migrations were not edited. It is additive: creates organizations, organization lifecycle events, memberships, membership events, account entitlements, entitlement events and indexes; adds the account revision column/trigger; adds validation and history/last-owner triggers. Foreign keys use RESTRICT; no cascade deletes or legacy backfills occur.

The migration runner recognizes the complete supported E2 history as the pre-R2 upgrade point, validates E2 schema before proceeding, verifies checksums, then applies the forward migration. Fresh install continues through the repository's normal migration path. A migration test was added to preserve old dashboard rows and account/environment provenance and assert that the R2 structures remain empty after upgrade. Fresh-install and upgrade SQL behavior have not yet been executed because QOS-020 identity validation failed; see test evidence below.

## Access-path inventory

| Path | R2 behavior | Legacy boundary / result |
| --- | --- | --- |
| Trading accounts list and single account | Active membership; active entitlement filter in SQL | Legacy dashboard grants and owner strings ignored; unassigned/cross-tenant account is not found |
| Portfolio, positions, orders, strategies, risk summary/decisions, events, reconciliation | Active organization context and tenant-constrained entitlement joins | No unrestricted account lookup; role is read-only for these current endpoints |
| System components and pipeline | Active organization context; pipeline count is active entitlement count | No account list is disclosed without tenant selection |
| Create paper account | Active OWNER required; explicit account entitlement written with actor/reason | Never writes legacy dashboard grant; partial failure leaves an unassigned account |
| Organization list/create/update | JWT subject; create is authenticated; update is OWNER and revision guarded | List includes active memberships even for suspended/closed orgs; CLOSED is terminal |
| Members, member update, owner transfer | Active organization membership plus ADMIN/OWNER permission; expected revision | Owner change only via transfer; no membership deletion |
| Account entitlements | Active member can read; OWNER can mutate with expected revisions | Suspended/revoked current rows remain visible to organization members; no shared-account model |
| Journal accounts/setups/trades/imports/attachments/psychology/analytics/risk policies | Existing JWT-subject `owner_id` predicates retained | Personal Journal records are not organization tenancy; cross-user Journal collaboration remains unimplemented |
| E1-R1 venue and MT5 execution paths | No R2 changes intended | E1-R1 boundary and MT5 deny-all are out of scope; hash verification is recorded in final report |

## QOS-020 test and validation evidence

Non-database validation completed:

- `cargo check -p domain -p persistence -p journal-api --all-targets` — PASS.
- `cargo test -p domain` — PASS, 37 unit tests and 1 architecture integration test (38 total); includes the new organization model/revision tests.
- `cargo test -p journal-api` — PASS, 18 unit tests; includes the role/execution non-authority test.
- `cargo test -p persistence --lib` — PASS, 0 library unit tests.
- `cargo test -p persistence --test qos021a_e2_migrations --test qos021a_e3a_r2_tenant --no-run` — PASS; both database integration test binaries compile, but were not executed.
- `cargo clippy -p domain -p persistence -p journal-api --all-targets -- -D warnings` — PASS.
- Targeted `rustfmt --edition 2021 --config skip_children=true --check` over all R2 Rust files — PASS. Workspace-wide formatting is not claimed because the repository contains pre-existing formatting differences.

Database execution is BLOCKED. The authorized guard command was run for QOS-020 run `20261110-230504-ffb59dfd0ab4`; it failed before spawning any child test command:

```text
QOS-020 guard rejected command: CalledProcessError: Command '['docker', 'inspect', 'f6f4f45ee58930f5059845a58a023e3f2c2643e1770a0c8ef6f47cb596a0ad82']' returned non-zero exit status 1.
```

No SQLx test, migration, or database-capable test was run after the guard failed. Required evidence still includes fresh migration execution, supported E2 upgrade execution, all R2 persistence and cross-tenant tests, last-owner and duplicate-assignment concurrency tests, and successful `--verify-sqlx-cleanup` for a verified QOS-020 instance. Staging was not contacted and no Docker lifecycle command was run.

## Known gaps and next gates

1. Resolve/renew QOS-020 instance identity through its separately authorized provisioning process, then verify its manifest and endpoint with `qos020_guard.py`; do not fall back to staging or an unguarded local database.
2. Under the verified guard, run the new R2 tenant tests and migration suite, add/execute concurrency coverage for duplicate membership, last-owner race, entitlement assignment/reassignment race, and stale revision races, then verify SQLx cleanup.
3. Add API-level tests for JWT plus organization-header selection, cross-tenant ID probes, suspended/revoked membership and entitlement denial, and role matrix behavior.
4. Consider an atomic transaction for account creation plus entitlement to avoid an unassigned orphan on entitlement failure.
5. Define separate least-privilege runtime and migration PostgreSQL roles and RLS only as a later approved design; the current shared role and application predicates are not DB-level tenant isolation.
6. Keep the broader Journal domain user-private until a separate organization-sharing and migration decision is approved.

R2 is **not complete** until these gates pass. This phase does not authorize R3 or any broker, risk-issuance, execution-authorization, broker-connectivity, or live-trading work.

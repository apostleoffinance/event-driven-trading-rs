# M0 — Organization and Workspace Architecture Audit

**Status:** Complete — source inspection only  
**Repository:** apostleoffinance/event-driven-trading-rs  
**Baseline:** main at merge commit db57bfa16cb6036af292680dfe17d9be4251ec40 (PR #187)  
**Tracker:** https://github.com/apostleoffinance/event-driven-trading-rs/issues/188

## 1. Executive summary

EdgeNest has an organization tenancy foundation, but no first-class Workspace entity. Current tenant resolution is:

JWT subject → active organization membership → X-Organization-Id → organization-scoped authorization/query.

The proposed Workspace layer should be introduced as a child operational scope without replacing the existing organization authority model.

**M0 recommendation:** retain Organization as the ownership, membership and billing boundary; add Workspace as a child operational boundary; introduce explicit workspace access grants; and migrate only resources whose semantics are genuinely workspace-specific. Do not bulk-convert every organization- or user-owned table.

No schema, application code, production data or runtime configuration was changed during M0.

## 2. Current architecture observed

### 2.1 Domain model

The domain crate currently defines:

- Organization: ID, name, lifecycle status (ACTIVE/SUSPENDED/CLOSED), optimistic revision and timestamps.
- OrganizationMembership: organization ID, user ID, role (OWNER/ADMIN/TRADER/RISK_MANAGER/VIEWER), status (ACTIVE/SUSPENDED/REVOKED), revision and timestamps.
- OrganizationAccountEntitlement: organization ID, Quant account ID, status, revision, assigning actor, reason and timestamps.

There is no Workspace domain type or workspace identifier in the inspected organization domain model.

Source: crates/domain/src/organization.rs.

### 2.2 Organization persistence

Migration 20261112000002_organization_tenant_foundation.sql creates:

- organizations
- organization_events
- organization_memberships
- organization_membership_events
- organization_account_entitlements
- organization_account_entitlement_events

Integrity characteristics:

- Membership is unique by (organization_id, user_id).
- Membership and entitlement changes are revisioned and have append-only event history.
- Foreign keys use RESTRICT; membership and entitlement records cannot be ordinarily deleted.
- A database trigger protects the last active OWNER.
- The migration intentionally does not infer organizations, memberships or account entitlements from legacy owner strings or dashboard grants.
- No PostgreSQL row-level security (RLS) policy is introduced by this foundation.

Sources: the migration above and docs/QOS-021A-E3A-R2_ORGANIZATION_TENANT_FOUNDATION.md.

### 2.3 Organization creation and API contract

POST /api/v1/organizations authenticates the JWT subject, constructs an Organization, and calls TradingStore::create_organization. Persistence creates the organization, initial OWNER membership and organization audit history transactionally.

The current response contains organization and membership fields, but no workspace object or workspace identifier.

GET /api/v1/organizations returns organizations for which the authenticated user has an active membership. The frontend filters returned organizations by organization status.

Sources: crates/journal-api/src/organizations_api.rs, crates/persistence/src/repos/organizations.rs, frontend/src/features/organization/OrganizationProvider.tsx.

### 2.4 Request authorization and tenant selection

For trading routes, the frontend client attaches X-Organization-Id. The server:

1. Authenticates the JWT.
2. Reads the requested organization header.
3. Loads the caller's current organization membership.
4. Requires an ACTIVE membership.
5. Applies the organization-role permission for the route.
6. Executes tenant-constrained SQL.

The organization header is a selector, not proof of authorization. The backend rechecks membership; tenant isolation is not delegated to the frontend.

The current authorization principal contains user ID, organization ID and organization role. There is no active workspace context in this principal.

Sources: crates/journal-api/src/organizations_api.rs, crates/journal-api/src/dashboard.rs, frontend/src/lib/api/client.ts.

### 2.5 Organization role semantics

The domain defines OWNER, ADMIN, TRADER, RISK_MANAGER and VIEWER. Domain helper methods express broad capabilities, but the R2 documented route matrix is narrower than the role names might imply. Existing roles do not grant execution authority.

Workspace authorization must not assume a role label alone defines every workspace capability. The new policy must be explicit and tested against actual routes.

Sources: crates/domain/src/organization.rs and docs/QOS-021A-E3A-R2_ORGANIZATION_TENANT_FOUNDATION.md.

## 3. Resource scoping inventory

This matrix distinguishes observed ownership from the recommended future workspace boundary. “Verify” means a full resource-by-resource read/write audit remains necessary before M4.

| Resource family | Current observed scope / authority | M0 recommendation |
|---|---|---|
| User identity, credentials, sessions | User identity; JWT subject | Remain user-scoped. Never move credentials into Workspace. |
| Organization, membership, lifecycle events | Organization | Remain organization-scoped. |
| Quant trading accounts | Account row plus organization_account_entitlements; entitlement is the tenant assignment authority | Add explicit workspace assignment beneath the organization entitlement. Do not replace entitlement or infer ownership from labels. |
| Account state, positions, orders, performance snapshots, risk summaries, trading events and reconciliation views | Trading API queries derive visibility through active organization entitlement and account ID | Workspace-scope through the Quant account's workspace assignment; every query and aggregate must join through it. Inventory writers and event consumers in M4. |
| Paper-account creation | OWNER-only route; creates account and organization entitlement | Create in an explicit workspace; retain organization entitlement and make account plus assignment atomic. |
| Strategies | Strategy API exists and is exposed in dashboard; current query path is tenant/organization-context based | Determine whether strategy is a persisted user-configurable resource or computed/runtime view before designing persistence. |
| Broker catalogue (brokers, broker_servers) | Global controlled catalogue | Remain platform-global; not owned by organization or workspace. |
| External broker accounts and trading-account broker bindings | External identity is global; binding is tied to organization entitlement via organization ID + Quant account ID | Add workspace association at customer assignment layer only after deciding account placement. Preserve global identity and verification evidence. |
| MT5 gateway hosts | Infrastructure/operational registry | Remain platform/infrastructure scoped unless customer ownership is demonstrated. |
| MT5 terminal slots and worker assignments | Carry organization and Quant-account context | Workspace should be part of customer assignment/admission boundary, not ownership of shared gateway host. Audit admission queries and worker frames before changing. |
| Personal Journal accounts, setups, trades, imports, attachments, psychology and journal analytics | User-private via JWT subject / owner_id; not Quant trading account entitlement | Keep user-private. Do not silently migrate personal journals to organization or workspace scope. |
| Journal risk policies and saved views | User-private in current Journal model | Remain user-private until a separate shared-journal product decision. |
| Market data, instruments, venues and reference catalogues | Platform/global data | Remain global. Workspace may store preferences, not duplicate canonical reference data. |
| Strategy source code and Python strategy runtime | Repository/deployment/runtime assets; strategies emit TradeIntent | Treat as deployable artifacts/runtime, not automatically workspace-owned rows. Workspace may own configuration/assignment and execution context. |
| Events, jobs, caches, exports and analytics | Mixed; some account-derived, others infrastructure or user-scoped | Carry trusted organization/workspace context where operating on workspace resources. Never accept tenant identity solely from an untrusted event payload. |

### Journal boundary

The R2 architecture documentation explicitly states that Journal resources remain user-private and are not Quant trading accounts or organization entitlements. A Journal account may link to a trading account, but that does not make all personal journal records organization-owned. M4 must preserve this distinction unless a separate shared-journal feature is approved.

### Broker identity boundary

The broker/server catalogue and external broker-account identity are not the same as a customer's workspace assignment. A broker server is a shared platform catalogue record; the Quant account binding is the customer-controlled association. Workspace ownership should be applied at the latter boundary, not to the global catalogue.

## 4. Frontend and session state

The current organization provider loads organizations, stores the active organization ID in session storage under edgenest.organization.active.v1, selects an organization and clears the React Query cache. It does not load, select or persist a workspace.

The API client resolves organization context from session storage and attaches X-Organization-Id to organization/trading API calls. This is the contract M2–M5 must evolve carefully.

**M0 finding — inactive-membership detection:** the organization list endpoint only returns organizations where the user has an ACTIVE membership. The frontend hasMemberships flag is calculated from that returned list. Consequently, it cannot distinguish “no memberships exist” from “memberships exist but are suspended/revoked.” PR #187's intended inactive-membership distinction is not fully supported by this response contract. Before relying on this flag for security-sensitive onboarding decisions, add a separate safe membership-state signal or endpoint; do not disclose inaccessible organization details.

Sources: crates/persistence/src/repos/organizations.rs, crates/journal-api/src/organizations_api.rs, frontend/src/features/organization/OrganizationProvider.tsx.

## 5. Recommended target domain model

User
└── OrganizationMembership ── Organization
    ├── OrganizationAccountEntitlement ── Quant Account
    └── Workspace
        ├── WorkspaceMembership
        ├── WorkspaceAccountAssignment ── Quant Account
        ├── Strategy Configuration / Assignment
        ├── Workspace Preferences
        └── Workspace-scoped activity and analytics

Personal Journal remains user-private and separate.

### Organization

Continues to own organization identity and lifecycle, organization membership and top-level administration, Quant-account entitlement/commercial ownership relationship, billing and organization-level usage.

### Workspace

A child operational environment. Suggested initial fields:

- id — immutable stable identifier.
- organization_id — required parent.
- name — display name.
- slug — stable organization-local identifier, if useful for URLs.
- status — ACTIVE / ARCHIVED (or similarly explicit lifecycle).
- is_default — one default per organization.
- revision — optimistic concurrency for mutable settings.
- created_by, created_at, updated_at.

Do not make region, environment, broker or strategy type mandatory workspace identity fields. Those are configuration dimensions.

### Workspace membership / access grant

Recommendation: explicit workspace grants for non-administrative organization members.

- An active organization membership is a prerequisite; a workspace grant cannot bypass it.
- Organization OWNER and ADMIN can administer workspaces across their organization.
- Other organization members receive explicit workspace membership/role grants.
- Initial workspace roles should be small (e.g. WORKSPACE_ADMIN, TRADER, RISK_MANAGER, VIEWER), mapped to capabilities.
- Organization creator receives access to the default workspace as its initial workspace administrator.
- Suspension/revocation of organization membership immediately denies workspace access regardless of workspace grant.
- Every authorization decision checks current organization membership and current workspace grant (or documented owner/admin override).

This is a proposed policy, not current behavior.

### Workspace account assignment

Recommendation: retain organization_account_entitlements as organization-level authority and add a child assignment associating an entitled Quant account with a Workspace.

For the first release, prefer **one active workspace per Quant account**. This avoids ambiguous execution ownership, duplicate risk aggregation and cross-workspace exposure. If a single account must later be visible in multiple workspaces, design shared-resource semantics with centralized risk/execution ownership; do not duplicate account rows.

Assignment invariants:

- The assigned workspace must belong to the same organization as the entitlement.
- The account must have an ACTIVE entitlement for that organization.
- Account assignment changes must be audited and revision guarded.
- Historical trades/executions remain attached to the same Quant account; moving it changes operational placement, not historical identity.
- Reassignment should be blocked while unsafe active execution/position state exists, or require a reviewed transfer protocol. Design this in M4, not as an ad hoc SQL update.

## 6. Target request authorization model

Proposed request context:

JWT subject → active OrganizationMembership → selected Organization → active Workspace → WorkspaceMembership / owner-admin policy → resource query.

Proposed invariants:

1. Workspace ID is never accepted as authorization proof by itself.
2. Workspace must belong to the selected organization.
3. User must have active membership in that organization.
4. Organization must permit the requested operation.
5. User must have an effective workspace grant or documented OWNER/ADMIN override.
6. Resource query must constrain by workspace (and organization where useful for defense in depth).
7. Cross-workspace resource IDs return a non-disclosing not-found/denied response consistent with existing API policy.
8. Background jobs and event consumers resolve workspace from trusted persisted assignment/context, not caller-controlled payload fields.
9. Cache keys, idempotency keys and analytics grouping include workspace identity wherever results are workspace-specific.
10. Archiving a workspace never erases financial or audit history.

The existing X-Organization-Id is a client selector validated by the backend. M3 should add a workspace selector (header or another explicit request-context mechanism) while retaining independent server-side membership and ownership checks. Choose one canonical transport contract and apply it across gateway, CORS, services, tests and frontend.

## 7. API and lifecycle recommendations

Illustrative API shape; final conventions are confirmed in M3:

- GET /api/v1/organizations/{organization_id}/workspaces
- POST /api/v1/organizations/{organization_id}/workspaces
- GET /api/v1/organizations/{organization_id}/workspaces/{workspace_id}
- PATCH /api/v1/organizations/{organization_id}/workspaces/{workspace_id}
- POST /api/v1/organizations/{organization_id}/workspaces/{workspace_id}/archive
- POST /api/v1/organizations/{organization_id}/workspaces/{workspace_id}/members (if explicit grants are adopted)
- Workspace-scoped resource routes or existing routes with validated workspace request context.

Lifecycle rules:

- Organization creation and default-workspace provisioning are one database transaction with initial owner membership and workspace grant.
- An organization has exactly one active default workspace.
- An organization always retains at least one usable workspace.
- Archiving the default workspace requires atomically selecting a replacement.
- Archived workspaces are read-only for ordinary users; historical records remain queryable under explicit audit/read permissions.
- Do not hard-delete organizations, workspaces, memberships, assignments or financial/audit records.
- Workspace names can change; immutable IDs and stable audit references cannot.

## 8. Migration and rollout constraints

The R2 migration deliberately did not backfill legacy users, organizations, memberships or account entitlements. Do not assume there is production data to backfill from legacy owner strings.

M1 must:

- Add Workspace and required constraints additively.
- Create one default workspace for each existing organization.
- Preserve organization IDs, memberships, entitlements, account IDs, broker bindings and all history.
- Assign each currently entitled Quant account to its organization's default workspace.
- Follow the repository migration runner/checksum conventions.
- Provide preflight counts and post-migration invariant checks.
- Use a verified database environment and documented backup/restore plan.
- Avoid claiming rollback is safe after new workspace records are written unless reverse semantics are lossless.

M2 must update organization creation to create the default workspace and initial access grant atomically. A failed workspace insert rolls back organization and membership creation.

M4 should use staged expand/backfill/dual-read-or-write/cutover/contract steps per resource family. Do not make all resource tables workspace-required in one migration before every writer and consumer is upgraded.

## 9. Decisions to carry into M1–M3

| Decision | M0 recommendation | Reason |
|---|---|---|
| Organization vs Workspace | Separate entities; Workspace is child of Organization | Ownership/billing and operational isolation differ |
| Default workspace | Exactly one per organization | Predictable onboarding and backward-compatible migration |
| Workspace access | Explicit grants for non-OWNER/ADMIN members | Avoid accidental visibility across research/live/prop environments |
| Owner/admin access | Organization OWNER/ADMIN can administer all workspaces | Preserve governance and recoverability |
| Account assignment | One active workspace per Quant account initially | Clear risk, execution, reporting and audit ownership |
| Account entitlement | Retain organization entitlement as authority | Separate legal/commercial ownership from operational placement |
| Journal | Keep user-private | Existing architecture intentionally excludes it from organization tenancy |
| Broker catalogue | Keep global | Catalogue identity is not customer workspace data |
| Broker binding | Workspace association at customer assignment layer | Preserve global external broker identity |
| Gateway infrastructure | Platform-level; workspace-aware customer assignment | Shared infrastructure is not a tenant-owned workspace resource |
| Workspace deletion | Archive only; preserve history | Financial/audit traceability |
| Billing | Organization-level | One commercial boundary can contain multiple workspaces |
| Workspace transport | One canonical server-validated workspace context | Avoid inconsistent header/query/path semantics |

## 10. Open questions / gates before schema work

1. **Workspace membership policy:** confirm OWNER/ADMIN override plus explicit grants for other members.
2. **Account placement:** confirm one active workspace per Quant account for v1, and define handling of open positions/orders during reassignment.
3. **Strategies:** determine whether current strategy objects are persisted configurations, runtime processes or computed dashboard views; map actual tables and APIs.
4. **Journal integration:** confirm personal journals remain private and workspace-linked journaling is a separate future capability.
5. **Workspace selector contract:** choose header vs route/path context and define gateway propagation/CORS behavior.
6. **Default lifecycle:** confirm one active default workspace and replacement rules for archival.
7. **Legacy data:** confirm actual database migration state and counts through authorized DB access before M1. Do not infer production data from source migrations.
8. **R2 verification:** the existing R2 report states database migration/integration/concurrency acceptance was not completed because the authorized QOS-020 database guard could not verify its instance. Resolve independently; do not use an unverified database or staging fallback.

## 11. M0 completion checklist

- [x] Inspected current domain, persistence, migrations, API authorization, frontend organization context and architecture reports.
- [x] Documented current organization tenancy and API request flow.
- [x] Built resource-scope inventory separating organization, user, platform and candidate workspace data.
- [x] Identified Quant-account entitlement as existing organization assignment authority.
- [x] Identified Journal data as user-private and excluded it from automatic migration.
- [x] Defined recommended workspace access and account-assignment policy for review.
- [x] Documented migration, authorization, event, cache and rollout constraints.
- [x] Identified mismatch in current inactive-membership detection.
- [ ] Obtain product confirmation for explicit workspace membership and one-workspace-per-account policies before M1.
- [ ] Resolve existing R2 database verification gate before production migration.

**M0 conclusion:** architecture discovery is complete. M1 can be designed against this target model, but schema implementation should wait for the two product-policy confirmations and a verified migration test environment.

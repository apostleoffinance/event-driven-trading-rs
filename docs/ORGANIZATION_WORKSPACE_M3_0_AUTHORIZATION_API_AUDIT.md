# M3.0 — Authorization and API audit

**Status:** Complete — source inspection only  
**Audited branch:** `main`  
**Audited commit:** `dc0a858243054c17291d8c3bc128acb401d1ad28` (merge of PR #191)  
**Parent tracker:** [Issue #188](https://github.com/apostleoffinance/event-driven-trading-rs/issues/188)  
**Scope:** Authentication, organization authorization, workspace persistence, API routing, frontend tenant selection, resource query boundaries, CORS and existing test coverage.

> This is an implementation-readiness audit. It does not add workspace endpoints, schema, authorization behavior or frontend state. No production database was accessed or modified.

## 1. Executive summary

EdgeNest has a functioning identity/session layer and an organization-level tenancy boundary. The M1/M2 work has introduced a persisted Workspace entity and atomic default-workspace provisioning. However, **Workspace is not yet an authorization boundary**.

The current API principal is organization-scoped. It contains the authenticated user, selected organization and organization role; it has no workspace identity or effective workspace permission. There is no workspace membership/access-grant table, no workspace HTTP API, no active-workspace request context and no workspace-aware authorization middleware. Trading dashboard SQL still scopes account-backed data through active organization account entitlements, not workspace account assignments.

M3 must introduce workspace access without weakening the existing organization authority model. Organization membership remains a prerequisite. Workspace selection is untrusted input and must be validated against the organization, the user's current membership and the user's effective workspace grant (or an explicit organization OWNER/ADMIN override). Resource queries must then use the validated workspace context.

**Recommendation:** use explicit workspace grants for non-OWNER/ADMIN organization members; preserve OWNER/ADMIN cross-workspace administration; derive effective capabilities as the intersection of organization-level authority and workspace-level grant; add a validated `X-Workspace-Id` request selector for existing workspace-scoped trading routes; keep organization/workspace path IDs consistent on management routes. Final product policy decisions are listed in §10 and must be confirmed before M3 schema/API implementation.

## 2. Current authentication and request authorization

### 2.1 Identity and session validation

Observed request path:

1. Browser obtains an access token through the Identity API.
2. The token carries `sub`, issuer, audience, expiry, token type, `jti`, session ID and session revision.
3. Journal API verifies the JWT signature/algorithm, issuer, audience and access-token claims.
4. The Journal API `session_guard` validates the token against database-backed user, session and access-token state before forwarding an `/api/*` request to a handler.
5. Session suspension/revocation and access-token revocation are checked server-side; organization membership and role are not embedded as trusted JWT authorization claims.

This is a sound basis for dynamic authorization: organization and future workspace grants can be re-evaluated from current database state rather than waiting for an old role claim to expire.

Sources:
- `crates/identity-token/src/lib.rs`
- `crates/journal-api/src/auth.rs`
- `crates/journal-api/src/main.rs` — `session_guard`
- `crates/persistence/src/repos/identity_sessions.rs`
- `migrations/20261114000001_identity_sessions_and_tokens.sql`
- `migrations/20261115000001_identity_suspension_and_audit_hardening.sql`

### 2.2 Organization tenant resolution

Current organization request path:

`Bearer JWT → session_guard → X-Organization-Id → current organization membership → organization role check → organization-constrained handler/query`

The header is a selector, not authorization proof. `TradingStore::organization_membership` joins the organization row and only returns a membership when the organization is ACTIVE. The handler additionally requires membership status ACTIVE. Suspended/revoked memberships and non-active organizations are therefore rejected by the normal tenant-context path.

The API principal currently contains:

- `user_id`
- `organization_id`
- `role` (`OrganizationRole`)

It does not contain a workspace ID, workspace role, grant revision or effective capability set.

Sources:
- `crates/journal-api/src/organizations_api.rs` — `TenantPrincipal`, `tenant_context`, `authorize`
- `crates/persistence/src/repos/organizations.rs` — `membership`
- `frontend/src/lib/api/client.ts` — organization header resolution

### 2.3 Current organization role policy

The domain defines `OWNER`, `ADMIN`, `TRADER`, `RISK_MANAGER` and `VIEWER`.

Current API-level permission checks are coarse:

- `Permission::Read`: any active organization member.
- `Permission::Admin`: OWNER or ADMIN.
- `Permission::Owner`: OWNER only.

The domain also defines `may_write_trading_workspace` and `may_manage_risk_configuration`, but these helpers are not a complete route-level policy engine. Do not assume that a role name or helper automatically grants a future workspace capability. M3 needs an explicit capability matrix and tests for every route.

Observed route policy:

| API surface | Current authorization |
|---|---|
| List organizations | Authenticated user; returns organizations with active membership rows |
| Create organization | Authenticated user; creator becomes initial OWNER |
| Organization member list/create/update | OWNER or ADMIN at handler; persistence further restricts ADMIN from changing OWNER/ADMIN authority |
| Organization owner transfer | OWNER |
| Organization lifecycle update | OWNER; special lifecycle path permits access to an ACTIVE or SUSPENDED organization so an owner can restore it |
| Account entitlement list | Any active organization member |
| Assign/change account entitlement | OWNER |
| Trading dashboard reads (accounts, portfolio, positions, orders, strategies, risk, risk decisions, events, reconciliation) | Any active organization member; data is constrained through active organization account entitlements |
| Paper account creation | OWNER |
| Broker binding list/catalog | Any active organization member |
| Broker binding registration/suspension/revocation | OWNER; ordinary organization roles cannot approve a binding as VERIFIED |
| Personal Journal APIs | Authenticated user; resources are owner/user-scoped rather than organization-shared |
| Market/reference data | Platform/global service scope; not an organization workspace resource |

The route matrix is based on the current handlers in `crates/journal-api/src/main.rs`, `organizations_api.rs`, `dashboard.rs` and `broker_registry.rs`.

## 3. Workspace persistence and authorization readiness

### 3.1 What exists

M1 provides:

- `workspaces`, with organization FK, name, slug, ACTIVE/ARCHIVED status, default flag, revision and creator.
- Exactly one active default workspace per organization, enforced by database constraints/triggers.
- Append-only `workspace_events`.
- `workspace_account_assignments`, with one workspace assignment per Quant account and composite foreign keys that ensure the assignment's organization matches both the entitlement and workspace.
- Append-only `workspace_account_assignment_events`.
- Domain validation and repository methods for list, get, default lookup, account lookup and create.

M2 makes organization, initial OWNER membership and the database-provisioned default workspace part of one transaction and returns `data.defaultWorkspace`.

Sources:
- `migrations/20261125000001_workspace_persistence.sql`
- `crates/domain/src/workspace.rs`
- `crates/persistence/src/repos/workspaces.rs`
- `crates/persistence/src/repos/mod.rs`
- `crates/persistence/src/repos/organizations.rs`
- `crates/journal-api/src/organizations_api.rs`

### 3.2 What does not exist

The current source has no:

- `workspace_memberships` or equivalent user-to-workspace access-grant table.
- Workspace role enum or capability policy.
- Workspace-aware API principal/request context.
- Workspace selector validation.
- Workspace list/get/create/update/archive/default HTTP endpoints.
- Workspace access-grant management endpoints.
- Workspace-aware API authorization tests.

The repository `list_for_organization` and `get_for_organization` methods constrain a lookup by organization ID, but do not authenticate a user or evaluate a workspace grant. They are persistence primitives, not an authorization boundary. `create` checks that the creator is an active organization OWNER/ADMIN, but there is no member-grant operation or grant lifecycle.

The M2-created default workspace does not have a separate grant row. This is compatible with the proposed OWNER override, but it means M3 must define the override explicitly and provide a grant model for other members.

### 3.3 Database isolation

The current migration set contains no PostgreSQL RLS enablement or policies. Tenant enforcement is therefore application-level: handlers and SQL queries must apply the validated tenant predicates consistently. M3 must not imply that a workspace ID or a foreign key alone creates row-level authorization.

RLS may be evaluated as a separate defense-in-depth decision, but it must not be introduced casually without a trusted transaction-local identity/context mechanism, connection-pool safety tests and a migration/operations plan.

## 4. API routing and frontend context

### 4.1 Journal API routing

The Journal API registers organization, broker-registry, trading-dashboard and personal-Journal routes. There are no workspace routes. A router-level session guard covers API routes, but it only authenticates the session; organization/workspace authorization remains handler-level.

The API gateway routes `/api/v1/organizations/*`, `/api/v1/trading/*` and `/api/v1/journal/*` to Journal API. It forwards request headers to the upstream service, so no custom workspace-header forwarding code is currently required.

### 4.2 CORS gap

Both the gateway and Journal API currently allow `X-Organization-Id` but not `X-Workspace-Id`.

When M3 adopts the workspace selector, update all of the following together:
- Gateway `CORS_ALLOWED_HEADERS`.
- Gateway `preflight_is_allowed`.
- Gateway preflight response allow-header value and tests.
- Journal API `CorsLayer::allow_headers`.
- Any deployment/API integration documentation and browser contract tests.

A handler may work in direct service tests while browser preflight still blocks the real frontend request. Treat this as an acceptance requirement, not a follow-up cleanup.

Sources:
- `crates/api-gateway/src/main.rs` — CORS allow-list and proxy
- `crates/journal-api/src/main.rs` — Journal API CORS and route table
- `frontend/src/lib/api/client.ts`
- `frontend/src/features/organization/OrganizationProvider.tsx`

### 4.3 Frontend state

The frontend currently persists only `edgenest.organization.active.v1` in session storage. It has no workspace provider, active workspace key, workspace list query or workspace cache boundary.

Organization switching clears the React Query cache. M5 must preserve that behavior for workspace switching and ensure query keys are workspace-qualified (or that all workspace-scoped cache entries are invalidated) so data from the previous workspace cannot remain visible.

The organization provider also calculates `hasMemberships` from the list of active-membership rows while separately filtering organizations by ACTIVE status. This does not distinguish “no membership rows” from “membership rows exist but all organizations are suspended/closed.” This is an existing onboarding-state accuracy issue noted in M0; it is not evidence of an authorization bypass, and should be corrected separately from workspace authorization.

## 5. Resource-scope and query inventory

| Resource family | Current scope/authority | M3/M4 treatment |
|---|---|---|
| Identity users, credentials, sessions, access tokens | User identity/session | Remain user-scoped; never workspace-owned |
| Organizations, memberships, lifecycle events | Organization | Remain organization-scoped |
| Organization account entitlements | Organization/account | Retain as ownership/commercial authority |
| Quant accounts, account states, positions, orders, performance snapshots, risk decisions, execution events and reconciliation snapshots | Trading dashboard reads join through active organization entitlements | M3 establishes validated workspace context; M4 must add workspace assignment predicates to every applicable query and aggregate |
| Broker/server catalogue | Platform-global | Remain global |
| External broker identity and broker bindings | Global external identity plus organization/account binding | Add workspace placement at the customer assignment boundary only after M4 admission/worker review |
| MT5 gateway hosts and shared infrastructure | Platform/infrastructure | Do not make workspace-owned |
| MT5 terminal slots, worker assignments and execution admission | Infrastructure plus organization/account context | M4 must carry trusted workspace context through admission and worker lifecycle; never trust event payload tenant IDs alone |
| Strategies and strategy deployments | Current dashboard strategy deployments are derived through account entitlement; strategy catalogue is shared | M4 must distinguish global strategy definition from workspace-owned deployment/configuration |
| Personal Journal accounts, trades, setups, attachments, psychology, saved views and journal risk policies | User-private via authenticated subject/owner ID | Remain user-private; do not silently move them into workspace scope |
| Instruments, venues and market/reference data | Platform/global | Remain global |
| Events, jobs, caches, exports and analytics | Mixed | M4 must classify each producer/consumer and propagate trusted workspace context wherever the data is workspace-specific |

### Trading query evidence

The current dashboard SQL uses `organization_account_entitlements` for account visibility. This is true for account listing, portfolio aggregates, positions, orders, strategy deployments, risk summaries, risk decisions, execution events and reconciliation snapshots. It does not yet join `workspace_account_assignments`.

This is expected before M4, but it means workspace selection in M3 alone must not be presented as data isolation. M3 can authorize a selected workspace; M4 must make each resource query actually honor it before workspace isolation is complete.

### Personal Journal boundary

The existing Journal repository/API uses the authenticated user's identity for private Journal records. Keep this boundary unchanged. A future shared workspace journal is a separate product and data-ownership decision.

## 6. Findings and risks

| ID | Severity | Finding | Consequence / required response |
|---|---|---|---|
| M3-AUTH-01 | BLOCKER | No workspace membership/access-grant model exists | Non-admin workspace access cannot be expressed or revoked independently. Add explicit grants or formally approve a different policy before exposing workspace APIs. |
| M3-AUTH-02 | BLOCKER | No workspace context exists in the request principal or trading API | Existing requests are organization-scoped only. Add one validated workspace context before any workspace-scoped operation is exposed. |
| M3-AUTH-03 | BLOCKER for isolation claim | Trading queries use organization entitlements, not workspace assignments | A selected workspace would not yet filter account-backed data. M4 must migrate every relevant read/write/query/aggregate/consumer before claiming workspace data isolation. |
| M3-API-04 | HIGH | Workspace endpoints are not registered | M3 must add authenticated list/get/create/update/archive/default and access-grant APIs with consistent path/organization validation. |
| M3-API-05 | HIGH | CORS does not allow a workspace selector | Browser requests using `X-Workspace-Id` would fail preflight until gateway and Journal API policies are updated together. |
| M3-TEST-06 | HIGH | No workspace authorization or API-level negative tests exist | Add cross-organization, cross-workspace, revoked/suspended membership, archived workspace and role-escalation tests before merging M3. |
| M3-DATA-07 | MEDIUM | Workspace repository methods are not user-authorized | Keep authorization in a dedicated service/policy layer and repeat critical membership/ownership checks transactionally for writes. Do not expose raw repository calls as API authorization. |
| M3-POLICY-08 | MEDIUM | Existing organization roles and route permissions are coarse and partly unused | Define explicit capabilities for workspace read, trading, risk, settings, membership, default selection and archive. Do not infer all permissions from role labels. |
| M3-UX-09 | LOW | Frontend has organization state only and an existing inactive-membership signal ambiguity | M5 must add workspace state/cache handling; separately correct membership-state UX without exposing inaccessible organization details. |

No confirmed cross-tenant bypass was established by this source audit of the current organization-scoped routes. The central issue is that workspace-level isolation has not yet been implemented; do not label the current organization-level model as workspace-isolated.

## 7. Recommended target authorization model

### 7.1 Trust chain

`JWT subject → active identity/session → active organization → active organization membership → selected active workspace in that organization → effective workspace access → workspace-scoped resource query`

Required invariants:

1. JWT subject is the only caller identity source; never accept a user ID from request JSON as the actor.
2. `X-Organization-Id` and `X-Workspace-Id` are untrusted selectors, not credentials.
3. Organization membership must be ACTIVE and the organization must be ACTIVE.
4. The workspace must exist, be owned by the selected organization and be in a lifecycle state that permits the requested operation.
5. OWNER/ADMIN override, if approved, must be explicit and checked against current database membership.
6. Other organization members must have an active workspace grant.
7. A workspace grant cannot elevate the caller beyond the caller's organization-level authority.
8. Every workspace resource query must include the validated workspace ID and, where appropriate, organization ID as defense in depth.
9. A resource ID from another workspace or organization must not reveal whether that resource exists; use the established non-disclosing not-found policy.
10. Membership suspension/revocation or workspace-grant revocation must take effect on the next authorized request; do not rely on long-lived JWT role claims.
11. Background workers, event consumers, exports, caches and analytics must derive workspace ownership from trusted persisted assignments/context, not client-controlled payloads.
12. Workspace archive/default changes must be transactional and preserve financial/audit history.

### 7.2 Proposed role/capability composition

Recommended default, subject to product confirmation:

- Organization OWNER and ADMIN: can discover and administer all workspaces in their organization.
- Other active organization members: can access only workspaces with an active explicit grant.
- Workspace roles should be small and capability-oriented (for example WORKSPACE_ADMIN, TRADER, RISK_MANAGER, VIEWER); they do not replace organization roles.
- Effective permission is the intersection of organization-level permission and workspace-grant permission. A workspace grant must never promote a VIEWER to an organization ADMIN or grant account entitlement/owner-transfer authority.
- Keep organization OWNER transfer, organization membership, billing and account entitlement authority at organization scope.

Do not add a separate workspace OWNER unless a concrete governance requirement is approved. Organization ownership should remain authoritative.

### 7.3 Request context contract

Recommended canonical contract:

- Keep `X-Organization-Id` as the organization selector.
- Add `X-Workspace-Id` for existing operational routes whose data is workspace-scoped (initially `/api/v1/trading/*`).
- Management routes remain nested under `/api/v1/organizations/{org}/workspaces[/ {workspace}]`; require the path organization to match the validated organization context and require the workspace to belong to that organization.
- Do not accept workspace ID simultaneously from header, query and body. There must be one canonical selector per route class.
- The server resolves an immutable `WorkspacePrincipal` (user, organization, workspace, organization role, optional workspace role/grant revision, effective capabilities) only after all checks pass.
- Missing workspace context on workspace-scoped trading routes should return a consistent 409/400 context-required response; invalid or inaccessible workspace IDs should use the non-disclosing not-found policy.
- Organization-only routes (organization membership, entitlement administration, organization billing) must not be forced to select a workspace.

The API gateway already forwards request headers. The CORS allow-lists and frontend client must be updated for the new selector.

## 8. Proposed M3 API surface

Final routes must follow the existing Axum/API naming conventions, but the minimum contract should cover:

| Operation | Proposed route | Policy |
|---|---|---|
| List accessible workspaces | `GET /api/v1/organizations/{org}/workspaces` | Return only accessible workspaces; OWNER/ADMIN may see all |
| Create workspace | `POST /api/v1/organizations/{org}/workspaces` | OWNER/ADMIN; transactionally create workspace and CREATED event |
| Get workspace | `GET /api/v1/organizations/{org}/workspaces/{workspace}` | Active grant or OWNER/ADMIN override |
| Update workspace | `PATCH /api/v1/organizations/{org}/workspaces/{workspace}` | OWNER/ADMIN or explicitly delegated workspace administrator, with revision check |
| Archive workspace | `POST /api/v1/organizations/{org}/workspaces/{workspace}/archive` | OWNER/ADMIN; reject unsafe default/last-workspace archival |
| Set default workspace | `POST /api/v1/organizations/{org}/workspaces/{workspace}/default` | OWNER/ADMIN; atomic replacement and audit event |
| List/manage workspace grants | Nested `/workspaces/{workspace}/members` routes | OWNER/ADMIN initially; explicit role/target constraints |

The API contract must define request/response schemas, revision semantics, idempotency/retry behavior, error codes and whether archived workspaces are visible for audit reads. Do not expose a write endpoint without its authorization policy and negative tests.

## 9. M3 implementation gates and test plan

### M3.1 — Policy and schema

- Confirm the policy decisions in §10.
- Add workspace grant domain types, persistence, constraints and append-only grant events.
- Enforce unique active grant per (workspace, user), organization/workspace consistency and organization-membership dependency.
- Ensure organization membership suspension/revocation immediately makes grants ineffective, even if grant rows remain for audit.
- Seed/establish the initial OWNER override consistently for new and existing organizations.
- Define grant role transitions, actor permissions, optimistic revisions and revocation semantics.

### M3.2 — Authorization service and API

- Introduce a single workspace-context resolver/policy service; avoid reimplementing slightly different checks in each handler.
- Resolve organization and workspace from one canonical request contract.
- Validate path/header consistency and workspace ownership.
- Implement list/get/create/update/archive/default and grant-management endpoints.
- Re-check authority in the transaction for writes to avoid time-of-check/time-of-use races.
- Return non-disclosing errors for cross-tenant identifiers.

### M3.3 — Negative security tests

At minimum:
- Missing/invalid/expired/revoked access token.
- Suspended identity session.
- Missing organization selector.
- Missing workspace selector on a workspace-scoped route.
- Active user with no organization membership.
- Suspended/revoked organization membership.
- Suspended/closed organization.
- Workspace ID belonging to another organization.
- Same-organization workspace with no grant.
- Revoked workspace grant.
- Organization OWNER/ADMIN override.
- Workspace grant cannot elevate organization VIEWER/TRADER/RISK_MANAGER authority.
- Archived workspace read/write behavior.
- Cross-workspace resource ID lookup is non-disclosing.
- Organization member revocation immediately denies workspace access.
- Concurrent grant revocation and workspace request.
- Workspace default replacement/archive race and last-usable-workspace invariant.
- Browser CORS preflight accepts `X-Workspace-Id` and rejects unapproved headers.

### M3.4 — Verification

Run and record:
- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Frontend lint, typecheck and production build.
- Database migration tests under the repository's authorized database guard.
- API-level authorization integration tests against a verified test database.
- Gateway + Journal CORS and production acceptance tests.

M3 exit requires all applicable checks green and an explicit test proving cross-workspace and cross-organization access denial. M3 completion alone does not complete M4 resource isolation.

## 10. Product/architecture decisions required before M3.1

The source audit cannot safely make these product decisions on behalf of the product owner. Recommended defaults are included.

| Decision | Recommended default | Why it must be confirmed |
|---|---|---|
| Workspace access model | OWNER/ADMIN implicit access to all; explicit grants for TRADER/RISK_MANAGER/VIEWER | Determines whether a workspace-grant table is required and the onboarding behavior |
| Workspace role set | WORKSPACE_ADMIN, TRADER, RISK_MANAGER, VIEWER; effective permission is intersection with organization role | Avoids privilege escalation and role explosion |
| Workspace creation/update/archive/default authority | OWNER/ADMIN only for initial release | Simple governance; delegated workspace admins can follow later |
| Workspace selector | `X-Workspace-Id` on operational routes; nested IDs for management routes | One consistent contract for frontend and API |
| No-workspace context | Reject workspace-scoped requests with context-required response; never silently choose a default on behalf of a multi-workspace user | Prevents ambiguous account/resource selection |
| Default workspace access | Initial organization OWNER has implicit access; no separate grant row required for override | Consistent with organization authority |
| Account placement | Retain one active workspace per Quant account for v1 | Clear execution, risk and reporting ownership |
| Archived workspace reads | Read-only audit access for users who had access, subject to current organization membership; no writes | Preserve history without retaining operational access |
| Journal | Remain user-private; no automatic workspace grants | Existing data ownership contract |
| RLS | Do not add in M3 unless separately designed and tested | Application authorization remains the source of truth for this milestone |

If any recommendation is rejected, record the replacement policy in an ADR before schema work begins.

## 11. M3.0 completion checklist

- [x] Audited JWT verification and DB-backed session/access-token validation.
- [x] Audited organization selection, membership lookup and role authorization.
- [x] Enumerated organization, trading, broker, Journal and platform API route classes.
- [x] Inspected workspace schema, domain model, repository façade and M2 provisioning.
- [x] Confirmed no workspace grant model, request context or HTTP endpoints exist.
- [x] Inspected gateway and Journal API CORS/header contracts.
- [x] Inspected frontend organization/session state and cache invalidation behavior.
- [x] Mapped organization-entitlement-scoped trading dashboard queries and the M4 gap.
- [x] Separated personal Journal and global platform data from candidate workspace-owned resources.
- [x] Identified authorization, API, CORS and test gaps with severity and implementation response.
- [x] Defined recommended target trust chain, route contract and negative-test plan.
- [x] Listed product/architecture decisions that must be confirmed before M3.1.
- [x] No application code, schema or production data changed during this audit.

**M3.0 conclusion:** audit complete. The next safe implementation step is M3.1 — confirm the workspace access policy and implement its domain/schema foundation. Do not start M4 resource migration or claim workspace isolation until M3 authorization and M4 query enforcement are both complete.

# Organization Workspace M3–M4 Implementation

## Scope

M3 introduces authoritative workspace request authorization and workspace/grant management APIs. M4 enforces workspace placement across account-backed trading reads and broker-worker execution paths.

Organization remains the identity, membership, billing and account-entitlement boundary. Workspace is the operational boundary for Quant trading resources. Personal Journal records remain user-private; broker catalogues and platform infrastructure remain global/platform-scoped.

## Authorization contract

Every request to `/api/v1/trading/*` must include:

- `Authorization: Bearer <access-token>`
- `X-Organization-Id: <organization-id>`
- `X-Workspace-Id: <workspace-id>`

The headers select a context; neither header proves access. The Journal API resolves the JWT subject and checks the current database state: active session, active organization, active membership, active workspace in that organization, and the current workspace grant. Missing workspace context is rejected. Inaccessible or cross-organization workspaces return a non-disclosing not-found response. The server never silently chooses a default workspace.

### Role intersection

| Organization role | Workspace role | Effective authority |
|---|---|---|
| OWNER / ADMIN | Implicit | All active workspaces; organization-level administration remains governed by the organization role |
| TRADER | WORKSPACE_ADMIN | Read and trade; operational workspace administration, but no grant, billing, ownership or entitlement administration |
| TRADER | TRADER | Read and trade |
| RISK_MANAGER | WORKSPACE_ADMIN / RISK_MANAGER | Read and risk-management capabilities; no trading execution |
| Any non-admin | VIEWER | Read only |
| Any non-admin | No active grant | No workspace access |

Workspace grants cannot elevate organization-level authority. OWNER/ADMIN do not receive grant rows. Membership suspension/revocation and promotion to OWNER/ADMIN revoke prior grants; demotion does not resurrect them.

The request principal is resolved from PostgreSQL, not stale JWT role claims. Trading dashboard SQL also rechecks current organization, membership and grant state in the same statement as the data read to avoid a resolver/query time-of-check-to-time-of-use gap.

## Workspace API

All organization-scoped paths require a matching `X-Organization-Id` and an active organization membership.

| Method | Endpoint | Authority |
|---|---|---|
| GET | `/api/v1/organizations/{org}/workspaces` | Active member; returns only accessible active workspaces |
| POST | `/api/v1/organizations/{org}/workspaces` | Organization OWNER/ADMIN |
| GET | `/api/v1/organizations/{org}/workspaces/{workspace}` | Active workspace grant or implicit OWNER/ADMIN access; header must match path |
| PATCH | `/api/v1/organizations/{org}/workspaces/{workspace}` | Organization OWNER/ADMIN |
| POST | `/api/v1/organizations/{org}/workspaces/{workspace}/archive` | Organization OWNER/ADMIN; workspace must not be default or contain assigned accounts |
| POST | `/api/v1/organizations/{org}/workspaces/{workspace}/default` | Organization OWNER/ADMIN |
| GET | `/api/v1/organizations/{org}/workspaces/{workspace}/members` | Organization OWNER/ADMIN |
| POST | `/api/v1/organizations/{org}/workspaces/{workspace}/members` | Organization OWNER/ADMIN |
| PATCH | `/api/v1/organizations/{org}/workspaces/{workspace}/members/{user}` | Organization OWNER/ADMIN |
| PUT | `/api/v1/organizations/{org}/workspaces/{workspace}/accounts/{account}/assignment` | Organization OWNER/ADMIN |

Workspace and account-assignment mutations use optimistic revisions and append-only history. Organization authority is rechecked under transaction locks for workspace lifecycle and assignment mutations. Grant writes are protected by database-side grantor and target-membership checks.

Workspace slugs are immutable after creation because the current append-only workspace event schema records names, status and default transitions but not slug transitions. Workspaces are archived, never deleted. A workspace cannot be archived while it has account assignments; move accounts first.

## M4 resource isolation

The Quant account's `workspace_account_assignments` row is the operational placement authority. Organization entitlement remains the ownership/access authority.

Trading account lists and details, portfolio aggregates, performance series, positions, orders, strategy deployments, risk summaries, risk decisions, execution events, reconciliation views and system pipeline account aggregates are filtered by:

1. active organization entitlement;
2. the account's current workspace assignment;
3. active organization and workspace;
4. current active membership; and
5. implicit OWNER/ADMIN access or a current active workspace grant.

New organization account entitlements and paper accounts receive a workspace assignment and append-only assignment event in the same transaction. Paper-account creation uses the explicitly selected workspace. Same-organization account reassignment is revisioned and audited.

Cross-organization account transfer is rejected while workspace assignment identity is immutable. A dedicated transfer protocol must atomically coordinate source and destination ownership, workspace placement and audit history before that capability is re-enabled.

### Background broker and MT5 paths

Broker account-binding registration requires an active entitlement, active workspace assignment and active workspace, and locks the assignment while registering.

Broker admission requires a current active workspace assignment and a trade-capable actor: implicit OWNER/ADMIN, or an organization TRADER with an active TRADER/WORKSPACE_ADMIN grant for the account's assigned workspace.

MT5 worker lease acquire, renew and validate recheck the active organization, entitlement, Quant account, workspace assignment, workspace status, worker service-principal membership and trade-capable workspace grant. Worker service principals should use an active organization TRADER membership and an active TRADER grant for the account's current workspace; do not grant OWNER/ADMIN solely to bypass workspace authorization.

## Frontend contract

The frontend exposes organization and workspace selectors. A single unambiguous accessible workspace may be selected automatically; multiple accessible workspaces require explicit selection. The selected workspace is held in session storage and propagated as `X-Workspace-Id` on trading API calls.

Changing organization clears the selected workspace. Changing workspace clears React Query caches and scopes/resets the event stream so data from the previous workspace is not retained in the active view. The API client refuses to guess when multiple workspaces are available.

## Verification

The implementation is validated through Rust formatting, compilation, Clippy, PostgreSQL integration tests, workspace API isolation tests, MT5 worker/admission contracts, gateway CORS tests, frontend lint/typecheck/build and production API/viewport acceptance. Merge is gated on the latest successful run for this branch.

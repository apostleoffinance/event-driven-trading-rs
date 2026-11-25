# M3.1 — Workspace access-grant domain and schema

**Status:** Implementation candidate  
**Policy:** Confirmed by product owner on 25 November 2026  
**Parent:** M3.0 authorization/API audit and issue #188

## Confirmed authorization policy

- Organization OWNER and ADMIN have implicit access to every active workspace in their organization. No grant row is created for them.
- Organization TRADER, RISK_MANAGER and VIEWER require an explicit active workspace grant.
- A workspace grant can only reduce or preserve the authority allowed by the organization role. It can never promote a VIEWER into a trader, risk manager or organization administrator.
- Organization membership remains the authority for organization membership, ownership, billing and account entitlements.
- Workspace grant creation, role changes and revocation require an active organization OWNER or ADMIN.
- Workspace grants are not a substitute for active organization membership.

## Domain model

WorkspaceAccessGrant is keyed by organization, workspace and user. It carries a workspace role, ACTIVE/REVOKED status, optimistic revision, grantor/updater, reason and timestamps.

Workspace roles:

| Workspace role | Workspace-level capabilities |
|---|---|
| WORKSPACE_ADMIN | Read, trade, risk configuration and workspace operational configuration |
| TRADER | Read and trade |
| RISK_MANAGER | Read and risk configuration |
| VIEWER | Read only |

The effective capability set is the intersection of organization-role capabilities and workspace-role capabilities. Examples:

- VIEWER + WORKSPACE_ADMIN remains read-only.
- TRADER + RISK_MANAGER grant can read but cannot trade or manage risk.
- RISK_MANAGER + TRADER grant can read but cannot trade or manage risk.
- OWNER/ADMIN need no grant and receive the organization-admin capability set for active workspaces.

Workspace operational configuration does not include organization membership, grant administration, ownership transfer, billing or organization-account entitlement changes. Those remain organization OWNER/ADMIN operations.

Archived workspaces are denied by the M3.1 operational capability resolver. Whether former grantees receive a separate audit-only read path is intentionally deferred to M3.2 policy/API design.

## Database model and invariants

Migration: 20261125000002_workspace_access_grants.sql.

- workspace_access_grants stores the current grant state.
- Composite foreign keys enforce workspace/organization and organization/member consistency.
- The primary key (workspace_id,user_id) preserves one grant identity across revoke/reactivate cycles.
- Grant roles are constrained to WORKSPACE_ADMIN, TRADER, RISK_MANAGER and VIEWER.
- New grants begin ACTIVE at revision 1; changes require the next revision.
- Grant identity, original grantor and creation timestamp are immutable.
- Only active organization OWNER/ADMIN actors can create, change or revoke grants.
- An active grant requires an active organization, active workspace and active non-admin organization membership.
- OWNER/ADMIN targets cannot receive active grant rows; their access is implicit.
- Grant rows cannot be deleted. Revocation is an explicit state transition.
- Every insert/update automatically writes an append-only workspace_access_grant_events row in the same transaction.
- Suspending/revoking a member or promoting them to OWNER/ADMIN automatically revokes their active workspace grants and records the resulting grant events in the same transaction. A later demotion does not resurrect those grants; a new grant is required.
- The schema intentionally does not seed grants for OWNER/ADMIN and does not grant non-admin users access to the default workspace automatically.

## Security boundary and remaining work

This milestone adds domain policy and durable grant state; it does not yet add a workspace request principal, workspace selector, API routes, grant repository/service methods, CORS changes or workspace-scoped resource filtering.

M3.2 must resolve the current organization membership and selected workspace on every protected request, evaluate the grant against current DB state, enforce path/header consistency, and repeat authorization transactionally for grant writes. M4 must then apply workspace assignment predicates to trading resources. Until M4 is complete, the existence of a grant is not proof that dashboard queries are workspace-isolated.

No RLS policy is introduced. Personal Journal remains user-private. Global reference data remains platform-scoped.

## Verification plan

The accompanying persistence integration tests cover grant creation, revision and audit history, rejection of OWNER/ADMIN targets, rejection of non-admin grantors, automatic revocation on membership suspension/promotion, and non-resurrection after demotion. Domain tests cover implicit OWNER/ADMIN access, required active grants, archived-workspace denial and capability intersection.

Run repository CI before treating this PR as verified. The authoring environment does not execute the repository's PostgreSQL integration suite.

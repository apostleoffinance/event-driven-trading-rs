# Organization Workspace M5 — Frontend Experience

## Scope

M5 completes the authenticated workspace experience on top of the M3 authorization API and M4 resource isolation. The frontend treats the organization as the ownership/administration boundary and the workspace as the operational trading-data boundary.

## User experience

- The authenticated shell exposes separate organization and workspace selectors.
- The active workspace is selected only from the authenticated workspace-list response.
- Users with one accessible workspace receive that workspace automatically; users with multiple accessible workspaces must choose explicitly.
- The workspace settings route is `/settings/workspaces`.
- All users can inspect and switch between workspaces returned by the API.
- Only organization OWNER/ADMIN users see workspace lifecycle and grant-management controls.
- Organization onboarding continues to use the existing atomic organization/default-workspace provisioning API.

## Workspace administration

The settings page supports:
- Creating a workspace with a generated stable slug.
- Renaming a workspace using the current revision.
- Setting the default workspace using the current revision.
- Requesting archive using the current revision. The API rejects archiving the default workspace or a workspace that still contains assigned Quant accounts.
- Listing, creating, changing roles for and revoking workspace access grants.

All mutations send a non-empty audit reason and use server-issued revision numbers. The backend remains authoritative for organization membership, role intersection, grant eligibility, account assignment constraints and workspace lifecycle. A hidden UI control is not treated as a security boundary.

## Context and cache behavior

- Organization changes clear the stored workspace selection.
- Workspace refresh is tied to organization context, not every route transition.
- Workspace switching clears the TanStack Query cache.
- Event-stream state is keyed by organization and workspace and is not reused across contexts.
- The command-center data loader depends on both organization and workspace IDs, so switching workspaces triggers a fresh authenticated read.
- Trading requests continue to send `X-Organization-Id` and `X-Workspace-Id`; browser storage is a selector only and never proof of authorization.

## API contract

- `GET /api/v1/organizations/{org}/workspaces`
- `POST /api/v1/organizations/{org}/workspaces`
- `PATCH /api/v1/organizations/{org}/workspaces/{workspace}`
- `POST /api/v1/organizations/{org}/workspaces/{workspace}/default`
- `POST /api/v1/organizations/{org}/workspaces/{workspace}/archive`
- `GET/POST /api/v1/organizations/{org}/workspaces/{workspace}/members`
- `PATCH /api/v1/organizations/{org}/workspaces/{workspace}/members/{user}`

Organization-admin operations include `X-Organization-Id` and bearer authentication. Trading data requests additionally require the active `X-Workspace-Id`.

## Safety constraints

- No mock trading data is introduced.
- No execution, order submission or broker behavior is changed.
- Workspace slugs are immutable after creation.
- Workspaces are archived rather than deleted.
- A workspace cannot be archived while it is default or has assigned Quant accounts.
- Grant changes use optimistic revisions; stale revisions surface API errors for the operator to refresh and retry.
- Frontend role checks improve usability only; server-side authorization is mandatory.

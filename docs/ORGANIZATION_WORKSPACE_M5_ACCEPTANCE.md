# M5 Workspace Frontend Acceptance Matrix

Run against an isolated test organization with at least two active workspaces and multiple active non-admin organization members.

| Scenario | Expected result |
|---|---|
| User opens organization settings | Active organizations and selected organization context are shown; empty state is explicit |
| User without memberships creates an organization | Organization, OWNER membership and default workspace are provisioned atomically |
| Owner opens organization settings | Organization members are listed; add-existing-account and membership controls are visible |
| Admin adds an existing account | Membership is created with selected organization role and audit reason |
| Admin changes membership role/status | Revision-checked API mutation succeeds; refreshed membership state is shown |
| Non-admin opens organization settings | Membership list is not exposed; management controls are hidden |
| Owner opens workspace settings | All accessible active workspaces are listed; create and lifecycle controls are visible |
| Trader opens workspace settings | Only authorized workspaces are listed; organization-admin controls are hidden |
| Organization has one accessible workspace | It is selected automatically |
| Organization has multiple accessible workspaces | No arbitrary workspace is selected; user chooses explicitly |
| User switches workspace | Query cache is cleared, event state is scoped to the new organization/workspace pair, dashboard reads reload |
| User changes organization | Prior workspace selection is cleared and the new organization's accessible workspaces are loaded |
| Owner creates a workspace | API creates it with the supplied name/slug/reason; it appears in the workspace list |
| Owner renames a workspace | Revision-checked API mutation succeeds and refreshed name is shown |
| Owner changes default workspace | API updates default state; old and new default indicators refresh |
| Owner archives default workspace | UI prevents the action; API independently rejects it |
| Owner archives a workspace with assigned accounts | API rejects it; UI presents the API error without hiding the workspace |
| Owner grants/revises/revokes access | API validates active organization membership and role eligibility; refreshed grant revision is used |
| Stale workspace/grant revision | API conflict is surfaced; user refreshes before retrying |
| Cross-organization or forged workspace ID | API denies access; client-side selection is not treated as authorization |
| API unavailable | Explicit loading/error state is shown; no mock organization, workspace or trading data is substituted |
| Email invitation requested | UI does not claim to send an invitation; current API only supports adding an existing account by user ID |
| Mobile viewport and keyboard navigation | Workspace list, forms, tables and controls remain usable and focus-visible |

## CI gates

- Quant OS CI: Rust formatting/check/tests/Clippy, PostgreSQL migration/integration tests, frontend lint/typecheck/build, and platform contract tests.
- EdgeNest Production Acceptance: PR Worker preview, authenticated API acceptance, and six viewport checks.

M5 is not accepted until the current PR head has green required CI and the workspace-specific UI scenarios have been reviewed against an authorized test organization.

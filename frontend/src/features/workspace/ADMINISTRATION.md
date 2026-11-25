# Workspace Administration Guide

## Select a workspace

Use the Organization selector and Workspace selector in the authenticated top bar. Organization and workspace are separate scopes. If the selected organization has more than one accessible workspace, choose one explicitly. Switching workspace clears cached workspace data and reloads the command center and event history for the new scope.

## Create a workspace

1. Open **Workspace settings** from the sidebar.
2. Select the intended organization in the top bar.
3. In **Create workspace**, enter a name.
4. Review the generated slug and create the workspace.

Creation is available only to organization OWNER and ADMIN members. A new workspace starts empty; existing Quant accounts are not moved.

## Rename, default and archive

Select a workspace in the settings list. Organization OWNER/ADMIN members can rename it, change the default workspace or request archive. Each administrative operation is recorded by the backend with a revision and reason.

- Slugs are immutable after creation.
- The default workspace cannot be archived.
- A workspace with assigned Quant accounts cannot be archived. Reassign accounts first through the supported administrative workflow.
- Archive is a lifecycle transition, not deletion.

## Grant workspace access

In **Member access**, choose an active non-owner/non-admin organization member and assign the minimum workspace role required:

- VIEWER — read-only access.
- RISK_MANAGER — read and risk-management capabilities.
- TRADER — read and trading capabilities.
- WORKSPACE_ADMIN — operational workspace administration, without organization-level authority.

Revocation takes effect on the backend and is checked on subsequent requests. Role changes and revocations are revision-checked. Organization OWNER/ADMIN access is implicit and is not represented by grant rows.

If an administrative request conflicts with a newer revision or a backend eligibility rule, refresh the page and review the current state before retrying. The browser interface never grants authority by itself.

# M1 — Workspace persistence and backfill

**Status:** Implemented in feature branch; database execution and CI verification pending  
**Milestone:** M1 of issue #188  
**Scope:** First-class Workspace persistence, organization default-workspace backfill, and Quant-account workspace placement.

## Summary

M1 adds a persisted Workspace entity beneath Organization. Every organization receives one active default workspace. Existing organization-account entitlements are retained as the organization ownership authority and mirrored into a single workspace assignment per Quant account.

This milestone does not change organization creation (M2), expose workspace HTTP endpoints or request context (M3), migrate all trading-resource queries to workspace scope (M4), or add frontend workspace selection (M5).

## Schema added

Migration: \`20261125000001_workspace_persistence.sql\`

### workspaces

- Immutable text ID.
- Required organization FK with RESTRICT deletion behavior.
- Name (1–120 characters) and organization-local slug.
- ACTIVE / ARCHIVED lifecycle.
- Default flag, positive revision, creator and timestamps.
- Unique (organization_id, slug).
- Partial unique index for one active default per organization.
- Organization/status lookup index.
- Database constraint triggers enforce exactly one active default at transaction commit. This allows M2 to create an organization and its initial default workspace in one transaction.
- Workspace identity/ownership cannot be changed; revisions must increment by exactly one; deletion is rejected in favor of archival.

### workspace_events

Append-only, revision-keyed workspace lifecycle audit records. The initial backfill records a CREATED event for every generated default workspace.

### workspace_account_assignments

One row per Quant account (account_id primary key), linked by composite foreign keys to:
- The existing organization entitlement (organization_id, account_id).
- A workspace in the same organization (workspace_id, organization_id).

This makes cross-organization placement structurally invalid and prevents one Quant account from being assigned to two workspaces. Organization entitlement remains the ownership/access authority; workspace assignment adds operational placement.

The backfill preserves ACTIVE, SUSPENDED and REVOKED entitlement rows. Effective trading access must continue to check the original entitlement status.

### workspace_account_assignment_events

Append-only, revision-keyed audit history for workspace placement. The backfill writes an initial assignment event for each existing entitlement. Assignment rows cannot be deleted; updates require the next revision and cannot change account or organization identity.

## Backfill behavior

For each organization without an active default workspace, migration inserts:

- ID: deterministic \`ws-default-\` + MD5(organization ID).
- Name: \`Default\`.
- Slug: \`default\`.
- Status: ACTIVE.
- is_default: true.
- Creator and timestamps inherited from the organization.

The migration verifies that every organization has exactly one active default workspace before continuing.

For every existing \`organization_account_entitlements\` row, it creates one assignment to that organization's default workspace, retaining the entitlement's assigning actor and timestamps. A post-backfill assertion fails the migration if any entitlement is missing an assignment.

The migration is transactional under the existing SQLx migration runner. A failure rolls back the schema/backfill transaction. There is intentionally no destructive down migration: once application code and new workspace records exist, rollback should be a forward repair or a separately reviewed recovery operation.

No production database was accessed or modified.

## Domain and persistence

Added \`domain::Workspace\` and \`WorkspaceStatus\` with:
- Strongly typed WorkspaceId.
- Name and slug validation.
- ACTIVE / ARCHIVED lifecycle.
- Optimistic revision validation.
- Guard against an archived workspace being marked default.

Added repository methods to \`TradingStore\`:
- \`create_workspace\` — transactionally inserts workspace and CREATED event; requires active organization OWNER/ADMIN creator.
- \`workspaces_for_organization\`
- \`workspace\` — organization-scoped lookup (does not disclose cross-organization IDs).
- \`default_workspace\`
- \`workspace_for_account\` — resolves assignment within the supplied organization; caller still checks active entitlement.

A transaction-level insert primitive is available internally for M2 to reuse when organization and default workspace are created atomically.

## Migration runner compatibility

The migration runner now recognizes the exact complete pre-M1 migration history as an upgrade point, validates the existing schema/checksums as before, and applies M1 forward. It continues to reject partial or unrecognized migration histories.

## Tests added

- Domain tests for valid workspace construction, invalid names/slugs/creator, optimistic revision conflicts, and archived/default invariants.
- SQLx integration tests for:
  - Backfilling one default workspace for each existing organization.
  - Recording initial workspace events.
  - Idempotent repeated migration invocation.
  - Rejecting an organization committed without a default workspace.

The SQLx tests require the repository's authorized QOS-020 database test guard. They have not been executed in this environment. The earlier R2 report records that database validation was blocked because the authorized database identity could not be verified. No unguarded local/staging database fallback was used.

## Verification status

- [x] Workspace SQL migration authored.
- [x] Organization default-workspace backfill and postcondition checks authored.
- [x] Existing organization entitlements preserved and assigned to default workspace.
- [x] Append-only workspace and assignment audit tables authored.
- [x] Domain Workspace model and strong ID added.
- [x] Workspace repository and TradingStore façade added.
- [x] Pre-M1 migration history recognition added.
- [x] Domain and SQLx integration tests authored.
- [ ] Rust formatting, check, tests and Clippy.
- [ ] SQLx fresh-install and pre-M1 upgrade migration execution under verified QOS-020 guard.
- [ ] CI acceptance.

## Explicit non-goals / next milestones

- M2: update organization creation to insert Organization, initial OWNER membership, default Workspace and initial workspace event in one transaction.
- M3: workspace list/get/create/update/archive/default APIs and server-side authorization.
- M4: workspace-scope resource reads/writes, broker-account binding and worker admission paths, events, jobs, caches and analytics.
- M5: frontend workspace selector and onboarding integration.
- Journal remains user-private; global broker catalogue remains platform-scoped.

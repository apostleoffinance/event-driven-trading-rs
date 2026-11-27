# M6 — Workspace Hardening and Controlled Rollout

## Release decision

**Current production decision: NO-GO until the following blockers are closed.**

1. Browser-token exposure: the cookie-backed BFF/session implementation is proposed in PR #213. It moves access/refresh credentials to host-only HttpOnly cookies, adds same-origin + double-submit CSRF checks, and performs refresh rotation server-side. This blocker remains open until the PR passes CI/review, the Worker is deployed with server-only API origins, and opt-in live acceptance verifies cookie flags, CSRF rejection and refresh rotation. CORS and short access-token TTLs are not substitutes.
2. The M0 architecture audit records an unresolved R2 authorized-database verification gate. A disposable QOS-020 CI database is not evidence that the actual target database has been inspected, backed up or migrated safely. Resolve target-database verification and backup/restore rehearsal before any production migration.
3. Workspace migration backfills account entitlements and creates append-only audit history. A destructive down-migration is not a safe rollback. Keep the workspace schema and history if application code must be rolled back.

M6 does not authorize live order submission, broker activation, or bypassing the separate Phase-1/host readiness gates.

## Automated release preflight

Run only against an explicitly authorized, read-only PostgreSQL connection:

    DATABASE_URL=<read-only-connection> bash scripts/workspace_release_preflight.sh

The script opens a READ ONLY transaction and checks:
- workspace persistence and grant migrations are applied;
- every organization has exactly one active default workspace;
- every organization account entitlement has a workspace assignment;
- every assignment points to an active workspace in the same organization;
- every workspace lifecycle revision has an append-only workspace event;
- the current assignment revision has an append-only audit event;
- active grants never target suspended/revoked memberships or implicit OWNER/ADMIN members; grants on suspended organizations or archived workspaces are inert because the request principal independently denies those scopes;
- every current grant revision has an append-only grant event.

The preflight intentionally does not require an active organization/workspace for a persisted grant row: suspended organizations and archived workspaces may retain historical grant rows, but those rows cannot authorize requests while the scope is inactive. Membership suspension/revocation and promotion to OWNER/ADMIN invalidate active grants.

The preflight intentionally does not require an active organization/workspace for a persisted grant row: suspended organizations and archived workspaces may retain historical grant rows, but those rows cannot authorize requests while the scope is inactive. Membership suspension/revocation and promotion to OWNER/ADMIN invalidate active grants.

It emits aggregate counts only. It does not print database URLs, row IDs, account IDs, user IDs or secrets. Exit 0 means the invariants pass; exit 1 means violations exist; exit 2 means the check could not safely complete. A failed/unavailable check is a release blocker.

The test harness uses a stub psql and verifies pass/fail behavior, read-only SQL, missing-connection failure and secret non-disclosure. CI runs it on every pull request.

## Production deployment control

The Cloudflare production frontend workflow is manual-only. A push to main or a frontend documentation change must not deploy production. A deployment requires all of the following:
- manual workflow dispatch with confirm_production_deploy set to true;
- a supplied HTTPS release-evidence reference;
- repository Actions variables EDGENEST_COOKIE_BFF_READY=true (after CI and implementation-security review) and EDGENEST_R2_DATABASE_VERIFIED=true (after database evidence review);
- approval through the GitHub production environment protection rules (configure required reviewers).

If any condition is absent, the deploy job is skipped or fails before build/deploy. Do not set either readiness variable until the corresponding implementation-security or database evidence is reviewed and approved. After deployment, run the opt-in live BFF acceptance and require it to pass before recording a final GO decision or expanding the Workspace rollout.

## Required deployment order

1. Confirm target database, environment, owner, maintenance window and rollback authority.
2. Take a verified database backup/snapshot and record its restore point. Rehearse restore in isolation.
3. Run the authorized QOS-020 guard and workspace release preflight. Save aggregate output.
4. Apply migrations with the migration role, never the runtime role. Verify migration checksums and versions.
5. Re-run workspace preflight with a read-only role. Resolve every non-zero count before deploying application code.
6. Deploy Journal API and gateway with exact HTTPS CORS origins and allow-headers: Authorization, Content-Type, Accept, X-Organization-Id and X-Workspace-Id.
7. Deploy the frontend only after the API contract is live.
8. Canary with an internal organization and PAPER account. Verify organization/workspace selection, account assignment, role intersection, grant revocation, cross-workspace denial and audit events. Do not activate live brokers or submit orders.
9. Capture logs, migration versions, preflight output, smoke results and viewport evidence with operator, timestamp, environment and release SHA.
10. Expand gradually only after canary evidence is reviewed and the cookie-backed session blocker is closed.

## Security acceptance

Verify against an authorized test database and deployed API:
- Missing/malformed organization or workspace selectors fail closed.
- Forged workspace IDs are not authorization evidence.
- Cross-organization/workspace lookups do not disclose data.
- Suspended/revoked memberships and grants deny access immediately.
- Workspace roles cannot elevate organization authority.
- Archived workspaces cannot be used operationally.
- Account reassignment is revision-checked and audited.
- Organization/workspace switching clears prior scoped client state.
- CORS allows the workspace selector only for approved origins/headers.

Existing Rust API tests cover assignment isolation, missing context, role/grant enforcement, revocation and selected-workspace paper-account creation. Persistence tests cover default backfill, idempotent migration, grant revisions, membership revocation and append-only grant history. Preserve these as mandatory gates.

## Authorization observability

Workspace authorization now emits structured tracing reason codes for database unavailability, principal lookup failures, missing organization/workspace context, inaccessible or inactive workspace context, capability denial, organization/path mismatch and missing organization-admin authority. These events intentionally omit bearer tokens, refresh tokens, user IDs, organization IDs and workspace IDs. Keep the append-only workspace, assignment and grant event tables as the durable audit record; application logs are operational signals, not a replacement for database history.

## Rollback and recovery

- Before migration: stop; no changes required.
- After migration but before application deployment: retain the expanded schema and backfilled assignments. Do not drop workspace tables/history.
- After application deployment: disable workspace-facing behavior or roll back to a code version proven compatible with the expanded schema. Keep migrations applied.
- If invariants fail: stop workspace/account mutations, preserve logs and audit rows, and restore only through the approved recovery process with explicit acceptance of post-backup data loss.
- Never delete assignments, truncate audit tables, fabricate defaults or edit migration history to force a green state.

## Operational boundaries

Organization is the identity, membership, entitlement and billing boundary. Workspace is the operational Quant resource boundary. Personal Journal remains user-private; global broker catalogues remain platform-scoped. Browser-stored IDs are selectors only; PostgreSQL authorization is authoritative. Workspace completion does not authorize live execution.

## Evidence record

Retain release SHA/deployment IDs, migration versions/checksums, backup and restore rehearsal reference, aggregate preflight output, CI/production-acceptance URLs, canary evidence, CORS/health/tenant-isolation results, operator approval, timestamps and final GO/NO-GO decision.

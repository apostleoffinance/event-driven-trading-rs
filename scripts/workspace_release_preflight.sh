#!/usr/bin/env bash
set -euo pipefail
# Read-only workspace release invariant check. Never writes data or runs migrations.
if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "BLOCKED: DATABASE_URL must reference an authorized read-only database role." >&2
  exit 2
fi
command -v psql >/dev/null 2>&1 || { echo "BLOCKED: psql is required." >&2; exit 2; }
SQL=$(cat <<'SQL'
BEGIN READ ONLY;
SELECT 'workspace_migration_applied', CASE WHEN EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version=20261125000001 AND success) THEN 0 ELSE 1 END;
SELECT 'workspace_grants_migration_applied', CASE WHEN EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version=20261125000002 AND success) THEN 0 ELSE 1 END;
SELECT 'one_active_default_per_organization', count(*) FROM (
 SELECT o.id FROM organizations o LEFT JOIN workspaces w ON w.organization_id=o.id AND w.is_default AND w.status='ACTIVE' GROUP BY o.id HAVING count(w.id)<>1
) invalid_defaults;
SELECT 'workspaces_missing_current_audit_event', count(*) FROM workspaces w WHERE NOT EXISTS (SELECT 1 FROM workspace_events e WHERE e.workspace_id=w.id AND e.organization_id=w.organization_id AND e.revision=w.revision AND e.next_name=w.name AND e.next_status=w.status AND e.next_is_default=w.is_default);
SELECT 'entitlements_without_workspace_assignment', count(*) FROM organization_account_entitlements e LEFT JOIN workspace_account_assignments a ON a.organization_id=e.organization_id AND a.account_id=e.account_id WHERE a.account_id IS NULL;
SELECT 'assignments_without_active_workspace', count(*) FROM workspace_account_assignments a LEFT JOIN workspaces w ON w.id=a.workspace_id AND w.organization_id=a.organization_id WHERE w.id IS NULL OR w.status<>'ACTIVE';
SELECT 'assignments_missing_current_audit_event', count(*) FROM workspace_account_assignments a WHERE NOT EXISTS (SELECT 1 FROM workspace_account_assignment_events e WHERE e.account_id=a.account_id AND e.workspace_id=a.workspace_id AND e.revision=a.revision);
SELECT 'invalid_active_workspace_grants', count(*) FROM workspace_access_grants g JOIN organizations o ON o.id=g.organization_id JOIN workspaces w ON w.id=g.workspace_id AND w.organization_id=g.organization_id JOIN organization_memberships m ON m.organization_id=g.organization_id AND m.user_id=g.user_id WHERE g.status='ACTIVE' AND (o.status<>'ACTIVE' OR w.status<>'ACTIVE' OR m.status<>'ACTIVE' OR m.role IN ('OWNER','ADMIN'));
SELECT 'grants_missing_current_audit_event', count(*) FROM workspace_access_grants g WHERE NOT EXISTS (SELECT 1 FROM workspace_access_grant_events e WHERE e.organization_id=g.organization_id AND e.workspace_id=g.workspace_id AND e.user_id=g.user_id AND e.revision=g.revision);
COMMIT;
SQL
)
if ! output="$(PGDATABASE="$DATABASE_URL" psql -X -qAt -v ON_ERROR_STOP=1 -F '|' 2>/dev/null <<< "$SQL")"; then
 echo "BLOCKED: read-only database checks could not be completed. Verify the authorized connection and migration state." >&2
 exit 2
fi
failed=0
while IFS='|' read -r invariant violations; do
 [[ -z "$invariant" ]] && continue
 if [[ ! "$violations" =~ ^[0-9]+$ ]]; then
  printf 'BLOCKED: %s returned an invalid result.\n' "$invariant"; failed=1
 elif [[ "$violations" -eq 0 ]]; then
  printf 'PASS: %s\n' "$invariant"
 else
  printf 'BLOCKED: %s (%s violation(s))\n' "$invariant" "$violations"; failed=1
 fi
done <<< "$output"
if [[ "$failed" -ne 0 ]]; then echo "WORKSPACE RELEASE PREFLIGHT: BLOCKED"; exit 1; fi
echo "WORKSPACE RELEASE PREFLIGHT: PASS"

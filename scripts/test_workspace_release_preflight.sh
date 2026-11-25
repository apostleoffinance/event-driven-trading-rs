#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/psql" <<'PSQL'
#!/usr/bin/env bash
set -euo pipefail
for arg in "$@"; do
  case "$arg" in postgres://*|postgresql://*) echo "Database URL leaked into process arguments" >&2; exit 98 ;; esac
done
cat > "$PSQL_INPUT_FILE"
printf '%s\n' "$PSQL_OUTPUT"
PSQL
chmod +x "$TMP/psql"

PASS_OUTPUT=$'workspace_migration_applied|0\nworkspace_grants_migration_applied|0\none_active_default_per_organization|0\nentitlements_without_workspace_assignment|0\nassignments_without_active_workspace|0\nassignments_missing_current_audit_event|0\ninvalid_active_workspace_grants|0\ngrants_missing_current_audit_event|0'
FAIL_OUTPUT=$'workspace_migration_applied|0\nworkspace_grants_migration_applied|0\none_active_default_per_organization|0\nentitlements_without_workspace_assignment|1\nassignments_without_active_workspace|0\nassignments_missing_current_audit_event|0\ninvalid_active_workspace_grants|0\ngrants_missing_current_audit_event|0'

if DATABASE_URL='postgres://secret.example.invalid/private' \
   PATH="$TMP:$PATH" PSQL_OUTPUT="$PASS_OUTPUT" PSQL_INPUT_FILE="$TMP/pass.sql" \
   bash "$ROOT/scripts/workspace_release_preflight.sh" >"$TMP/pass.out" 2>"$TMP/pass.err"; then
  grep -q 'WORKSPACE RELEASE PREFLIGHT: PASS' "$TMP/pass.out"
else
  echo "Expected the all-zero invariant fixture to pass." >&2
  cat "$TMP/pass.err" >&2
  exit 1
fi

grep -q 'BEGIN READ ONLY;' "$TMP/pass.sql"
if grep -Eiq '^[[:space:]]*(INSERT|UPDATE|DELETE|ALTER|DROP|CREATE|TRUNCATE|GRANT|REVOKE)[[:space:]]' "$TMP/pass.sql"; then
  echo "Preflight SQL must remain read-only." >&2
  exit 1
fi
if grep -q 'postgres://secret' "$TMP/pass.out" "$TMP/pass.err"; then
  echo "Preflight output must not disclose the database URL." >&2
  exit 1
fi

set +e
DATABASE_URL='postgres://secret.example.invalid/private' \
  PATH="$TMP:$PATH" PSQL_OUTPUT="$FAIL_OUTPUT" PSQL_INPUT_FILE="$TMP/fail.sql" \
  bash "$ROOT/scripts/workspace_release_preflight.sh" >"$TMP/fail.out" 2>"$TMP/fail.err"
status=$?
set -e
if [[ "$status" -ne 1 ]] || ! grep -q 'entitlements_without_workspace_assignment (1 violation' "$TMP/fail.out"; then
  echo "Expected a non-zero invariant count to block release." >&2
  cat "$TMP/fail.out" "$TMP/fail.err" >&2
  exit 1
fi

set +e
env -u DATABASE_URL PATH="$TMP:$PATH" bash "$ROOT/scripts/workspace_release_preflight.sh" >"$TMP/missing.out" 2>"$TMP/missing.err"
status=$?
set -e
if [[ "$status" -ne 2 ]] || ! grep -q 'DATABASE_URL must reference' "$TMP/missing.err"; then
  echo "Expected missing DATABASE_URL to fail closed." >&2
  exit 1
fi

echo "PASS: workspace release preflight script tests"

#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf 'ERROR: %s\n' "$1" >&2
    exit 2
}

: "${DATABASE_ADMIN_URL:?set DATABASE_ADMIN_URL to the isolated staging administrator URL}"
: "${MT5_V2_STAGING_APP_USER:?set MT5_V2_STAGING_APP_USER}"
: "${MT5_V2_STAGING_APP_PASSWORD:?set MT5_V2_STAGING_APP_PASSWORD}"

[[ "$MT5_V2_STAGING_APP_USER" == "mt5_v2_app" ]] \
    || fail "MT5_V2_STAGING_APP_USER must be mt5_v2_app"
[[ ${#MT5_V2_STAGING_APP_PASSWORD} -ge 32 ]] \
    || fail "staging app password must contain at least 32 characters"
[[ "$MT5_V2_STAGING_APP_PASSWORD" != *$'\n'* && "$MT5_V2_STAGING_APP_PASSWORD" != *$'\r'* ]] \
    || fail "staging app password must not contain line breaks"

command -v python3 >/dev/null 2>&1 || fail "python3 is required"

python3 - <<'PY'
import os
import re
import sys
from urllib.parse import unquote, urlsplit

try:
    url = urlsplit(os.environ["DATABASE_ADMIN_URL"])
    username = unquote(url.username or "")
    password = unquote(url.password or "")
    valid = (
        url.scheme in {"postgres", "postgresql"}
        and url.hostname == "127.0.0.1"
        and url.port == 55439
        and url.path == "/mt5_v2_staging"
        and re.fullmatch(r"[a-z_][a-z0-9_]{0,62}", username)
        and password
        and not any(character in username + password for character in "\r\n")
        and not url.query
        and not url.fragment
    )
except ValueError:
    valid = False

if not valid:
    sys.exit(
        "ERROR: DATABASE_ADMIN_URL must target "
        "127.0.0.1:55439/mt5_v2_staging and include admin credentials"
    )
PY

command -v psql >/dev/null 2>&1 || fail "psql is required"

export PGHOST=127.0.0.1
export PGPORT=55439
export PGDATABASE=mt5_v2_staging
export PGUSER
export PGPASSWORD
PGUSER="$(python3 - <<'PY'
import os
from urllib.parse import unquote, urlsplit

print(unquote(urlsplit(os.environ["DATABASE_ADMIN_URL"]).username))
PY
)"
PGPASSWORD="$(python3 - <<'PY'
import os
from urllib.parse import unquote, urlsplit

print(unquote(urlsplit(os.environ["DATABASE_ADMIN_URL"]).password))
PY
)"

[[ "$PGUSER" != "$MT5_V2_STAGING_APP_USER" ]] \
    || fail "staging app role must differ from the database administrator"

export MT5_V2_STAGING_APP_USER MT5_V2_STAGING_APP_PASSWORD
psql -X --no-password --quiet --set=ON_ERROR_STOP=1 <<'SQL'
\getenv app_role MT5_V2_STAGING_APP_USER
\getenv app_password MT5_V2_STAGING_APP_PASSWORD

SELECT format(
    'CREATE ROLE %I LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD %L',
    :'app_role',
    :'app_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'app_role')
\gexec

SELECT format(
    'ALTER ROLE %I LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD %L',
    :'app_role',
    :'app_password'
)
\gexec

SELECT format('GRANT CONNECT ON DATABASE %I TO %I', current_database(), :'app_role')
\gexec
SELECT format('GRANT USAGE, CREATE ON SCHEMA public TO %I', :'app_role')
\gexec
SELECT format('GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO %I', :'app_role')
\gexec
SELECT format('GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO %I', :'app_role')
\gexec
SQL

printf 'Staging app role provisioned. No credentials were printed.\n'
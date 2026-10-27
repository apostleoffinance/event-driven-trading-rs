#!/usr/bin/env bash
# Load a protected shell-format environment file for a launchd job, then exec.
set -Eeuo pipefail

if (($# < 1)); then
  echo "Usage: mt5-macos-launch-wrapper.sh COMMAND [ARG ...]" >&2
  exit 64
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "BLOCKED: this wrapper is for macOS launchd jobs." >&2
  exit 2
fi

env_file="${QUANTOS_ENV_FILE:-}"
if [[ -z "$env_file" || ! -f "$env_file" || -L "$env_file" ]]; then
  echo "BLOCKED: QUANTOS_ENV_FILE must name an existing regular non-symlink file." >&2
  exit 2
fi

file_uid="$(stat -f '%u' "$env_file")"
file_mode="$(stat -f '%Lp' "$env_file")"
if [[ "$file_uid" != "$(id -u)" ]]; then
  echo "BLOCKED: environment file must be owned by the current service user." >&2
  exit 2
fi
# Require exactly owner read/write (0600) or owner read-only (0400).
if [[ "$file_mode" != "600" && "$file_mode" != "400" ]]; then
  echo "BLOCKED: environment file permissions must be 0600 or 0400." >&2
  exit 2
fi

# shellcheck disable=SC1090
set -a
source "$env_file"
set +a
exec "$@"

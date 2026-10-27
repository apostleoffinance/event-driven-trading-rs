#!/usr/bin/env bash
# Quant OS macOS developer preflight. This is NOT the real-terminal release gate.
set -Eeuo pipefail

usage() {
  cat <<'USAGE'
Usage: mt5-macos-preflight.sh [--skip-rust] [--require-real-terminal]

Runs portable Quant OS checks on macOS. The current MetaTrader5 Python API
integration in this repository is Windows-only. Therefore this script will
never claim real MT5 terminal conformance on macOS unless a supported native
terminal provider is implemented and explicitly verified.

Options:
  --skip-rust             Skip cargo fmt/check/test/clippy checks.
  --require-real-terminal Fail closed unless a supported macOS terminal provider
                          is explicitly configured and available.
  -h, --help              Show this help.

Required for Rust integration tests: a reachable PostgreSQL test database if
the repository's tests require one. Never put credentials in command arguments
or paste secrets/logs into issues.
USAGE
}

skip_rust=0
require_real_terminal=0
while (($#)); do
  case "$1" in
    --skip-rust) skip_rust=1 ;;
    --require-real-terminal) require_real_terminal=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "BLOCKED: this entry point is for macOS; portable tests can be run directly on other systems." >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
ADAPTER_DIR="$REPO_ROOT/adapters/mt5-python"
cd "$REPO_ROOT"

failed=0
run_check() {
  local label="$1"; shift
  printf '\n== %s ==\n' "$label"
  if "$@"; then
    printf 'PASS: %s\n' "$label"
  else
    printf 'FAIL: %s\n' "$label" >&2
    failed=1
  fi
}

for tool in python3 openssl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "BLOCKED: required tool not found: $tool" >&2
    exit 2
  fi
done

echo "Quant OS macOS preflight"
echo "Repository: $REPO_ROOT"
echo "Commit: $(git rev-parse HEAD 2>/dev/null || echo UNKNOWN)"
echo "macOS: $(sw_vers -productVersion 2>/dev/null || echo UNKNOWN)"
echo "Python: $(python3 --version 2>&1)"
echo "OpenSSL: $(openssl version 2>&1)"
echo "Safety boundary: local contract checks do not establish real-terminal conformance."

cd "$ADAPTER_DIR"
run_check "MT5 Python adapter contract tests" python3 -m unittest discover -s tests -v
cd "$REPO_ROOT"

if ((skip_rust == 0)); then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "BLOCKED: cargo not found; install the stable Rust toolchain or rerun with --skip-rust." >&2
    exit 2
  fi
  run_check "Rust formatting" cargo fmt --all -- --check
  run_check "Rust workspace check" cargo check --workspace
  run_check "Rust workspace tests" cargo test --workspace
  run_check "Rust Clippy" cargo clippy --workspace --all-targets
fi

# The current terminal provider deliberately rejects non-Windows platforms.
# Never convert this into a PASS just because the macOS MT5 application exists.
if [[ "${MT5_MACOS_TERMINAL_PROVIDER:-}" == "" ]]; then
  echo "NOT_RUN: real MT5 terminal API is not configured for macOS."
  if ((require_real_terminal)); then
    echo "BLOCKED: --require-real-terminal requested, but MT5_MACOS_TERMINAL_PROVIDER is unset." >&2
    exit 2
  fi
else
  echo "BLOCKED: MT5_MACOS_TERMINAL_PROVIDER is set, but this preflight does not certify third-party providers."
  echo "A provider-specific authenticated demo smoke test and independent review are required." >&2
  if ((require_real_terminal)); then exit 2; fi
fi

if ((failed)); then
  echo "RESULT: FAIL — one or more local checks failed. No venue execution is authorized." >&2
  exit 1
fi

echo "RESULT: LOCAL_CHECKS_PASS_OR_NOT_RUN — inspect NOT_RUN items. This is not QOS-015A release approval."
echo "Venue execution must remain disabled."

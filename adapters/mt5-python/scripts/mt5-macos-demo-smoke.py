#!/usr/bin/env python3
"""Read-only macOS MT5 demo smoke test for a trusted terminal provider.

A provider must connect to the real designated demo terminal. This script never
submits, modifies, or cancels orders. A passing local check is not Windows
conformance or release authorization.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import ssl
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from mt5_adapter.worker import (
    AdapterError,
    Config,
    build_market_data_snapshot,
    build_snapshot,
    load_terminal_runtime,
    tls_context,
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def parse_time(value: Any, field: str) -> datetime:
    if not isinstance(value, str):
        raise AdapterError(f"database snapshot omitted timestamp field: {field}")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise AdapterError(f"database timestamp is timezone-naive: {field}")
    return parsed.astimezone(timezone.utc)


def latest_persisted_state(config: Config, timeout_seconds: int = 20) -> dict[str, Any]:
    database_url = os.environ.get("DATABASE_URL", "")
    if not database_url or database_url.strip() != database_url:
        raise AdapterError("DATABASE_URL is required for persisted snapshot verification")
    query = """
WITH market AS (
  SELECT sequence, captured_at, source, account_currency, server
  FROM public.mt5_bridge_market_data_snapshots
  WHERE account_id = :'account_id' AND terminal_id = :'terminal_id'
  ORDER BY captured_at DESC LIMIT 1
), reconciliation AS (
  SELECT reconciled_sequence, captured_at, source,
         snapshot -> 'account_state' ->> 'currency' AS account_currency,
         snapshot ->> 'server' AS server
  FROM public.mt5_bridge_reconciliation_snapshots
  WHERE account_id = :'account_id' AND terminal_id = :'terminal_id'
  ORDER BY created_at DESC LIMIT 1
)
SELECT json_build_object(
  'market_sequence', market.sequence,
  'market_captured_at', market.captured_at,
  'market_source', market.source,
  'market_account_currency', market.account_currency,
  'market_server', market.server,
  'reconciliation_sequence', reconciliation.reconciled_sequence,
  'reconciliation_captured_at', reconciliation.captured_at,
  'reconciliation_source', reconciliation.source,
  'reconciliation_currency', reconciliation.account_currency,
  'reconciliation_server', reconciliation.server
)::text
FROM market CROSS JOIN reconciliation;
"""
    env = {**os.environ, "PGDATABASE": database_url}
    try:
        result = subprocess.run(
            [
                "psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1",
                "-v", f"account_id={config.account_id}",
                "-v", f"terminal_id={config.terminal_id}",
                "-c", query,
            ],
            check=False,
            capture_output=True,
            text=True,
            env=env,
            timeout=max(5, min(timeout_seconds, 60)),
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise AdapterError("unable to query persisted MT5 snapshots using psql") from exc
    if result.returncode != 0 or not result.stdout.strip():
        raise AdapterError("persisted MT5 snapshots are unavailable or database query failed")
    try:
        record = json.loads(result.stdout.strip().splitlines()[-1])
    except json.JSONDecodeError as exc:
        raise AdapterError("database returned invalid snapshot verification JSON") from exc
    required = (
        "market_sequence", "market_captured_at", "market_source",
        "market_account_currency", "market_server", "reconciliation_sequence",
        "reconciliation_captured_at", "reconciliation_source",
        "reconciliation_currency", "reconciliation_server",
    )
    if not isinstance(record, dict) or any(record.get(key) is None for key in required):
        raise AdapterError("market-data/reconciliation snapshots are not both persisted for the configured identity")
    return record


def verify_tls_configuration(config: Config) -> None:
    # Do not open a second bridge connection while the single active worker is
    # streaming. The live worker's authenticated persisted snapshots are the
    # end-to-end connectivity evidence; this check validates local TLS settings.
    context = tls_context(config)
    if context.minimum_version != ssl.TLSVersion.TLSv1_3:
        raise AdapterError("client TLS minimum version is not TLS 1.3")
    if context.maximum_version != ssl.TLSVersion.TLSv1_3:
        raise AdapterError("client TLS maximum version is not TLS 1.3")


class ReadOnlyTerminalView:
    """Delegate terminal reads while preventing smoke-test Market Watch changes."""

    def __init__(self, terminal: Any, allowed_symbols: tuple[str, ...]) -> None:
        self._terminal = terminal
        self._allowed_symbols = frozenset(allowed_symbols)

    def __getattr__(self, name: str) -> Any:
        return getattr(self._terminal, name)

    def select_symbol(self, symbol: str) -> bool:
        # The worker owns symbol selection. The smoke probe must not mutate it.
        return symbol in self._allowed_symbols


def smoke(timeout_seconds: int, persisted_only: bool = False) -> dict[str, Any]:
    config = Config.from_env()
    expected_currency = os.environ.get("MT5_EXPECTED_ACCOUNT_CURRENCY", "")
    if not expected_currency or expected_currency.strip() != expected_currency:
        raise AdapterError("MT5_EXPECTED_ACCOUNT_CURRENCY is required")
    if sys.platform != "darwin":
        raise AdapterError("this smoke test is intended for macOS")
    repository_commit = os.environ.get("QUANTOS_REPOSITORY_COMMIT", "")
    if not re.fullmatch(r"(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})", repository_commit):
        raise AdapterError("QUANTOS_REPOSITORY_COMMIT must be the exact 40- or 64-character deployed commit hash")

    started = utc_now()
    local_market_type = "NOT_BUILT_PERSISTED_ONLY"
    local_reconciliation_type = "NOT_BUILT_PERSISTED_ONLY"
    if not persisted_only:
        # The EA serves one worker connection. For a direct terminal probe, stop
        # the supervised worker first so the EA can reconnect to this smoke client.
        terminal, constants = load_terminal_runtime(config)
        try:
            terminal.connect(config)
            if not terminal.connected():
                raise AdapterError("terminal provider did not establish a connected session")
            account = terminal.account_info()
            if account is None or int(account.login) != config.login or str(account.server) != config.server:
                raise AdapterError("authenticated terminal identity does not match configured login/server")
            if str(account.currency) != expected_currency:
                raise AdapterError("authenticated terminal account currency does not match expected currency")
            read_only_terminal = ReadOnlyTerminalView(terminal, config.symbols)
            market = build_market_data_snapshot(config, read_only_terminal, 1)
            reconciliation = build_snapshot(config, read_only_terminal, constants, 1)
            local_market_type = market["payload"]["type"]
            local_reconciliation_type = reconciliation["payload"]["type"]
        finally:
            terminal.shutdown()

    # TLS configuration is checked locally. Do not open a second Rust bridge
    # connection: the Rust daemon intentionally accepts one active worker stream.
    verify_tls_configuration(config)
    persisted = latest_persisted_state(config, timeout_seconds)
    now = datetime.now(timezone.utc)
    market_at = parse_time(persisted["market_captured_at"], "market_captured_at")
    recon_at = parse_time(persisted["reconciliation_captured_at"], "reconciliation_captured_at")
    for field, captured_at in (("market", market_at), ("reconciliation", recon_at)):
        if (now - captured_at).total_seconds() > 60 or (captured_at - now).total_seconds() > 5:
            raise AdapterError(f"latest persisted {field} snapshot is stale or future-dated")

    expected_source = "mt5_macos_mql5_socket_v1"
    if persisted["market_source"] != expected_source or persisted["reconciliation_source"] != expected_source:
        raise AdapterError("persisted snapshots do not identify the native macOS MQL5 provider")
    if persisted["market_server"] != config.server or persisted["reconciliation_server"] != config.server:
        raise AdapterError("persisted terminal server identity does not match configured server")
    if (
        persisted["market_account_currency"] != expected_currency
        or persisted["reconciliation_currency"] != expected_currency
    ):
        raise AdapterError("persisted account currency does not match expected demo currency")
    if int(persisted["market_sequence"]) != int(persisted["reconciliation_sequence"]):
        raise AdapterError("latest market-data and reconciliation snapshots do not share the same sequence")

    return {
        "schema_version": 1,
        "platform": "macOS",
        "started_at_utc": started,
        "finished_at_utc": utc_now(),
        "repository_commit": repository_commit.lower(),
        "terminal_provider": os.environ.get("MT5_TERMINAL_PROVIDER", "UNCONFIGURED"),
        "terminal_probe": "PERSISTED_ACTIVE_WORKER_STREAM" if persisted_only else "DIRECT_MQL5_EA_CONNECTION",
        "account_masked": "..." + str(config.login)[-4:],
        "broker_server": config.server,
        "account_currency": expected_currency,
        "terminal_id": config.terminal_id,
        "symbols": list(config.symbols),
        "market_contract_type": local_market_type,
        "reconciliation_contract_type": local_reconciliation_type,
        "client_tls13_mtls_context": "PASS",
        "active_bridge_persistence": "PASS",
        "persisted_market_sequence": int(persisted["market_sequence"]),
        "persisted_market_captured_at": persisted["market_captured_at"],
        "persisted_market_source": persisted["market_source"],
        "persisted_reconciliation_sequence": int(persisted["reconciliation_sequence"]),
        "persisted_reconciliation_captured_at": persisted["reconciliation_captured_at"],
        "persisted_reconciliation_source": persisted["reconciliation_source"],
        "market_and_reconciliation_sequences_match": True,
        "result": "PASS_MACOS_PERSISTED_STREAM" if persisted_only else "PASS_MACOS_DEMO_SMOKE",
        "venue_execution_enabled": False,
        "note": "Provider smoke evidence only; not Windows-specific release approval. Persisted-only mode validates the active worker stream without opening a competing EA connection.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout-seconds", type=int, default=10)
    parser.add_argument("--persisted-only", action="store_true", help="Verify fresh persisted snapshots from the active native-provider worker without opening a competing EA connection.")
    parser.add_argument("--evidence-out", type=Path)
    args = parser.parse_args()
    if args.timeout_seconds < 1 or args.timeout_seconds > 60:
        parser.error("--timeout-seconds must be between 1 and 60")
    try:
        report = smoke(args.timeout_seconds, persisted_only=args.persisted_only)
    except Exception as exc:
        # Provider/terminal exceptions may contain operational details; report only
        # the type and keep raw diagnostics in the protected host log.
        print(f"BLOCKED: macOS demo smoke failed ({type(exc).__name__}).", file=sys.stderr)
        return 1
    rendered = json.dumps(report, indent=2, sort_keys=True)
    if args.evidence_out:
        args.evidence_out.parent.mkdir(parents=True, exist_ok=True)
        args.evidence_out.write_text(rendered + "\n", encoding="utf-8")
        args.evidence_out.chmod(0o600)
    print(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Fail-closed macOS LaunchAgent worker restart verification.

Only the worker LaunchAgent is unloaded/reloaded. The daemon is never stopped.
Requires a trusted real-terminal provider, a working read-only smoke test, and
persisted PostgreSQL snapshots. This does not satisfy Windows release gates.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
SMOKE_PATH = SCRIPT_DIR / "mt5-macos-demo-smoke.py"
SPEC = importlib.util.spec_from_file_location("mt5_macos_demo_smoke", SMOKE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("unable to load macOS demo smoke module")
SMOKE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SMOKE)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def run_launchctl(*args: str, expected_success: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["launchctl", *args],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if expected_success and result.returncode != 0:
        raise RuntimeError("launchctl operation failed: " + args[0])
    return result


def job_loaded(domain: str, label: str) -> bool:
    result = run_launchctl("print", f"{domain}/{label}", expected_success=False)
    return result.returncode == 0


def run_smoke(timeout_seconds: int) -> None:
    result = subprocess.run(
        [sys.executable, str(SMOKE_PATH), "--timeout-seconds", str(timeout_seconds), "--persisted-only"],
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout_seconds + 30,
    )
    if result.returncode != 0:
        # Deliberately do not echo captured output; terminal/provider errors may
        # include operational identifiers. Operators should retain redacted logs
        # separately in the approved evidence store.
        raise RuntimeError("provider-backed macOS demo smoke did not pass")


def write_evidence(path: Path, record: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    path.chmod(0o600)


def verify(args: argparse.Namespace) -> dict[str, Any]:
    if sys.platform != "darwin":
        raise RuntimeError("restart verification must run on macOS")
    if not shutil_which("launchctl") or not shutil_which("psql"):
        raise RuntimeError("launchctl and psql must be installed and available")
    worker_plist = args.worker_plist.expanduser().resolve(strict=True)
    if not worker_plist.is_file():
        raise RuntimeError("worker plist is not a regular file")
    import plistlib
    with worker_plist.open("rb") as stream:
        plist = plistlib.load(stream)
    if plist.get("Label") != args.worker_label:
        raise RuntimeError("worker plist Label does not match --worker-label")

    uid = os.getuid()
    domain = f"gui/{uid}"
    if not job_loaded(domain, args.worker_label):
        raise RuntimeError("worker LaunchAgent must already be loaded")
    if not job_loaded(domain, args.daemon_label):
        raise RuntimeError("daemon LaunchAgent must already be loaded; it will not be started or stopped")

    record: dict[str, Any] = {
        "started_at_utc": utc_now(),
        "worker_service": args.worker_label,
        "daemon_service": args.daemon_label,
        "first_smoke": "NOT_RUN",
        "worker_stopped_at_utc": None,
        "worker_restarted_at_utc": None,
        "daemon_remained_running": False,
        "pre_stop_market_sequence": None,
        "post_restart_market_sequence": None,
        "post_restart_reconciliation_sequence": None,
        "post_restart_market_captured_at": None,
        "post_restart_reconciliation_captured_at": None,
        "sequence_advanced": False,
        "post_restart_reconciliation_fresh": False,
        "second_smoke": "NOT_RUN",
        "worker_restored_after_test": False,
        "result": "BLOCKED",
    }
    worker_unloaded = False
    try:
        run_smoke(args.timeout_seconds)
        record["first_smoke"] = "PASS"
        config = SMOKE.Config.from_env()
        before = SMOKE.latest_persisted_state(config)
        record["pre_stop_market_sequence"] = int(before["market_sequence"])

        run_launchctl("bootout", domain, str(worker_plist))
        deadline = time.monotonic() + 30
        while job_loaded(domain, args.worker_label) and time.monotonic() < deadline:
            time.sleep(1)
        if job_loaded(domain, args.worker_label):
            raise RuntimeError("worker LaunchAgent remained loaded after bootout")
        worker_unloaded = True
        record["worker_stopped_at_utc"] = utc_now()
        if not job_loaded(domain, args.daemon_label):
            raise RuntimeError("daemon LaunchAgent stopped during worker-only restart")
        record["daemon_remained_running"] = True

        record["worker_restarted_at_utc"] = utc_now()
        run_launchctl("bootstrap", domain, str(worker_plist))
        deadline = time.monotonic() + min(args.timeout_seconds, 30)
        while not job_loaded(domain, args.worker_label) and time.monotonic() < deadline:
            time.sleep(1)
        if not job_loaded(domain, args.worker_label):
            raise RuntimeError("worker LaunchAgent did not load after bootstrap")
        worker_unloaded = False
        record["worker_restored_after_test"] = True
        deadline = time.monotonic() + args.timeout_seconds
        stop_time = datetime.fromisoformat(record["worker_stopped_at_utc"].replace("Z", "+00:00"))
        latest: dict[str, Any] | None = None
        while time.monotonic() < deadline:
            if not job_loaded(domain, args.worker_label):
                time.sleep(args.poll_seconds)
                continue
            try:
                latest = SMOKE.latest_persisted_state(config)
                market_at = SMOKE.parse_time(latest["market_captured_at"], "market_captured_at")
                recon_at = SMOKE.parse_time(latest["reconciliation_captured_at"], "reconciliation_captured_at")
                if (
                    int(latest["market_sequence"]) > int(record["pre_stop_market_sequence"])
                    and market_at >= stop_time
                    and recon_at >= stop_time
                ):
                    break
            except Exception:
                latest = None
            time.sleep(args.poll_seconds)

        if latest is None:
            raise RuntimeError("no fresh persisted snapshot pair appeared after worker restart")
        market_at = SMOKE.parse_time(latest["market_captured_at"], "market_captured_at")
        recon_at = SMOKE.parse_time(latest["reconciliation_captured_at"], "reconciliation_captured_at")
        record["post_restart_market_sequence"] = int(latest["market_sequence"])
        record["post_restart_reconciliation_sequence"] = int(latest["reconciliation_sequence"])
        record["post_restart_market_captured_at"] = market_at.isoformat()
        record["post_restart_reconciliation_captured_at"] = recon_at.isoformat()
        record["sequence_advanced"] = (
            int(latest["market_sequence"]) > int(record["pre_stop_market_sequence"])
        )
        record["post_restart_reconciliation_fresh"] = market_at >= stop_time and recon_at >= stop_time
        if not record["sequence_advanced"] or not record["post_restart_reconciliation_fresh"]:
            raise RuntimeError("sequence advancement or fresh reconciliation assertion failed")
        if not job_loaded(domain, args.daemon_label):
            raise RuntimeError("daemon LaunchAgent is not loaded after worker recovery")

        run_smoke(args.timeout_seconds)
        record["second_smoke"] = "PASS"
        record["result"] = "PASS_RESTART_RECOVERY"
        return record
    except Exception as exc:
        # Store only the exception class to avoid leaking paths, IDs, or provider
        # diagnostics into the evidence artifact.
        record["failure"] = type(exc).__name__
        raise
    finally:
        if worker_unloaded:
            try:
                if not job_loaded(domain, args.worker_label):
                    run_launchctl("bootstrap", domain, str(worker_plist))
                deadline = time.monotonic() + 30
                while not job_loaded(domain, args.worker_label) and time.monotonic() < deadline:
                    time.sleep(1)
                record["worker_restored_after_test"] = job_loaded(domain, args.worker_label)
            except Exception:
                record["worker_restored_after_test"] = False
        record["finished_at_utc"] = utc_now()
        write_evidence(args.evidence_out, record)


def shutil_which(name: str) -> str | None:
    import shutil
    return shutil.which(name)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker-label", required=True)
    parser.add_argument("--daemon-label", required=True)
    parser.add_argument("--worker-plist", required=True, type=Path)
    parser.add_argument("--timeout-seconds", type=int, default=240)
    parser.add_argument("--poll-seconds", type=int, default=5)
    parser.add_argument("--evidence-out", required=True, type=Path)
    args = parser.parse_args()
    if args.timeout_seconds < 30 or not 1 <= args.poll_seconds <= 30:
        parser.error("timeout must be >=30 seconds and poll interval must be 1-30 seconds")
    try:
        record = verify(args)
    except Exception as exc:
        print(f"BLOCKED: macOS worker restart verification failed ({type(exc).__name__}).", file=sys.stderr)
        print("Check the configured evidence path; preflight failures before the restart sequence may not produce an evidence file.", file=sys.stderr)
        return 1
    print(json.dumps(record, indent=2, sort_keys=True))
    print("PASS: macOS worker restart recovery assertions passed; Windows release gates remain separate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

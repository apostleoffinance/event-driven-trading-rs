#!/usr/bin/env python3
"""Fail-closed validator for the MT5 worker-restart evidence JSON.

This validates the recorded restart-recovery assertions only. It cannot establish
that the evidence is authentic, that certificate lifecycle review passed, or that
the target Windows host is production-ready.
"""
from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime
from pathlib import Path
from typing import Any


def parse_timestamp(value: Any, field: str) -> datetime:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{field} must be a non-empty ISO-8601 timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"{field} must be a valid ISO-8601 timestamp") from exc
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise ValueError(f"{field} must include a UTC offset")
    return parsed


def validate(record: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(record, dict):
        return ["evidence root must be a JSON object"]

    required_strings = {
        "worker_service": lambda value: bool(value.strip()),
        "daemon_service": lambda value: bool(value.strip()),
        "first_smoke": lambda value: value == "PASS",
        "second_smoke": lambda value: value == "PASS",
        "result": lambda value: value == "PASS_RESTART_RECOVERY",
    }
    for field, predicate in required_strings.items():
        value = record.get(field)
        if not isinstance(value, str) or not predicate(value):
            errors.append(f"{field} does not satisfy the required pass condition")

    if record.get("daemon_remained_running") is not True:
        errors.append("daemon_remained_running must be true")
    if record.get("sequence_advanced") is not True:
        errors.append("sequence_advanced must be true")
    if record.get("post_restart_reconciliation_fresh") is not True:
        errors.append("post_restart_reconciliation_fresh must be true")

    sequence_fields = ("pre_stop_market_sequence", "post_restart_market_sequence",
                       "post_restart_reconciliation_sequence")
    sequences: dict[str, int] = {}
    for field in sequence_fields:
        value = record.get(field)
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            errors.append(f"{field} must be a non-negative integer")
        else:
            sequences[field] = value
    if ("pre_stop_market_sequence" in sequences and
            "post_restart_market_sequence" in sequences and
            sequences["post_restart_market_sequence"] <= sequences["pre_stop_market_sequence"]):
        errors.append("post_restart_market_sequence must exceed pre_stop_market_sequence")

    timestamps: dict[str, datetime] = {}
    for field in ("started_at_utc", "worker_stopped_at_utc", "worker_restarted_at_utc",
                  "post_restart_market_captured_at", "post_restart_reconciliation_captured_at",
                  "finished_at_utc"):
        try:
            timestamps[field] = parse_timestamp(record.get(field), field)
        except ValueError as exc:
            errors.append(str(exc))

    if "worker_stopped_at_utc" in timestamps and "worker_restarted_at_utc" in timestamps:
        if timestamps["worker_restarted_at_utc"] < timestamps["worker_stopped_at_utc"]:
            errors.append("worker_restarted_at_utc precedes worker_stopped_at_utc")
    for field in ("post_restart_market_captured_at", "post_restart_reconciliation_captured_at"):
        if field in timestamps and "worker_stopped_at_utc" in timestamps:
            if timestamps[field] < timestamps["worker_stopped_at_utc"]:
                errors.append(f"{field} must be at or after worker_stopped_at_utc")
    if "finished_at_utc" in timestamps and "started_at_utc" in timestamps:
        if timestamps["finished_at_utc"] < timestamps["started_at_utc"]:
            errors.append("finished_at_utc precedes started_at_utc")

    if record.get("failure") not in (None, ""):
        errors.append("failure field must be absent or empty for a passing record")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="Path to mt5-demo-restart-verify.ps1 JSON output")
    args = parser.parse_args()
    try:
        record = json.loads(args.evidence.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"BLOCKED: cannot read valid evidence JSON ({type(exc).__name__}).", file=sys.stderr)
        return 2

    errors = validate(record)
    if errors:
        print("BLOCKED: restart evidence failed validation.")
        for error in errors:
            print(f"- {error}")
        print("This validator does not authorize venue execution.", file=sys.stderr)
        return 1

    print("PASS: recorded worker-restart evidence satisfies the structural acceptance checks.")
    print("This is not proof of evidence authenticity, certificate lifecycle conformance, or execution readiness.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

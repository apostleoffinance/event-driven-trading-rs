#!/usr/bin/env python3
"""Fail-closed structural validator for MT5-MVP-01A-3/4 acceptance evidence.

This validates completeness and internal consistency only. It does not authenticate
evidence, connect to MT5/PostgreSQL, approve account ownership, or authorize release.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
FORBIDDEN_KEYS = {
    "password", "mt5_password", "secret", "secret_value", "client_secret",
    "private_key", "server_key", "database_url", "connection_string",
    "access_token", "refresh_token", "broker_login", "raw_login",
}
REQUIRED_TEST_IDS = set(range(1, 22))


def _walk_for_secret_fields(value: Any, path: str = "$") -> list[str]:
    errors: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            normalized = str(key).strip().lower()
            if normalized in FORBIDDEN_KEYS or normalized.endswith("_private_key"):
                errors.append(f"{path}.{key} is a prohibited secret-bearing field")
            errors.extend(_walk_for_secret_fields(child, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            errors.extend(_walk_for_secret_fields(child, f"{path}[{index}]"))
    return errors


def _required_text(mapping: dict[str, Any], key: str, errors: list[str], prefix: str = "") -> None:
    value = mapping.get(key)
    if not isinstance(value, str) or not value.strip() or value.strip() != value:
        errors.append(f"{prefix}{key} must be a non-empty, unpadded string")


def _sha256(mapping: dict[str, Any], key: str, errors: list[str], prefix: str = "") -> None:
    value = mapping.get(key)
    if not isinstance(value, str) or not SHA256_RE.fullmatch(value):
        errors.append(f"{prefix}{key} must be a lowercase 64-character SHA-256 digest")


def _commit_sha(mapping: dict[str, Any], key: str, errors: list[str]) -> None:
    value = mapping.get(key)
    if not isinstance(value, str) or not COMMIT_RE.fullmatch(value):
        errors.append(f"{key} must be a lowercase 40-character Git commit SHA")


def _utc_timestamp(value: Any, field: str, errors: list[str]) -> None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{field} must be an ISO-8601 UTC timestamp")
        return
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        errors.append(f"{field} must be a valid ISO-8601 timestamp")
        return
    if parsed.tzinfo is None or parsed.utcoffset() is None or parsed.utcoffset().total_seconds() != 0:
        errors.append(f"{field} must include a zero UTC offset")
        return


def validate(record: Any) -> list[str]:
    if not isinstance(record, dict):
        return ["evidence root must be a JSON object"]

    errors = _walk_for_secret_fields(record)
    if record.get("evidence_schema_version") != 1:
        errors.append("evidence_schema_version must equal 1")
    if record.get("milestone") != "MT5-MVP-01A":
        errors.append("milestone must equal MT5-MVP-01A")
    if record.get("protocol_schema_version") != 2:
        errors.append("protocol_schema_version must equal the pinned v2 contract")
    if record.get("broker_environment") != "DEMO":
        errors.append("broker_environment must be DEMO")
    if record.get("quant_environment") != "PAPER":
        errors.append("quant_environment must be PAPER")
    if record.get("execution_enabled") is not False:
        errors.append("execution_enabled must be false")

    _commit_sha(record, "source_commit_sha", errors)
    _required_text(record, "captured_at_utc", errors)
    if "captured_at_utc" in record:
        _utc_timestamp(record.get("captured_at_utc"), "captured_at_utc", errors)

    artifacts = record.get("artifacts")
    if not isinstance(artifacts, dict):
        errors.append("artifacts must be an object")
        artifacts = {}
    for key in ("worker_sha256", "daemon_sha256", "terminal_sha256", "worker_certificate_sha256"):
        _sha256(artifacts, key, errors, "artifacts.")
    for key in ("worker_version", "daemon_version", "terminal_build", "python_version",
                "python_architecture", "metatrader5_package_version", "configuration_revision"):
        _required_text(artifacts, key, errors, "artifacts.")

    identity = record.get("identity")
    if not isinstance(identity, dict):
        errors.append("identity must be an object")
        identity = {}
    for key in ("organization_ref", "quant_account_ref", "external_account_ref",
                "binding_ref", "binding_revision", "broker_ref", "broker_server_ref",
                "terminal_id", "observed_server"):
        _required_text(identity, key, errors, "identity.")
    _sha256(identity, "observed_login_sha256", errors, "identity.")

    host = record.get("host")
    if not isinstance(host, dict):
        errors.append("host must be an object")
        host = {}
    for key in ("windows_edition", "windows_build", "architecture", "service_identity_sid"):
        _required_text(host, key, errors, "host.")
    if host.get("working_tree_clean") is not True:
        errors.append("host.working_tree_clean must be true for pinned release evidence")

    tests = record.get("acceptance_tests")
    if not isinstance(tests, list):
        errors.append("acceptance_tests must be an array")
        return errors
    seen: set[int] = set()
    for index, item in enumerate(tests):
        prefix = f"acceptance_tests[{index}]."
        if not isinstance(item, dict):
            errors.append(f"{prefix[:-1]} must be an object")
            continue
        test_id = item.get("test_id")
        if isinstance(test_id, bool) or not isinstance(test_id, int) or test_id not in REQUIRED_TEST_IDS:
            errors.append(f"{prefix}test_id must be an integer from 1 through 21")
            continue
        if test_id in seen:
            errors.append(f"{prefix}test_id {test_id} is duplicated")
        seen.add(test_id)
        if item.get("result") != "PASS":
            errors.append(f"{prefix}result must be PASS")
        _required_text(item, "evidence_ref", errors, prefix)
        _utc_timestamp(item.get("observed_at_utc"), prefix + "observed_at_utc", errors)

    missing = sorted(REQUIRED_TEST_IDS - seen)
    if missing:
        errors.append("acceptance_tests missing required test IDs: " + ", ".join(map(str, missing)))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="Redacted MT5-MVP-01A acceptance evidence JSON")
    args = parser.parse_args()
    try:
        record = json.loads(args.evidence.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"BLOCKED: cannot read valid evidence JSON ({type(exc).__name__}).", file=sys.stderr)
        return 2
    errors = validate(record)
    if errors:
        print("BLOCKED: MT5-MVP-01A acceptance evidence is incomplete or inconsistent.")
        for error in errors:
            print(f"- {error}")
        print("This validator does not authorize deployment or venue execution.", file=sys.stderr)
        return 1
    print("PASS: all 21 acceptance evidence records are structurally complete and internally consistent.")
    print("This is not proof of evidence authenticity, independent review, or release authorization.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

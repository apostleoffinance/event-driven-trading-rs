#!/usr/bin/env python3
"""Fail-closed QOS-015 release gate validator for protected evidence manifests.

The manifest is an operator attestation, not cryptographic proof. Validate the
referenced artifacts and obtain independent review before relying on a PASS.
This tool never enables or invokes venue execution.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REQUIRED_GATES = {
    "windows_demo_smoke": (
        "status", "evidence_ref", "actual_windows_host", "demo_account_masked",
        "broker_server", "account_currency", "exact_symbols", "fresh_quotes_specs",
        "market_data_persisted", "reconciliation_persisted", "distinct_sequences",
    ),
    "worker_restart": (
        "status", "evidence_ref", "restart_evidence_json_ref",
        "daemon_remained_running", "sequence_advanced", "fresh_reconciliation",
        "second_smoke_passed",
    ),
    "certificate_review": (
        "status", "evidence_ref", "client_server_cert_validity",
        "fingerprint_pin_verified", "ca_trust_reviewed", "revocation_enforcement_verified",
        "certificate_key_pair_verified",
    ),
    "private_key_acl_review": (
        "status", "evidence_ref", "client_key_acl_reviewed",
        "server_key_acl_reviewed", "parent_directories_effective_access_reviewed",
        "unreviewed_principals_resolved",
    ),
    "rotation_rollback": (
        "status", "evidence_ref", "rotation_tested", "rollback_tested",
        "old_or_untrusted_identity_rejection_tested", "rotation_owner",
    ),
    "qos015b_persistence": (
        "status", "evidence_ref", "target_host_feed_verified",
        "fresh_bid_ask_verified", "instrument_specs_verified",
        "postgres_persistence_verified", "stale_sequence_rejected",
        "account_terminal_identity_bound",
    ),
    "qos013_sizing": (
        "status", "evidence_ref", "account_currency_conversion_verified",
        "costs_and_slippage_inputs_verified", "margin_and_exposure_inputs_verified",
        "broker_volume_rounding_verified", "final_modeled_loss_verified",
        "persistent_sizing_audit_verified", "fail_closed_missing_data_tested",
    ),
    "independent_review": (
        "status", "evidence_ref", "reviewer", "reviewed_at_utc",
        "reviewed_manifest_sha256", "no_execution_enabled",
    ),
}


def validate(manifest: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(manifest, dict):
        return ["manifest root must be a JSON object"]
    if manifest.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if manifest.get("venue_execution_enabled") is not False:
        errors.append("venue_execution_enabled must be explicitly false")
    commit = manifest.get("repository_commit")
    if not isinstance(commit, str) or len(commit) not in (40, 64) or any(c not in "0123456789abcdefABCDEF" for c in commit):
        errors.append("repository_commit must be a 40- or 64-character commit hash")
    operator = manifest.get("operator")
    if not isinstance(operator, str) or not operator.strip():
        errors.append("operator must be a non-empty string")
        operator = None
    gates = manifest.get("gates")
    if not isinstance(gates, dict):
        return errors + ["gates must be a JSON object"]

    for gate_name, required_fields in REQUIRED_GATES.items():
        gate = gates.get(gate_name)
        if not isinstance(gate, dict):
            errors.append(f"gates.{gate_name} is missing or not an object")
            continue
        if gate.get("status") != "PASS":
            errors.append(f"gates.{gate_name}.status must be PASS")
        for field in required_fields:
            value = gate.get(field)
            if field == "status":
                continue
            if isinstance(value, str):
                normalized = value.strip().upper()
                placeholders = {
                    "UNKNOWN", "N/A", "TBD", "FAIL", "BLOCKED", "UNVERIFIED",
                    "PROTECTED_EVIDENCE_REFERENCE", "PROTECTED_RESTART_JSON_REFERENCE",
                    "REPLACE_WITH_OPERATOR_ID", "REPLACE_WITH_64_CHARACTER_SHA256",
                    "INDEPENDENT_REVIEWER_ID", "UNASSIGNED",
                }
                if not value.strip() or normalized in placeholders or normalized.startswith("REPLACE_WITH_"):
                    errors.append(f"gates.{gate_name}.{field} must contain a resolved value")
            elif isinstance(value, bool):
                if value is not True:
                    errors.append(f"gates.{gate_name}.{field} must be true")
            elif isinstance(value, list):
                if not value or any(not isinstance(item, str) or not item.strip() for item in value):
                    errors.append(f"gates.{gate_name}.{field} must be a non-empty list of resolved values")
            else:
                errors.append(f"gates.{gate_name}.{field} must be a non-empty string, list, or true")

    reviewer_gate = gates.get("independent_review")
    if isinstance(reviewer_gate, dict):
        reviewer = reviewer_gate.get("reviewer")
        if isinstance(operator, str) and isinstance(reviewer, str) and reviewer.strip().casefold() == operator.strip().casefold():
            errors.append("independent reviewer must differ from operator")
        if reviewer_gate.get("no_execution_enabled") is not True:
            errors.append("independent review must explicitly confirm no venue execution was enabled")
        reviewed_at = reviewer_gate.get("reviewed_at_utc")
        if isinstance(reviewed_at, str):
            try:
                from datetime import datetime
                parsed = datetime.fromisoformat(reviewed_at.replace("Z", "+00:00"))
                if parsed.tzinfo is None or parsed.utcoffset() is None:
                    raise ValueError("timezone missing")
            except ValueError:
                errors.append("independent_review.reviewed_at_utc must be a valid timezone-aware ISO-8601 timestamp")
        manifest_hash = reviewer_gate.get("reviewed_manifest_sha256")
        if not isinstance(manifest_hash, str) or len(manifest_hash) != 64 or any(c not in "0123456789abcdefABCDEF" for c in manifest_hash):
            errors.append("independent_review.reviewed_manifest_sha256 must be a 64-character SHA-256 hex digest")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="Protected QOS-015 release evidence manifest JSON")
    args = parser.parse_args()
    try:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"BLOCKED: cannot read valid manifest ({type(exc).__name__}).", file=sys.stderr)
        return 2
    errors = validate(manifest)
    if errors:
        print("BLOCKED: one or more required release gates are missing, unresolved, or failed.")
        for error in errors:
            print(f"- {error}")
        print("No venue execution is authorized.", file=sys.stderr)
        return 1
    print("PASS: all required QOS-015 evidence gates are attested PASS.")
    print("Before relying on this result, independently verify artifact references and manifest hash.")
    print("This validator does not enable or authorize venue execution.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

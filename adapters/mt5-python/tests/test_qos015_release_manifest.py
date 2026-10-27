import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).parents[1] / "scripts" / "validate_qos015_release_manifest.py"
TEMPLATE = Path(__file__).parents[3] / "docs" / "QOS-015_RELEASE_EVIDENCE_MANIFEST_TEMPLATE.json"
SPEC = importlib.util.spec_from_file_location("validate_qos015_release_manifest", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def passing_manifest():
    yes = True
    gates = {
        "windows_demo_smoke": {
            "status": "PASS", "evidence_ref": "evidence/smoke.log",
            "actual_windows_host": "vps-demo-01", "demo_account_masked": "****1234",
            "broker_server": "Broker-Demo", "account_currency": "USD",
            "exact_symbols": ["EURUSD.a"], "fresh_quotes_specs": yes,
            "market_data_persisted": yes, "reconciliation_persisted": yes,
            "distinct_sequences": yes,
        },
        "worker_restart": {
            "status": "PASS", "evidence_ref": "evidence/restart.log",
            "restart_evidence_json_ref": "evidence/restart.json",
            "daemon_remained_running": yes, "sequence_advanced": yes,
            "fresh_reconciliation": yes, "second_smoke_passed": yes,
        },
        "certificate_review": {
            "status": "PASS", "evidence_ref": "evidence/cert-review.txt",
            "client_server_cert_validity": "verified in certificate report",
            "fingerprint_pin_verified": yes, "ca_trust_reviewed": yes,
            "revocation_enforcement_verified": yes, "certificate_key_pair_verified": yes,
        },
        "private_key_acl_review": {
            "status": "PASS", "evidence_ref": "evidence/acl-review.txt",
            "client_key_acl_reviewed": yes, "server_key_acl_reviewed": yes,
            "parent_directories_effective_access_reviewed": yes,
            "unreviewed_principals_resolved": yes,
        },
        "rotation_rollback": {
            "status": "PASS", "evidence_ref": "evidence/rotation-test.txt",
            "rotation_tested": yes, "rollback_tested": yes,
            "old_or_untrusted_identity_rejection_tested": yes, "rotation_owner": "ops-owner",
        },
        "qos015b_persistence": {
            "status": "PASS", "evidence_ref": "evidence/qos015b.txt",
            "target_host_feed_verified": yes, "fresh_bid_ask_verified": yes,
            "instrument_specs_verified": yes, "postgres_persistence_verified": yes,
            "stale_sequence_rejected": yes, "account_terminal_identity_bound": yes,
        },
        "qos013_sizing": {
            "status": "PASS", "evidence_ref": "evidence/qos013.txt",
            "account_currency_conversion_verified": yes, "costs_and_slippage_inputs_verified": yes,
            "margin_and_exposure_inputs_verified": yes, "broker_volume_rounding_verified": yes,
            "final_modeled_loss_verified": yes, "persistent_sizing_audit_verified": yes,
            "fail_closed_missing_data_tested": yes,
        },
        "independent_review": {
            "status": "PASS", "evidence_ref": "evidence/review.txt",
            "reviewer": "reviewer-2", "reviewed_at_utc": "2026-10-27T12:00:00Z",
            "reviewed_manifest_sha256": "a" * 64, "no_execution_enabled": yes,
        },
    }
    return {
        "schema_version": 1, "operator": "operator-1",
        "repository_commit": "0123456789abcdef0123456789abcdef01234567",
        "venue_execution_enabled": False, "gates": gates,
    }


class Qos015ReleaseManifestTests(unittest.TestCase):
    def test_all_passes_validate(self):
        self.assertEqual(MODULE.validate(passing_manifest()), [])

    def test_checked_in_template_is_blocked_by_default(self):
        template = json.loads(TEMPLATE.read_text(encoding="utf-8"))
        errors = MODULE.validate(template)
        self.assertTrue(errors)
        self.assertTrue(any("status must be PASS" in error for error in errors))

    def test_missing_gate_blocks(self):
        manifest = passing_manifest()
        del manifest["gates"]["qos013_sizing"]
        self.assertTrue(any("qos013_sizing is missing" in error for error in MODULE.validate(manifest)))

    def test_unknown_or_false_gate_blocks(self):
        manifest = passing_manifest()
        manifest["gates"]["certificate_review"]["revocation_enforcement_verified"] = False
        self.assertTrue(any("revocation_enforcement_verified must be true" in error for error in MODULE.validate(manifest)))

    def test_independent_reviewer_must_differ_from_operator(self):
        manifest = passing_manifest()
        manifest["gates"]["independent_review"]["reviewer"] = manifest["operator"]
        self.assertTrue(any("independent reviewer must differ from operator" in error for error in MODULE.validate(manifest)))

    def test_execution_must_remain_disabled(self):
        manifest = passing_manifest()
        manifest["venue_execution_enabled"] = True
        self.assertIn("venue_execution_enabled must be explicitly false", MODULE.validate(manifest))

    def test_cli_blocks_checked_in_template(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), str(TEMPLATE)],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("BLOCKED", result.stdout)
        self.assertIn("No venue execution is authorized", result.stderr)

    def test_cli_blocks_malformed_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_text("{", encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(path)],
                capture_output=True, text=True, check=False,
            )
        self.assertEqual(result.returncode, 2)
        self.assertIn("BLOCKED", result.stderr)


if __name__ == "__main__":
    unittest.main()

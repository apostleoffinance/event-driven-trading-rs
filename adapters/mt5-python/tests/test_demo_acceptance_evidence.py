import importlib.util
import unittest
from pathlib import Path
from datetime import datetime, timezone

SCRIPT = Path(__file__).parents[1] / "scripts" / "validate_demo_acceptance_evidence.py"
SPEC = importlib.util.spec_from_file_location("validate_demo_acceptance_evidence", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def passing_record():
    return {
        "evidence_schema_version": 1,
        "milestone": "MT5-MVP-01A",
        "protocol_schema_version": 2,
        "broker_environment": "DEMO",
        "quant_environment": "PAPER",
        "execution_enabled": False,
        "source_commit_sha": "a" * 40,
        "captured_at_utc": "2026-11-19T08:00:00Z",
        "artifacts": {
            "worker_sha256": "1" * 64,
            "daemon_sha256": "2" * 64,
            "terminal_sha256": "3" * 64,
            "worker_certificate_sha256": "4" * 64,
            "worker_version": "worker-1",
            "daemon_version": "daemon-1",
            "terminal_build": "build-1",
            "python_version": "3.12.8",
            "python_architecture": "64bit",
            "metatrader5_package_version": "5.0.45",
            "configuration_revision": "config-rev-1",
        },
        "identity": {
            "organization_ref": "org-ref",
            "quant_account_ref": "account-ref",
            "external_account_ref": "external-ref",
            "binding_ref": "binding-ref",
            "binding_revision": "revision-1",
            "broker_ref": "broker-ref",
            "broker_server_ref": "server-ref",
            "terminal_id": "terminal-01",
            "observed_server": "Broker-Demo",
            "observed_login_sha256": "5" * 64,
        },
        "host": {
            "windows_edition": "Windows Server",
            "windows_build": "20348",
            "architecture": "AMD64",
            "service_identity_sid": "S-1-5-21-1000",
            "working_tree_clean": True,
        },
        "acceptance_tests": [
            {
                "test_id": test_id,
                "result": "PASS",
                "observed_at_utc": "2026-11-19T08:00:00Z",
                "evidence_ref": f"evidence://test-{test_id}",
            }
            for test_id in range(1, 22)
        ],
    }


class DemoAcceptanceEvidenceTests(unittest.TestCase):
    def test_accepts_complete_pinned_demo_evidence(self):
        self.assertEqual(MODULE.validate(passing_record()), [])

    def test_rejects_missing_acceptance_test(self):
        record = passing_record()
        record["acceptance_tests"].pop()
        self.assertTrue(any("missing required test IDs: 21" in e for e in MODULE.validate(record)))

    def test_rejects_duplicate_test_id(self):
        record = passing_record()
        record["acceptance_tests"][-1]["test_id"] = 20
        self.assertTrue(any("is duplicated" in e for e in MODULE.validate(record)))

    def test_rejects_failed_or_blocked_test(self):
        record = passing_record()
        record["acceptance_tests"][16]["result"] = "BLOCKED"
        self.assertTrue(any("result must be PASS" in e for e in MODULE.validate(record)))

    def test_rejects_live_or_unknown_environment(self):
        record = passing_record()
        record["broker_environment"] = "LIVE"
        self.assertIn("broker_environment must be DEMO", MODULE.validate(record))

    def test_rejects_execution_enabled(self):
        record = passing_record()
        record["execution_enabled"] = True
        self.assertIn("execution_enabled must be false", MODULE.validate(record))

    def test_rejects_unpinned_artifact_hash(self):
        record = passing_record()
        record["artifacts"]["daemon_sha256"] = "not-a-hash"
        self.assertIn(
            "artifacts.daemon_sha256 must be a lowercase 64-character SHA-256 digest",
            MODULE.validate(record),
        )

    def test_rejects_non_utc_timestamp(self):
        record = passing_record()
        record["captured_at_utc"] = "2026-11-19T09:00:00+01:00"
        self.assertIn("captured_at_utc must include a zero UTC offset", MODULE.validate(record))

    def test_rejects_secret_bearing_fields(self):
        record = passing_record()
        record["artifacts"]["private_key"] = "never-allow-this"
        self.assertTrue(any("prohibited secret-bearing field" in e for e in MODULE.validate(record)))

    def test_rejects_dirty_source_tree(self):
        record = passing_record()
        record["host"]["working_tree_clean"] = False
        self.assertIn("host.working_tree_clean must be true for pinned release evidence", MODULE.validate(record))


if __name__ == "__main__":
    unittest.main()

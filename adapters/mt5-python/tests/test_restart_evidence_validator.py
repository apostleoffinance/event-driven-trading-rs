import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path

SCRIPT = Path(__file__).parents[1] / "scripts" / "validate_restart_evidence.py"
SPEC = importlib.util.spec_from_file_location("validate_restart_evidence", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def passing_record():
    return {
        "started_at_utc": "2026-10-27T10:00:00+00:00",
        "worker_service": "QuantOS-MT5-Worker",
        "daemon_service": "QuantOS-MT5-Bridge",
        "first_smoke": "PASS",
        "worker_stopped_at_utc": "2026-10-27T10:01:00+00:00",
        "worker_restarted_at_utc": "2026-10-27T10:01:10+00:00",
        "daemon_remained_running": True,
        "pre_stop_market_sequence": 100,
        "post_restart_market_sequence": 102,
        "post_restart_reconciliation_sequence": 102,
        "post_restart_market_captured_at": "2026-10-27T10:01:15+00:00",
        "post_restart_reconciliation_captured_at": "2026-10-27T10:01:16+00:00",
        "sequence_advanced": True,
        "post_restart_reconciliation_fresh": True,
        "second_smoke": "PASS",
        "result": "PASS_RESTART_RECOVERY",
        "finished_at_utc": "2026-10-27T10:02:00+00:00",
    }


class RestartEvidenceValidatorTests(unittest.TestCase):
    def test_accepts_complete_passing_record(self):
        self.assertEqual(MODULE.validate(passing_record()), [])

    def test_rejects_sequence_that_did_not_advance_even_if_flag_is_true(self):
        record = passing_record()
        record["post_restart_market_sequence"] = 100
        self.assertIn(
            "post_restart_market_sequence must exceed pre_stop_market_sequence",
            MODULE.validate(record),
        )

    def test_rejects_reconciliation_timestamp_before_worker_stop(self):
        record = passing_record()
        record["post_restart_reconciliation_captured_at"] = "2026-10-27T10:00:59+00:00"
        self.assertIn(
            "post_restart_reconciliation_captured_at must be at or after worker_stopped_at_utc",
            MODULE.validate(record),
        )

    def test_rejects_naive_timestamp(self):
        record = passing_record()
        record["worker_stopped_at_utc"] = "2026-10-27T10:01:00"
        self.assertIn(
            "worker_stopped_at_utc must include a UTC offset",
            MODULE.validate(record),
        )

    def test_rejects_failed_or_unknown_smoke_status(self):
        record = passing_record()
        record["second_smoke"] = "UNKNOWN"
        self.assertIn(
            "second_smoke does not satisfy the required pass condition",
            MODULE.validate(record),
        )

    def test_rejects_failure_field_on_passing_record(self):
        record = passing_record()
        record["failure"] = "worker never recovered"
        self.assertIn(
            "failure field must be absent or empty for a passing record",
            MODULE.validate(record),
        )

    def test_cli_fails_closed_for_malformed_json(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            path.write_text("{not json", encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(path)],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 2)
        self.assertIn("BLOCKED", result.stderr)


if __name__ == "__main__":
    unittest.main()

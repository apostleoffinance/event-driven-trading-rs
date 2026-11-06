import importlib.util
import unittest
from pathlib import Path
from datetime import datetime, timezone


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"


def load_script(name, module_name):
    spec = importlib.util.spec_from_file_location(module_name, SCRIPTS / name)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {name}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SMOKE = load_script("mt5-macos-demo-smoke.py", "mt5_macos_demo_smoke_test")
RESTART = load_script("mt5-macos-restart-verify.py", "mt5_macos_restart_verify_test")


class MacOSConformanceHelperTests(unittest.TestCase):
    def test_smoke_timestamp_parser_accepts_timezone_aware_iso8601(self):
        parsed = SMOKE.parse_time("2026-10-27T12:00:00Z", "captured_at")
        self.assertEqual(parsed.utcoffset().total_seconds(), 0)

    def test_smoke_timestamp_parser_rejects_timezone_naive_value(self):
        with self.assertRaises(Exception):
            SMOKE.parse_time("2026-10-27T12:00:00", "captured_at")

    def test_restart_evidence_writer_restricts_permissions(self):
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "evidence.json"
            RESTART.write_evidence(target, {"result": "BLOCKED", "venue_execution_enabled": False})
            self.assertTrue(target.exists())
            self.assertEqual(target.stat().st_mode & 0o777, 0o600)


    @staticmethod
    def persisted_fixture():
        from datetime import timedelta
        now = datetime(2026, 11, 6, 12, 0, tzinfo=timezone.utc)
        captured = (now - timedelta(seconds=10)).isoformat()
        return now, {
            "market_sequence": 41,
            "market_captured_at": captured,
            "market_source": "mt5_macos_mql5_socket_v1",
            "market_account_currency": "USD",
            "market_server": "Demo-Server",
            "reconciliation_sequence": 41,
            "reconciliation_captured_at": captured,
            "reconciliation_source": "mt5_macos_mql5_socket_v1",
            "reconciliation_currency": "USD",
            "reconciliation_server": "Demo-Server",
        }

    def test_persisted_gate_accepts_fresh_matching_identity_and_sequence(self):
        from types import SimpleNamespace
        now, state = self.persisted_fixture()
        sequence, market_at, recon_at = SMOKE.validate_persisted_state(
            SimpleNamespace(server="Demo-Server"), "USD", state, now=now
        )
        self.assertEqual(sequence, 41)
        self.assertEqual(market_at, recon_at)

    def test_persisted_gate_rejects_stale_market_snapshot(self):
        from types import SimpleNamespace
        from datetime import timedelta
        now, state = self.persisted_fixture()
        state["market_captured_at"] = (now - timedelta(seconds=61)).isoformat()
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(SimpleNamespace(server="Demo-Server"), "USD", state, now=now)

    def test_persisted_gate_rejects_future_snapshot_beyond_tolerance(self):
        from types import SimpleNamespace
        from datetime import timedelta
        now, state = self.persisted_fixture()
        state["reconciliation_captured_at"] = (now + timedelta(seconds=6)).isoformat()
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(SimpleNamespace(server="Demo-Server"), "USD", state, now=now)

    def test_persisted_gate_rejects_market_reconciliation_sequence_mismatch(self):
        from types import SimpleNamespace
        now, state = self.persisted_fixture()
        state["reconciliation_sequence"] = 42
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(SimpleNamespace(server="Demo-Server"), "USD", state, now=now)

    def test_persisted_gate_rejects_wrong_server_and_currency(self):
        from types import SimpleNamespace
        now, state = self.persisted_fixture()
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(SimpleNamespace(server="Wrong-Server"), "USD", state, now=now)
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(SimpleNamespace(server="Demo-Server"), "EUR", state, now=now)

    def test_persisted_gate_rejects_nonpositive_sequence(self):
        from types import SimpleNamespace
        now, state = self.persisted_fixture()
        state["market_sequence"] = 0
        state["reconciliation_sequence"] = 0
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(SimpleNamespace(server="Demo-Server"), "USD", state, now=now)

    def test_persisted_gate_rejects_naive_validation_clock(self):
        from types import SimpleNamespace
        now, state = self.persisted_fixture()
        with self.assertRaises(SMOKE.AdapterError):
            SMOKE.validate_persisted_state(
                SimpleNamespace(server="Demo-Server"), "USD", state, now=datetime(2026, 11, 6, 12, 0)
            )

if __name__ == "__main__":
    unittest.main()

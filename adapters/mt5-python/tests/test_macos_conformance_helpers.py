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


if __name__ == "__main__":
    unittest.main()

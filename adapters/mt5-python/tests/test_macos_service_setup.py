from __future__ import annotations

import importlib.util
import plistlib
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "setup-macos-services.py"
SPEC = importlib.util.spec_from_file_location("setup_macos_services", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
SETUP = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SETUP)


class MacOSServiceSetupTests(unittest.TestCase):
    def test_shell_quote_preserves_spaces_and_shell_metacharacters(self) -> None:
        value = "/Users/demo/Library/Application Support/QuantOS;$(echo unsafe)"
        quoted = SETUP.shell_quote(value)
        self.assertEqual(quoted, "'/Users/demo/Library/Application Support/QuantOS;$(echo unsafe)'")

    def test_build_plist_has_expected_launchd_identity_and_wrapper(self) -> None:
        wrapper = Path("/repo/adapters/mt5-python/scripts/mt5-macos-launch-wrapper.sh")
        env_file = Path("/Users/demo/Library/Application Support/QuantOS/quantos.env")
        repo = Path("/repo")
        plist_bytes = SETUP.build_plist(
            SETUP.WORKER_LABEL,
            wrapper,
            env_file,
            ["/repo/.venv/bin/python", "-m", "mt5_adapter.worker"],
            repo / "adapters/mt5-python",
            Path("/Users/demo/Library/Application Support/QuantOS/logs/worker.out"),
            Path("/Users/demo/Library/Application Support/QuantOS/logs/worker.err"),
        )
        record = plistlib.loads(plist_bytes)
        self.assertEqual(record["Label"], SETUP.WORKER_LABEL)
        self.assertEqual(record["ProgramArguments"][:2], ["/bin/bash", str(wrapper)])
        self.assertEqual(record["ProgramArguments"][-2:], ["-m", "mt5_adapter.worker"])
        self.assertEqual(record["EnvironmentVariables"]["QUANTOS_ENV_FILE"], str(env_file))
        self.assertTrue(record["RunAtLoad"])
        self.assertTrue(record["KeepAlive"])

    def test_bridge_and_worker_labels_are_distinct(self) -> None:
        self.assertNotEqual(SETUP.BRIDGE_LABEL, SETUP.WORKER_LABEL)
        self.assertEqual(SETUP.BRIDGE_LABEL, "com.quantos.mt5-bridge")
        self.assertEqual(SETUP.WORKER_LABEL, "com.quantos.mt5-worker")


if __name__ == "__main__":
    unittest.main()

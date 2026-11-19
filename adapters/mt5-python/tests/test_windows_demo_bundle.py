"""Tests for the secret-free Windows DEMO deployment bundle packager."""
from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "package_windows_demo_bundle.py"
SPEC = importlib.util.spec_from_file_location("package_windows_demo_bundle", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class WindowsDemoBundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        (self.root / "Cargo.lock").write_text("# pinned fixture lockfile\\n", encoding="utf-8")
        for item in MODULE.INCLUDE_FILES:
            target = self.root / item
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("safe deployment content\n", encoding="utf-8")
        adapter = self.root / "adapters/mt5-python/mt5_adapter"
        (adapter / "__init__.py").write_text("", encoding="utf-8")
        (adapter / "worker.py").write_text("def main(): pass\n", encoding="utf-8")
        (adapter / "windows_instance.py").write_text("class WindowsInstance: pass\n", encoding="utf-8")
        (adapter / "config.py").write_text("SAFE = True\n", encoding="utf-8")
        self.daemon = self.root / "mt5-bridge-daemon.exe"
        self.daemon.write_bytes(b"MZ" + b"\x00" * 128)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(["git", "-C", str(self.root), "config", "user.email", "ci@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(self.root), "config", "user.name", "CI"], check=True)
        subprocess.run(["git", "-C", str(self.root), "add", "."], check=True)
        subprocess.run(["git", "-C", str(self.root), "commit", "-qm", "fixture"], check=True)
        self.output = Path(self.temp.name) / "release.zip"

    def tearDown(self):
        self.temp.cleanup()

    def test_builds_pinned_secret_free_demo_paper_bundle(self):
        result = MODULE.build_bundle(self.root, self.daemon, self.output)
        self.assertTrue(self.output.is_file())
        self.assertEqual(result["manifest"]["broker_environment"], "DEMO")
        self.assertEqual(result["manifest"]["quant_environment"], "PAPER")
        self.assertIs(result["manifest"]["execution_enabled"], False)
        with zipfile.ZipFile(self.output) as archive:
            names = set(archive.namelist())
            self.assertIn("bin/mt5-bridge-daemon.exe", names)
            self.assertIn("adapters/mt5-python/mt5_adapter/config.py", names)
            self.assertIn("manifest/release-manifest.json", names)
            self.assertIn("manifest/windows-resolved-Cargo.lock", names)
            manifest = json.loads(archive.read("manifest/release-manifest.json"))
            self.assertEqual(manifest["source_commit_sha"], result["manifest"]["source_commit_sha"])
            self.assertFalse(any(name.lower().endswith((".pem", ".key", ".p12", ".pfx")) or name.lower().split("/")[-1] in {".env", ".env.local", "id_rsa", "id_ed25519"} for name in names))
            self.assertNotIn("MT5_PASSWORD", archive.read("manifest/release-manifest.json").decode())

    def test_rejects_non_windows_executable(self):
        self.daemon.write_bytes(b"not-a-pe")
        with self.assertRaisesRegex(ValueError, "not a Windows PE"):
            MODULE.build_bundle(self.root, self.daemon, self.output)

    def test_allows_only_windows_resolved_cargo_lockfile_delta(self):
        lockfile = self.root / "Cargo.lock"
        lockfile.write_text("# windows target dependency resolution\\n", encoding="utf-8")
        result = MODULE.build_bundle(self.root, self.daemon, self.output)
        with zipfile.ZipFile(self.output) as archive:
            bundled = archive.read("manifest/windows-resolved-Cargo.lock")
        self.assertEqual(bundled, lockfile.read_bytes())
        self.assertEqual(result["manifest"]["resolved_cargo_lock_sha256"], MODULE.sha256(lockfile))

    def test_rejects_dirty_source_checkout(self):
        (self.root / "uncommitted.txt").write_text("change", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "must be clean"):
            MODULE.build_bundle(self.root, self.daemon, self.output)

    def test_manifest_keeps_external_deployment_gates_explicit(self):
        result = MODULE.build_bundle(self.root, self.daemon, self.output)
        gates = result["manifest"]["external_gates"]
        self.assertTrue(any("secret manager" in gate for gate in gates))
        self.assertTrue(any("TenTrade DEMO" in gate for gate in gates))
        self.assertTrue(any("21 Windows acceptance" in gate for gate in gates))
        self.assertIn("not bundled; operator must pin and verify", result["manifest"]["worker"]["metatrader5_package"])


if __name__ == "__main__":
    unittest.main()

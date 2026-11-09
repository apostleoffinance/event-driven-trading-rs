from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import qos015c_runtime_inventory as inventory


class RuntimeInventoryHashTests(unittest.TestCase):
    def test_hashes_regular_file_and_reports_size(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.bin"
            payload = b"read-only inventory fixture"
            path.write_bytes(payload)
            result = inventory.sha256(path)
            self.assertTrue(result["exists"])
            self.assertTrue(result["regular_file"])
            self.assertEqual(result["size_bytes"], len(payload))
            self.assertEqual(result["sha256"], hashlib.sha256(payload).hexdigest())

    def test_missing_file_is_unknown_not_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = inventory.sha256(Path(directory) / "missing.bin")
            self.assertEqual(result, {"exists": False})

    def test_symlink_is_not_hashed_as_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target.bin"
            link = Path(directory) / "link.bin"
            target.write_bytes(b"artifact")
            link.symlink_to(target)
            result = inventory.sha256(link)
            self.assertTrue(result["exists"])
            self.assertFalse(result["regular_file"])
            self.assertTrue(result["symlink"])

    def test_git_value_reads_checkout_identity_without_mutation(self) -> None:
        head = inventory.git_value(REPO_ROOT, "rev-parse", "HEAD")
        self.assertIsNotNone(head)
        self.assertEqual(len(head), 40)


if __name__ == "__main__":
    unittest.main()

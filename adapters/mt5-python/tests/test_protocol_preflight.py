import argparse
import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.mt5_protocol_preflight import verify


class ProtocolPreflightTests(unittest.TestCase):
    def test_accepts_matching_staged_contract_and_ea_hashes(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "adapters/mt5-python/mt5_adapter").mkdir(parents=True)
            (root / "crates/intent-bridge/src").mkdir(parents=True)
            (root / "adapters/mt5-python/mt5_adapter/worker.py").write_text(
                'payload = {"schema_version": 2}\n', encoding="utf-8"
            )
            (root / "crates/intent-bridge/src/mt5_v1.rs").write_text(
                "pub const MT5_BRIDGE_SCHEMA_VERSION: u16 = 2;\n", encoding="utf-8"
            )
            (root / "adapters/mt5-python/mt5_adapter/macos_socket_provider.py").write_text(
                'MQL5_PROVIDER_PROTOCOL_VERSION = 1\nMQL5_PROVIDER_BUILD_VERSION = "2.00"\n',
                encoding="utf-8",
            )
            (root / "adapters/mt5-python/mql5").mkdir()
            ea_source = root / "adapters/mt5-python/mql5/provider.mq5"
            ea_source.write_text(
                '#property version   "2.00"\n'
                "const int QUANTOS_PROVIDER_PROTOCOL_VERSION = 1;\n"
                'const string QUANTOS_PROVIDER_BUILD_VERSION = "2.00";\n',
                encoding="utf-8",
            )
            daemon = root / "target/release/mt5-bridge-daemon"
            daemon.parent.mkdir(parents=True)
            daemon.write_bytes(b"staged-daemon")
            daemon.chmod(0o755)
            ea_binary = root / "provider.ex5"
            ea_binary.write_bytes(b"EX5\x02compiled-ea")
            arguments = argparse.Namespace(
                repo_root=root,
                daemon_binary=daemon,
                ea_source=ea_source,
                ea_binary=ea_binary,
                expected_worker_sha256=hashlib.sha256(
                    (root / "adapters/mt5-python/mt5_adapter/worker.py").read_bytes()
                ).hexdigest(),
                expected_provider_sha256=hashlib.sha256(
                    (root / "adapters/mt5-python/mt5_adapter/macos_socket_provider.py").read_bytes()
                ).hexdigest(),
                expected_daemon_sha256=hashlib.sha256(daemon.read_bytes()).hexdigest(),
                expected_daemon_size_bytes=daemon.stat().st_size,
                expected_ea_source_sha256=hashlib.sha256(ea_source.read_bytes()).hexdigest(),
                expected_ea_binary_sha256=hashlib.sha256(ea_binary.read_bytes()).hexdigest(),
            )
            verify(arguments)

    def test_rejects_mismatched_daemon_schema(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "adapters/mt5-python/mt5_adapter").mkdir(parents=True)
            (root / "crates/intent-bridge/src").mkdir(parents=True)
            (root / "adapters/mt5-python/mt5_adapter/worker.py").write_text(
                'payload = {"schema_version": 2}\n', encoding="utf-8"
            )
            (root / "crates/intent-bridge/src/mt5_v1.rs").write_text(
                "pub const MT5_BRIDGE_SCHEMA_VERSION: u16 = 1;\n", encoding="utf-8"
            )
            (root / "adapters/mt5-python/mt5_adapter/macos_socket_provider.py").write_text(
                'MQL5_PROVIDER_PROTOCOL_VERSION = 1\nMQL5_PROVIDER_BUILD_VERSION = "2.00"\n',
                encoding="utf-8",
            )
            (root / "adapters/mt5-python/mql5").mkdir()
            ea_source = root / "adapters/mt5-python/mql5/provider.mq5"
            ea_source.write_text(
                '#property version   "2.00"\n'
                "const int QUANTOS_PROVIDER_PROTOCOL_VERSION = 1;\n"
                'const string QUANTOS_PROVIDER_BUILD_VERSION = "2.00";\n',
                encoding="utf-8",
            )
            daemon = root / "target/release/mt5-bridge-daemon"
            daemon.parent.mkdir(parents=True)
            daemon.write_bytes(b"staged-daemon")
            daemon.chmod(0o755)
            ea_binary = root / "provider.ex5"
            ea_binary.write_bytes(b"EX5\x02compiled-ea")
            with self.assertRaisesRegex(ValueError, "Rust daemon bridge"):
                verify(argparse.Namespace(
                    repo_root=root,
                    daemon_binary=daemon,
                    ea_source=ea_source,
                    ea_binary=ea_binary,
                    expected_worker_sha256=hashlib.sha256(
                        (root / "adapters/mt5-python/mt5_adapter/worker.py").read_bytes()
                    ).hexdigest(),
                    expected_provider_sha256=hashlib.sha256(
                        (root / "adapters/mt5-python/mt5_adapter/macos_socket_provider.py").read_bytes()
                    ).hexdigest(),
                    expected_daemon_sha256=hashlib.sha256(daemon.read_bytes()).hexdigest(),
                    expected_daemon_size_bytes=daemon.stat().st_size,
                    expected_ea_source_sha256=hashlib.sha256(ea_source.read_bytes()).hexdigest(),
                    expected_ea_binary_sha256=hashlib.sha256(ea_binary.read_bytes()).hexdigest(),
                ))


def _sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class PreflightArtifactTests(unittest.TestCase):
    def setUp(self):
        self._temp = tempfile.TemporaryDirectory()
        self.addCleanup(self._temp.cleanup)
        base = Path(self._temp.name)
        self.root = base / "repo"
        (self.root / "adapters/mt5-python/mt5_adapter").mkdir(parents=True)
        (self.root / "crates/intent-bridge/src").mkdir(parents=True)
        (self.root / "adapters/mt5-python/mql5").mkdir()
        adapter = self.root / "adapters/mt5-python/mt5_adapter"
        (adapter / "worker.py").write_text('payload = {"schema_version": 2}\n', encoding="utf-8")
        (self.root / "crates/intent-bridge/src/mt5_v1.rs").write_text(
            "pub const MT5_BRIDGE_SCHEMA_VERSION: u16 = 2;\n", encoding="utf-8"
        )
        (adapter / "macos_socket_provider.py").write_text(
            'MQL5_PROVIDER_PROTOCOL_VERSION = 1\nMQL5_PROVIDER_BUILD_VERSION = "2.00"\n',
            encoding="utf-8",
        )
        self.ea_source = self.root / "adapters/mt5-python/mql5/provider.mq5"
        self.ea_source.write_text(
            '#property version   "2.00"\n'
            "const int QUANTOS_PROVIDER_PROTOCOL_VERSION = 1;\n"
            'const string QUANTOS_PROVIDER_BUILD_VERSION = "2.00";\n',
            encoding="utf-8",
        )
        self.daemon = base / "remapped-build" / "mt5-bridge-daemon"
        self.daemon.parent.mkdir()
        self.daemon.write_bytes(b"rc-daemon")
        self.daemon.chmod(0o755)
        self.ea_binary = self.root / "provider.ex5"
        self.ea_binary.write_bytes(b"EX5\x02compiled")

    def args(self, **overrides):
        values = dict(
            repo_root=self.root,
            daemon_binary=self.daemon,
            ea_source=self.ea_source,
            ea_binary=self.ea_binary,
            expected_worker_sha256=_sha(self.root / "adapters/mt5-python/mt5_adapter/worker.py"),
            expected_provider_sha256=_sha(
                self.root / "adapters/mt5-python/mt5_adapter/macos_socket_provider.py"
            ),
            expected_daemon_sha256=_sha(self.daemon),
            expected_daemon_size_bytes=self.daemon.stat().st_size,
            expected_ea_source_sha256=_sha(self.ea_source),
            expected_ea_binary_sha256=_sha(self.ea_binary),
        )
        values.update(overrides)
        return argparse.Namespace(**values)

    def test_accepts_explicit_external_rc_daemon_path(self):
        verify(self.args())

    def test_rejects_incorrect_daemon_hash(self):
        with self.assertRaisesRegex(ValueError, "Rust daemon hash"):
            verify(self.args(expected_daemon_sha256="0" * 64))

    def test_rejects_incorrect_daemon_size_metadata(self):
        with self.assertRaisesRegex(ValueError, "size does not match"):
            verify(self.args(expected_daemon_size_bytes=self.daemon.stat().st_size + 1))

    def test_rejects_invalid_daemon_hash_metadata(self):
        with self.assertRaisesRegex(ValueError, "exactly 64 hexadecimal"):
            verify(self.args(expected_daemon_sha256="not-a-sha256"))

    def test_rejects_missing_daemon_size_metadata(self):
        values = vars(self.args())
        values.pop("expected_daemon_size_bytes")
        with self.assertRaisesRegex(ValueError, "positive integer"):
            verify(argparse.Namespace(**values))

    def test_rejects_incorrect_ea_source_hash(self):
        with self.assertRaisesRegex(ValueError, "EA source hash"):
            verify(self.args(expected_ea_source_sha256="0" * 64))

    def test_rejects_incorrect_ea_protocol_version(self):
        self.ea_source.write_text(
            '#property version   "2.00"\n'
            "const int QUANTOS_PROVIDER_PROTOCOL_VERSION = 2;\n"
            'const string QUANTOS_PROVIDER_BUILD_VERSION = "2.00";\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "MQL5 EA provider"):
            verify(self.args(expected_ea_source_sha256=_sha(self.ea_source)))

    def test_rejects_missing_daemon(self):
        self.daemon.unlink()
        with self.assertRaises(OSError):
            verify(self.args())

    def test_rejects_missing_ea_binary(self):
        self.ea_binary.unlink()
        with self.assertRaises(OSError):
            verify(self.args())

    def test_rejects_non_executable_daemon(self):
        self.daemon.chmod(0o644)
        with self.assertRaisesRegex(ValueError, "regular executable file"):
            verify(self.args())

    def test_rejects_daemon_directory(self):
        arguments = self.args()
        self.daemon.unlink()
        self.daemon.mkdir()
        self.daemon.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "regular executable file"):
            verify(arguments)

    def test_rejects_empty_ea_binary(self):
        self.ea_binary.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "non-empty regular file"):
            verify(self.args(expected_ea_binary_sha256=_sha(self.ea_binary)))

    def test_rejects_ea_binary_with_invalid_header(self):
        self.ea_binary.write_bytes(b"MZ-not-an-ex5")
        with self.assertRaisesRegex(ValueError, "invalid header"):
            verify(self.args(expected_ea_binary_sha256=_sha(self.ea_binary)))

    def test_rejects_symlinked_daemon(self):
        link = self.daemon.parent / "link-daemon"
        link.symlink_to(self.daemon)
        with self.assertRaisesRegex(ValueError, "symbolic links"):
            verify(self.args(daemon_binary=link))


if __name__ == "__main__":
    unittest.main()

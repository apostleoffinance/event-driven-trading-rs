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
            ea_binary.write_bytes(b"compiled-ea")
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
            ea_binary.write_bytes(b"compiled-ea")
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
                    expected_ea_source_sha256=hashlib.sha256(ea_source.read_bytes()).hexdigest(),
                    expected_ea_binary_sha256=hashlib.sha256(ea_binary.read_bytes()).hexdigest(),
                ))


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Fail closed unless staged MT5 bridge sources and EA artifact are compatible."""
from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path


BRIDGE_SCHEMA_VERSION = 2
MQL5_PROVIDER_PROTOCOL_VERSION = 1
MQL5_PROVIDER_BUILD_VERSION = "2.00"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_version(path: Path, pattern: str, expected: int, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    values = re.findall(pattern, text)
    if not values or any(int(value) != expected for value in values):
        raise ValueError(f"{label} must declare protocol version {expected}")


def inside(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def verify(args: argparse.Namespace) -> None:
    root = args.repo_root.resolve(strict=True)
    daemon = args.daemon_binary.resolve(strict=True)
    ea_source = args.ea_source.resolve(strict=True)
    ea_binary = args.ea_binary.resolve(strict=True)

    if not inside(daemon, root):
        raise ValueError("daemon binary must be built inside the staged repository")
    if not daemon.is_file() or not daemon.stat().st_mode & 0o111:
        raise ValueError("staged daemon binary is not executable")
    if not inside(ea_source, root):
        raise ValueError("EA source must belong to the staged repository")

    adapter = root / "adapters/mt5-python/mt5_adapter"
    worker = adapter / "worker.py"
    provider = adapter / "macos_socket_provider.py"
    rust_protocol = root / "crates/intent-bridge/src/mt5_v1.rs"
    require_version(
        worker,
        r'"schema_version"\s*:\s*(\d+)',
        BRIDGE_SCHEMA_VERSION,
        "Python worker bridge",
    )
    require_version(
        rust_protocol,
        r"MT5_BRIDGE_SCHEMA_VERSION:\s*u16\s*=\s*(\d+)",
        BRIDGE_SCHEMA_VERSION,
        "Rust daemon bridge",
    )
    require_version(
        provider,
        r"MQL5_PROVIDER_PROTOCOL_VERSION\s*=\s*(\d+)",
        MQL5_PROVIDER_PROTOCOL_VERSION,
        "Python-to-EA provider",
    )
    require_version(
        ea_source,
        r"QUANTOS_PROVIDER_PROTOCOL_VERSION\s*=\s*(\d+)",
        MQL5_PROVIDER_PROTOCOL_VERSION,
        "MQL5 EA provider",
    )
    if '#property version   "2.00"' not in ea_source.read_text(encoding="utf-8"):
        raise ValueError("MQL5 EA build version must be 2.00")
    if f'MQL5_PROVIDER_BUILD_VERSION = "{MQL5_PROVIDER_BUILD_VERSION}"' not in provider.read_text(
        encoding="utf-8"
    ):
        raise ValueError("Python provider build version must be 2.00")
    if f'QUANTOS_PROVIDER_BUILD_VERSION = "{MQL5_PROVIDER_BUILD_VERSION}"' not in ea_source.read_text(
        encoding="utf-8"
    ):
        raise ValueError("MQL5 EA provider build version must be 2.00")

    actual_worker_hash = sha256_file(worker)
    actual_daemon_hash = sha256_file(daemon)
    actual_source_hash = sha256_file(ea_source)
    actual_binary_hash = sha256_file(ea_binary)
    expected_hashes = (
        (actual_worker_hash, args.expected_worker_sha256, "Python worker"),
        (
            sha256_file(provider),
            args.expected_provider_sha256,
            "Python terminal provider",
        ),
        (actual_daemon_hash, args.expected_daemon_sha256, "Rust daemon"),
        (actual_source_hash, args.expected_ea_source_sha256, "EA source"),
        (actual_binary_hash, args.expected_ea_binary_sha256, "EA binary"),
    )
    for actual, expected, label in expected_hashes:
        if actual.lower() != expected.lower():
            raise ValueError(f"{label} hash does not match the reviewed build manifest")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--daemon-binary", type=Path, required=True)
    parser.add_argument("--ea-source", type=Path, required=True)
    parser.add_argument("--ea-binary", type=Path, required=True)
    parser.add_argument("--expected-worker-sha256", required=True)
    parser.add_argument("--expected-provider-sha256", required=True)
    parser.add_argument("--expected-daemon-sha256", required=True)
    parser.add_argument("--expected-ea-source-sha256", required=True)
    parser.add_argument("--expected-ea-binary-sha256", required=True)
    args = parser.parse_args()
    try:
        verify(args)
    except (OSError, ValueError) as exc:
        print(f"BLOCKED: {exc}", file=sys.stderr)
        return 2
    print(
        "PASS: staged worker/Rust bridge schema v2 and EA provider protocol v1 "
        "are compatible; worker, provider, daemon, EA source, and EA binary hashes match "
        "the supplied build manifest."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

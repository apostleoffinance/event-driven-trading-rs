#!/usr/bin/env python3
"""Fail closed unless staged MT5 bridge sources and EA artifact are compatible."""
from __future__ import annotations

import argparse
import hashlib
import re
import stat
import sys
from pathlib import Path


BRIDGE_SCHEMA_VERSION = 2
MQL5_PROVIDER_PROTOCOL_VERSION = 1
MQL5_PROVIDER_BUILD_VERSION = "2.00"
EX5_MAGIC = b"EX5"


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
    if args.daemon_binary.is_symlink() or args.ea_binary.is_symlink():
        raise ValueError("artifacts must not be symbolic links")
    daemon = args.daemon_binary.resolve(strict=True)
    ea_source = args.ea_source.resolve(strict=True)
    ea_binary = args.ea_binary.resolve(strict=True)

    daemon_stat = daemon.stat()
    if not stat.S_ISREG(daemon_stat.st_mode) or not daemon_stat.st_mode & 0o111:
        raise ValueError("daemon artifact must be a regular executable file")
    expected_daemon_size = getattr(args, "expected_daemon_size_bytes", None)
    if not isinstance(expected_daemon_size, int) or expected_daemon_size <= 0:
        raise ValueError("expected daemon artifact size must be a positive integer")
    if daemon_stat.st_size != expected_daemon_size:
        raise ValueError("daemon artifact size does not match the reviewed build metadata")
    if not inside(ea_source, root):
        raise ValueError("EA source must belong to the staged repository")
    if not stat.S_ISREG(ea_binary.stat().st_mode) or ea_binary.stat().st_size == 0:
        raise ValueError("EA binary must be a non-empty regular file")
    with ea_binary.open("rb") as header:
        if header.read(3) != EX5_MAGIC:
            raise ValueError("EA binary is not an EX5 artifact (invalid header)")

    for label, digest in (
        ("Python worker", args.expected_worker_sha256),
        ("Python terminal provider", args.expected_provider_sha256),
        ("Rust daemon", args.expected_daemon_sha256),
        ("EA source", args.expected_ea_source_sha256),
        ("EA binary", args.expected_ea_binary_sha256),
    ):
        if not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
            raise ValueError(f"{label} SHA-256 metadata must contain exactly 64 hexadecimal characters")

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
    parser.add_argument(
        "--daemon-binary",
        type=Path,
        required=True,
        help="explicit path to the reviewed daemon artifact; SHA-256 and size are verified",
    )
    parser.add_argument("--ea-source", type=Path, required=True)
    parser.add_argument("--ea-binary", type=Path, required=True)
    parser.add_argument("--expected-worker-sha256", required=True)
    parser.add_argument("--expected-provider-sha256", required=True)
    parser.add_argument("--expected-daemon-sha256", required=True)
    parser.add_argument("--expected-daemon-size-bytes", type=int, required=True)
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
        "are compatible; worker, provider, daemon, EA source, and EA binary hashes "
        "match the supplied build manifest, and daemon size/executable metadata match."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

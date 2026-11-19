#!/usr/bin/env python3
"""Build a secret-free Windows DEMO deployment bundle from a pinned checkout.

This creates deployable artifacts only. It does not provision a host, install a
terminal, retrieve credentials, create certificates, migrate a database, or
claim live MT5 acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
INCLUDE_FILES = (
    "adapters/mt5-python/pyproject.toml",
    "adapters/mt5-python/README.md",
    "adapters/mt5-python/mt5_adapter/__init__.py",
    "adapters/mt5-python/scripts/mt5-windows-deployment-preflight.ps1",
    "adapters/mt5-python/scripts/mt5-demo-smoke.ps1",
    "adapters/mt5-python/scripts/mt5-demo-restart-verify.ps1",
    "adapters/mt5-python/scripts/capture-demo-acceptance-manifest.ps1",
    "adapters/mt5-python/scripts/validate_demo_acceptance_evidence.py",
    "adapters/mt5-python/scripts/validate_restart_evidence.py",
    "adapters/mt5-python/scripts/protect-mt5-password.ps1",
    "config/mt5/tentrade.demo.example.toml",
    "docs/MT5-MVP-01A_WINDOWS_DEMO_ACCEPTANCE_PLAN.md",
    "docs/MT5_WINDOWS_DEMO_DEPLOYMENT_ACCEPTANCE.md",
)

FORBIDDEN_PARTS = {
    ".env", ".env.local", ".env.production", "secrets", "secret",
    "credentials", "private", "private-key", "id_rsa", "id_ed25519",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_value(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        check=True, capture_output=True, text=True,
    )
    return result.stdout.strip()


def build_bundle(root: Path, daemon: Path, output: Path) -> dict:
    root = root.resolve()
    daemon = daemon.resolve()
    output = output.resolve()
    if not daemon.is_file():
        raise ValueError("Windows daemon executable does not exist")
    with daemon.open("rb") as stream:
        if stream.read(2) != b"MZ":
            raise ValueError("daemon artifact is not a Windows PE executable")
    status = git_value(root, "status", "--porcelain")
    if status:
        raise ValueError("source checkout must be clean before packaging")
    commit = git_value(root, "rev-parse", "HEAD")
    if len(commit) != 40 or any(c not in "0123456789abcdef" for c in commit):
        raise ValueError("source commit is not a lowercase 40-character SHA")

    missing = [item for item in INCLUDE_FILES if not (root / item).is_file()]
    if missing:
        raise ValueError("required bundle inputs are missing: " + ", ".join(missing))
    for item in INCLUDE_FILES:
        parts = {part.lower() for part in Path(item).parts}
        if parts & FORBIDDEN_PARTS:
            raise ValueError("refusing to package a secret-bearing path")

    adapter_files = sorted(
        (root / "adapters/mt5-python/mt5_adapter").glob("*.py")
    )
    if not adapter_files:
        raise ValueError("MT5 adapter Python sources are missing")
    adapter_hash = hashlib.sha256()
    for source in adapter_files:
        adapter_hash.update(source.name.encode("utf-8"))
        adapter_hash.update(bytes.fromhex(sha256(source)))

    manifest = {
        "manifest_schema_version": 1,
        "milestone": "MT5-MVP-01A",
        "source_commit_sha": commit,
        "broker_environment": "DEMO",
        "quant_environment": "PAPER",
        "execution_enabled": False,
        "protocol_schema_version": 2,
        "daemon": {
            "file": "bin/mt5-bridge-daemon.exe",
            "sha256": sha256(daemon),
            "architecture": "windows-amd64",
        },
        "worker": {
            "source_tree_sha256": adapter_hash.hexdigest(),
            "python_requirement": "3.12.x 64-bit; operator-pinned",
            "metatrader5_package": "not bundled; operator must pin and verify",
        },
        "external_gates": [
            "approved Windows host/provider and interactive-session supervisor",
            "approved secret manager and workload identity",
            "approved CA and worker/server certificates",
            "independently verified TenTrade DEMO server and account",
            "verified broker binding, READY gateway and terminal slot",
            "approved isolated database and migration/identity guard",
            "all 21 Windows acceptance checks and worker restart evidence",
        ],
        "limitations": [
            "Bundle creation is not host provisioning or deployment.",
            "No broker credentials, private keys, certificates or database URLs are included.",
            "No MetaTrader terminal or MetaTrader5 Python wheel is included.",
            "No live connection, data persistence or restart acceptance is asserted.",
        ],
    }

    output.parent.mkdir(parents=True, exist_ok=True)
    staging = output.parent / (output.stem + "-staging")
    if staging.exists():
        shutil.rmtree(staging)
    (staging / "bin").mkdir(parents=True)
    (staging / "manifest").mkdir()
    shutil.copy2(daemon, staging / "bin/mt5-bridge-daemon.exe")
    for item in INCLUDE_FILES:
        destination = staging / item
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / item, destination)
    for source in adapter_files:
        destination = staging / source.relative_to(root)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    (staging / "manifest/release-manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    if output.exists():
        output.unlink()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(staging.rglob("*")):
            if path.is_file():
                archive.write(path, path.relative_to(staging).as_posix())
    shutil.rmtree(staging)
    return {"output": str(output), "sha256": sha256(output), "manifest": manifest}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--daemon", type=Path, required=True, help="Built Windows mt5-bridge-daemon.exe")
    parser.add_argument("--output", type=Path, required=True, help="Output ZIP path")
    args = parser.parse_args()
    try:
        result = build_bundle(args.root, args.daemon, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"BLOCKED: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1
    print(json.dumps({
        "bundle": result["output"],
        "bundle_sha256": result["sha256"],
        "source_commit_sha": result["manifest"]["source_commit_sha"],
        "daemon_sha256": result["manifest"]["daemon"]["sha256"],
        "execution_enabled": False,
        "broker_environment": "DEMO",
        "deployment_status": "NOT_DEPLOYED",
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

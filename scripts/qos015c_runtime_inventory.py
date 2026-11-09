#!/usr/bin/env python3
"""Read-only QOS-015C runtime inventory. Never starts, stops, restarts, or mutates services."""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

PROJECT = "edgenest-mt5-bridge-v2-staging"
VOLUME = "edgenest_mt5_bridge_v2_staging_data"
CONTAINERS_FILTER = f"label=com.docker.compose.project={PROJECT}"
PORTS = (8765, 9443, 15432, 15433, 15543, 18765, 19443)
ARTIFACT_PATHS = {
    "python_worker_source": "adapters/mt5-python/mt5_adapter/worker.py",
    "python_provider_source": "adapters/mt5-python/mt5_adapter/macos_socket_provider.py",
    "ea_source": "adapters/mt5-python/mql5/QuantOSReadOnlyProvider.mq5",
}

def run(args: list[str], cwd: Path | None = None, timeout: int = 12) -> dict[str, Any]:
    if not shutil.which(args[0]):
        return {"available": False, "reason": f"{args[0]} not installed"}
    try:
        p = subprocess.run(args, cwd=cwd, text=True, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, timeout=timeout, check=False)
        return {"available": True, "exit_code": p.returncode,
                "stdout": p.stdout.strip(), "stderr": p.stderr.strip()[:500]}
    except subprocess.TimeoutExpired:
        return {"available": True, "error": "timed out"}
    except Exception as exc:
        return {"available": True, "error": type(exc).__name__}

def sha256(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"exists": False}
    if not path.is_file() or path.is_symlink():
        return {"exists": True, "regular_file": False, "symlink": path.is_symlink()}
    h = hashlib.sha256()
    size = 0
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
            size += len(chunk)
    return {"exists": True, "regular_file": True, "size_bytes": size, "sha256": h.hexdigest()}

def git_value(root: Path, *args: str) -> str | None:
    r = run(["git", *args], cwd=root)
    return r.get("stdout") if r.get("exit_code") == 0 else None

def docker_inventory(root: Path) -> dict[str, Any]:
    version = run(["docker", "version", "--format", "{{.Server.Version}}"])
    context = run(["docker", "context", "show"])
    ids = run(["docker", "ps", "-a", "--no-trunc", "--filter", CONTAINERS_FILTER,
               "--format", "{{.ID}}"])
    containers: list[dict[str, Any]] = []
    if ids.get("exit_code") == 0:
        for cid in filter(None, ids.get("stdout", "").splitlines()):
            fmt = "{{json .Config.Labels}}"
            labels_r = run(["docker", "inspect", "--format", fmt, cid])
            inspect_r = run(["docker", "inspect", "--format",
                "{{.Name}}|{{.Config.Image}}|{{.State.Status}}|{{if .State.Health}}{{.State.Health.Status}}{{else}}no-healthcheck{{end}}|{{json .Mounts}}|{{json .NetworkSettings.Ports}}",
                cid])
            raw = inspect_r.get("stdout", "")
            fields = raw.split("|", 5)
            labels = {}
            try:
                labels = json.loads(labels_r.get("stdout") or "{}")
            except json.JSONDecodeError:
                pass
            containers.append({
                "id": cid[:12],
                "name": fields[0].lstrip("/") if len(fields) > 0 else None,
                "image": fields[1] if len(fields) > 1 else None,
                "state": fields[2] if len(fields) > 2 else None,
                "health": fields[3] if len(fields) > 3 else None,
                "mounts": json.loads(fields[4]) if len(fields) > 4 and fields[4] else [],
                "ports": json.loads(fields[5]) if len(fields) > 5 and fields[5] else {},
                "compose_labels": {k: v for k, v in labels.items()
                    if k.startswith("com.docker.compose.") and k in {
                        "com.docker.compose.project", "com.docker.compose.service",
                        "com.docker.compose.project.config_files",
                        "com.docker.compose.project.working_dir",
                        "com.docker.compose.version"}},
            })
    volume_r = run(["docker", "volume", "inspect", VOLUME])
    volume = None
    if volume_r.get("exit_code") == 0:
        try:
            v = json.loads(volume_r["stdout"])[0]
            volume = {"name": v.get("Name"), "driver": v.get("Driver"),
                      "scope": v.get("Scope"), "mountpoint": v.get("Mountpoint"),
                      "labels": v.get("Labels") or {}}
        except (ValueError, KeyError, IndexError):
            volume = {"error": "could not parse volume inspection"}
    return {"server_version": version.get("stdout"), "context": context.get("stdout"),
            "containers": containers, "expected_external_volume": volume,
            "docker_cli_errors": [x for x in (version, context, ids, volume_r)
                                  if x.get("error") or x.get("exit_code", 0) not in (0, None)]}

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo", default=".", help="repository checkout (default: current directory)")
    ap.add_argument("--daemon-binary", help="exact daemon binary path to hash; never executed")
    ap.add_argument("--ea-source", action="append", default=[], help="installed EA source path; repeatable")
    ap.add_argument("--ea-binary", action="append", default=[], help="installed EA binary path; repeatable")
    ap.add_argument("--output", help="write JSON report to this path; default stdout")
    args = ap.parse_args()

    root = Path(args.repo).expanduser().resolve()
    if not (root / ".git").exists():
        print("ERROR: --repo must point to a Git working tree", file=sys.stderr)
        return 2

    manifest_path = root / "build-manifest.json"
    manifest: dict[str, Any] = {}
    if manifest_path.is_file():
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except Exception:
            manifest = {"parse_error": True}

    artifacts: dict[str, Any] = {}
    expected = manifest.get("artifacts", {})
    for name, rel in ARTIFACT_PATHS.items():
        actual = sha256(root / rel)
        expected_hash = expected.get(name, {}).get("sha256")
        artifacts[name] = {**actual, "path": rel, "manifest_sha256": expected_hash,
                           "matches_manifest": bool(expected_hash and actual.get("sha256") == expected_hash)}
    if args.daemon_binary:
        p = Path(args.daemon_binary).expanduser().resolve()
        actual = sha256(p)
        expected_hash = expected.get("rust_daemon_reproducible_remapped", {}).get("sha256")
        if not expected_hash:
            expected_hash = expected.get("rust_daemon", {}).get("sha256")
        artifacts["daemon_binary"] = {**actual, "path": str(p),
            "manifest_sha256": expected_hash,
            "matches_manifest": bool(expected_hash and actual.get("sha256") == expected_hash)}
    for label, values, manifest_key in (
        ("ea_source_installed", args.ea_source, "ea_source"),
        ("ea_binary_installed", args.ea_binary, "ea_binary"),
    ):
        artifacts[label] = []
        expected_hash = expected.get(manifest_key, {}).get("sha256")
        for value in values:
            p = Path(value).expanduser().resolve()
            actual = sha256(p)
            artifacts[label].append({"path": str(p), **actual,
                "manifest_sha256": expected_hash,
                "matches_manifest": bool(expected_hash and actual.get("sha256") == expected_hash)})

    status = git_value(root, "status", "--porcelain=v1", "--untracked-files=all") or ""
    refs = {}
    for ref in ("HEAD", "main", "origin/main", "release/mt5-bridge-v2-rc1",
                "origin/release/mt5-bridge-v2-rc1"):
        refs[ref] = git_value(root, "rev-parse", "--verify", ref)
    repo = {
        "root": str(root), "branch": git_value(root, "branch", "--show-current"),
        "head": git_value(root, "rev-parse", "HEAD"), "refs": refs,
        "dirty": bool(status), "status_entries": [
            line[3:] for line in status.splitlines() if len(line) >= 4],
        "status_entry_count": len(status.splitlines()),
    }

    launchd = {}
    if platform.system() == "Darwin":
        r = run(["launchctl", "list"])
        jobs = []
        if r.get("exit_code") == 0:
            for line in r.get("stdout", "").splitlines():
                fields = line.split(None, 2)
                if len(fields) != 3 or not fields[2].startswith("com.quantos.mt5-"):
                    continue
                pid = fields[0]
                job = {"pid": pid if pid.isdigit() else None,
                       "last_exit_status": fields[1], "label": fields[2]}
                if pid.isdigit() and shutil.which("lsof"):
                    cwd_r = run(["lsof", "-a", "-p", pid, "-d", "cwd", "-Fn"])
                    exe_r = run(["lsof", "-a", "-p", pid, "-d", "txt", "-Fn"])
                    job["working_directory"] = next(
                        (x[1:] for x in cwd_r.get("stdout", "").splitlines() if x.startswith("n")), None)
                    job["executable_path"] = next(
                        (x[1:] for x in exe_r.get("stdout", "").splitlines() if x.startswith("n")), None)
                jobs.append(job)
        launchd = {"available": r.get("exit_code") == 0, "matching_jobs": jobs}
    listeners = {}
    if shutil.which("lsof"):
        for port in PORTS:
            r = run(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"])
            listeners[str(port)] = r.get("stdout", "").splitlines()[1:] if r.get("exit_code") == 0 else []

    report = {
        "schema_version": 1,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "read_only": True,
        "no_service_or_database_mutations_performed": True,
        "host": {"platform": platform.platform(), "python": platform.python_version(),
                 "hostname": platform.node()},
        "repository": repo,
        "build_manifest": {
            "path": "build-manifest.json" if manifest else None,
            "status": manifest.get("status"),
            "updated_at": manifest.get("updated_at"),
            "source": manifest.get("source"),
            "verification": manifest.get("verification"),
            "manifest_candidate_head_matches_checkout": bool(
                manifest.get("source", {}).get("candidate_head")
                and manifest.get("source", {}).get("candidate_head") == repo.get("head")),
        },
        "artifacts": artifacts,
        "docker": docker_inventory(root),
        "macos_launchd": launchd,
        "listening_ports": listeners,
        "safety_notes": [
            "No environment variables, passwords, private keys, tokens, DATABASE_URL, or container Config.Env values are collected.",
            "Docker container mounts and Compose labels are metadata only; no volume contents are read.",
            "No container, service, terminal, database, certificate, or EA file is changed.",
            "The daemon binary is hashed only; it is never executed.",
            "A missing hash/path is UNKNOWN, not PASS. Compare the report with the protected manifest and operator evidence.",
        ],
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        out = Path(args.output).expanduser()
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(rendered, encoding="utf-8")
        try:
            out.chmod(0o600)
        except OSError:
            pass
        print(f"Inventory written to {out} (mode 0600 where supported).")
    else:
        print(rendered, end="")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())

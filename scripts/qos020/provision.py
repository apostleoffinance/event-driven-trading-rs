#!/usr/bin/env python3
"""Provision one uniquely named disposable QOS-020 PostgreSQL instance."""

from __future__ import annotations

import datetime as dt
import json
import os
import secrets
import socket
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path

from qos020_guard import docker_evidence, validate_identity

COMPOSE = Path(__file__).with_name("docker-compose.qos020.yml").resolve()
IMAGE = "postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777"


def run(command: list[str], *, env: dict[str, str] | None = None) -> str:
    try:
        result = subprocess.run(command, check=True, capture_output=True, text=True, env=env)
    except subprocess.CalledProcessError as exc:
        raise RuntimeError(f"command failed with exit status {exc.returncode}: {Path(command[0]).name}") from None
    return result.stdout.strip()


def ensure_unclaimed(run_id: str, project: str, volume: str, network: str) -> None:
    checks = [
        (["docker", "ps", "-aq", "--filter", f"label=com.docker.compose.project={project}"], "container project"),
        (["docker", "volume", "inspect", volume], "volume"),
        (["docker", "network", "inspect", network], "network"),
    ]
    for command, description in checks:
        result = subprocess.run(command, capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            raise RuntimeError(f"generated QOS-020 {description} identity already exists; refusing reuse")


def allocate_loopback_port() -> int:
    with socket.socket() as candidate:
        candidate.bind(("127.0.0.1", 0))
        port = candidate.getsockname()[1]
    return allocate_loopback_port() if port in {15433, 15543} else port


def main() -> int:
    now = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d-%H%M%S")
    run_id = f"{now}-{secrets.token_hex(6)}"
    project = f"qos020-integration-{run_id}"
    provisioning_id = run_id
    instance_id = str(uuid.uuid4())
    host_port = allocate_loopback_port()
    volume = f"qos020_it_{run_id.replace('-', '_')}"
    network = f"qos020-integration-net-{run_id}"
    container_name = f"{project}-postgres-1"
    runtime_dir = Path(tempfile.gettempdir()) / "qos020" / run_id
    runtime_dir.mkdir(mode=0o700, parents=True, exist_ok=False)
    runtime_dir.chmod(0o700)
    secret_dir = runtime_dir / "secrets"
    secret_dir.mkdir(mode=0o700)
    bootstrap_password = secrets.token_hex(32)
    sqlx_password = secrets.token_hex(32)
    for filename, value in (("bootstrap_password", bootstrap_password), ("sqlx_password", sqlx_password)):
        path = secret_dir / filename
        path.write_text(value + "\n", encoding="utf-8")
        path.chmod(0o600)

    try:
        ensure_unclaimed(run_id, project, volume, network)
        # GitHub-hosted runners are ephemeral; fetch the exact immutable digest
        # before inspecting it rather than assuming it is preloaded.
        run(["docker", "pull", IMAGE])
        image_data = json.loads(run(["docker", "image", "inspect", IMAGE]))[0]
        if IMAGE not in (image_data.get("RepoDigests") or []):
            raise RuntimeError("pinned PostgreSQL digest is not available locally")

        env = os.environ.copy()
        env.update({
            "QOS020_RUN_ID": run_id,
            "QOS020_PROVISIONING_ID": provisioning_id,
            "QOS020_VOLUME_NAME": volume,
            "QOS020_NETWORK_NAME": network,
            "QOS020_SECRET_DIR": str(secret_dir),
            "QOS020_HOST_PORT": str(host_port),
        })
        compose = ["docker", "compose", "--project-directory", str(COMPOSE.parent), "--project-name", project, "--file", str(COMPOSE)]
        run(compose + ["config", "--quiet"], env=env)
        run(compose + ["up", "--detach", "--wait"], env=env)

        container_id = run(compose + ["ps", "--quiet", "postgres"], env=env)
        if not container_id:
            raise RuntimeError("QOS-020 PostgreSQL container was not created")
        container = json.loads(run(["docker", "inspect", container_id]))[0]
        bindings = container["HostConfig"]["PortBindings"]["5432/tcp"]
        if not bindings or bindings[0]["HostIp"] != "127.0.0.1":
            raise RuntimeError("QOS-020 database host binding is absent or not loopback-only")
        port = int(bindings[0]["HostPort"])
        if port != host_port:
            raise RuntimeError("Docker published an unexpected QOS-020 host port")

        init_sql = (Path(__file__).parent / "init" / "10-qos020-init.sql").read_text(encoding="utf-8")
        init_stdin = (
            f"\\set qos020_sqlx_password {sqlx_password}\n"
            f"\\set qos020_run_id {run_id}\n"
            f"\\set qos020_provisioning_id {provisioning_id}\n"
            f"\\set qos020_instance_id {instance_id}\n"
            "\\set qos020_expected_pg_major 16\n"
            + init_sql
        )
        subprocess.run(
            ["docker", "exec", "--interactive", container_id, "psql", "--no-psqlrc",
             "--set", "ON_ERROR_STOP=1", "--username", "qos020_bootstrap",
             "--dbname", "qos020_test_master"],
            input=init_stdin, check=True, text=True, capture_output=True,
        )
        manifest = {
            "lane": "integration-test",
            "environment_type": "integration-test",
            "run_id": run_id,
            "provisioning_id": provisioning_id,
            "instance_creation_identity": instance_id,
            "project": project,
            "container_id": container_id,
            "container_name": container["Name"].lstrip("/"),
            "volume": volume,
            "network": network,
            "compose_working_dir": str(COMPOSE.parent),
            "compose_file": str(COMPOSE),
            "host": "127.0.0.1",
            "port": port,
            "database": "qos020_test_master",
            "postgres_major": 16,
            "image_reference": IMAGE,
            "image_id": image_data["Id"],
        }
        manifest_path = runtime_dir / "manifest.json"
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        manifest_path.chmod(0o600)
        url = f"postgresql://qos020_sqlx:{sqlx_password}@127.0.0.1:{port}/qos020_test_master"
        evidence = docker_evidence(manifest, url, secret_dir / "sqlx_password")
        validate_identity(manifest, url, evidence)

        github_output = os.environ.get("GITHUB_OUTPUT")
        if github_output:
            with open(github_output, "a", encoding="utf-8") as output:
                output.write(f"manifest={manifest_path}\n")
                output.write(f"secrets_dir={secret_dir}\n")
                output.write(f"run_id={run_id}\n")

        print("QOS-020 PostgreSQL instance provisioned and identity-verified.")
        print(f"run_id={run_id}")
        print(f"project={project}")
        print(f"container_id={container_id}")
        print(f"container_name={manifest['container_name']}")
        print(f"network={network}")
        print(f"volume={volume}")
        print(f"endpoint=127.0.0.1:{port} database=qos020_test_master role=qos020_sqlx")
        print(f"manifest={manifest_path}")
        print(f"secrets_dir={secret_dir} (credentials not displayed)")
        return 0
    except Exception as exc:
        print(f"QOS-020 provisioning failed: {exc.__class__.__name__}: {exc}", file=sys.stderr)
        print(f"resources for review: run_id={run_id} project={project} volume={volume} network={network} runtime_dir={runtime_dir}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

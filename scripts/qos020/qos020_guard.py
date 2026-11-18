#!/usr/bin/env python3
"""Fail-closed identity guard for QOS-020 PostgreSQL test commands."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

PG_MAJOR = 16
PROTECTED_PORTS = {15433, 15543}
PROTECTED_DATABASES = {"mt5_v2_staging", "trading"}
PROJECT_PREFIX = "qos020-integration-"


class GuardError(RuntimeError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def validate_manifest_endpoint(
    manifest: dict[str, object], url: str, lane: str | None = None, run_id_arg: str | None = None
) -> dict[str, object]:
    _require(manifest.get("lane") == "integration-test", "explicit integration-test lane required")
    _require(lane in {None, "integration-test"}, "explicit integration-test lane required")
    run_id = manifest.get("run_id")
    _require(isinstance(run_id, str) and re.fullmatch(r"[a-z0-9-]{12,80}", run_id) is not None,
             "explicit valid test-run ID required")
    _require(run_id_arg in {None, run_id}, "explicit test-run ID mismatch")
    project = manifest.get("project")
    _require(isinstance(project, str) and project == PROJECT_PREFIX + run_id,
             "unexpected Compose project identity")
    _require(manifest.get("volume") == "qos020_it_" + run_id.replace("-", "_"),
             "unexpected generated volume identity")
    _require(manifest.get("network") == "qos020-integration-net-" + run_id,
             "unexpected generated network identity")
    _require(manifest.get("environment_type") == "integration-test", "unexpected manifest environment")
    _require(manifest.get("postgres_major") == PG_MAJOR, "unexpected manifest PostgreSQL major version")
    _require(manifest.get("image_reference") == "postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777",
             "unexpected PostgreSQL image digest")
    _require(manifest.get("provisioning_id") == run_id, "provisioning identity does not match run ID")
    _require(bool(manifest.get("instance_creation_identity")), "manifest instance creation identity missing")
    endpoint = parse_url(url)
    _require(endpoint["host"] == "127.0.0.1", "database host must be exactly 127.0.0.1")
    _require(endpoint["port"] not in PROTECTED_PORTS, "protected or legacy database port rejected")
    _require(endpoint["port"] >= 1024, "database host port is not a permitted published port")
    _require(endpoint["database"] not in PROTECTED_DATABASES, "protected database name rejected")
    _require(endpoint["username"] == "qos020_sqlx", "database must use the dedicated SQLx role")
    _require(endpoint["database"] == "qos020_test_master", "unexpected SQLx master database")
    _require(endpoint["host"] == manifest.get("host") and endpoint["port"] == manifest.get("port"),
             "database endpoint does not match registered provisioner endpoint")
    return endpoint


def parse_url(url: str) -> dict[str, object]:
    try:
        parsed = urlsplit(url)
        _require(parsed.scheme in {"postgres", "postgresql"}, "unsupported database URL scheme")
        _require(parsed.hostname is not None and parsed.port is not None, "database URL host and port are required")
        _require(parsed.username is not None and parsed.password is not None, "database URL credentials are required")
        _require(parsed.path not in {"", "/"}, "database name is required")
        _require(not parsed.query and not parsed.fragment, "database URL query and fragment are not allowed")
        return {
            "host": parsed.hostname,
            "port": parsed.port,
            "username": unquote(parsed.username),
            "password": unquote(parsed.password),
            "database": unquote(parsed.path[1:]),
        }
    except ValueError as exc:
        raise GuardError("malformed database URL") from exc


def validate_identity(
    manifest: dict[str, object], url: str, evidence: dict[str, object]
) -> dict[str, object]:
    run_id = manifest.get("run_id")
    endpoint = validate_manifest_endpoint(manifest, url)
    project = manifest["project"]

    _require(evidence.get("container_id") == manifest.get("container_id"), "container identity mismatch")
    _require(evidence.get("container_running") is True, "QOS-020 container is not running")
    _require(evidence.get("container_project") == project, "container Compose project mismatch")
    _require(evidence.get("container_run_id") == run_id, "container run ID mismatch")
    _require(evidence.get("container_provisioning_id") == run_id, "container provisioning identity mismatch")
    _require(evidence.get("container_environment") == "integration-test", "container environment mismatch")
    _require(evidence.get("container_service") == "postgres", "unexpected Compose service")
    _require(evidence.get("container_name") == manifest.get("container_name"), "container name mismatch")
    _require(evidence.get("container_working_dir") == manifest.get("compose_working_dir"),
             "Compose working directory mismatch")
    _require(evidence.get("container_config_file") == manifest.get("compose_file"),
             "Compose config file mismatch")
    _require(evidence.get("container_mount_volume") == manifest.get("volume"), "mounted volume mismatch")
    _require(evidence.get("container_network") == manifest.get("network"), "container network mismatch")
    _require(evidence.get("volume_name") == manifest.get("volume"), "volume identity mismatch")
    _require(evidence.get("volume_project") == project and evidence.get("volume_run_id") == run_id,
             "volume labels mismatch")
    _require(evidence.get("volume_environment") == "integration-test", "volume environment label mismatch")
    _require(evidence.get("volume_provisioning_id") == run_id, "volume provisioning identity mismatch")
    _require(evidence.get("network_name") == manifest.get("network"), "network identity mismatch")
    _require(evidence.get("network_project") == project and evidence.get("network_run_id") == run_id,
             "network labels mismatch")
    _require(evidence.get("network_environment") == "integration-test", "network environment label mismatch")
    _require(evidence.get("network_provisioning_id") == run_id, "network provisioning identity mismatch")
    _require(evidence.get("network_container_ids") == [manifest.get("container_id")],
             "QOS-020 network has unexpected attached containers")
    _require(evidence.get("postgres_major") == PG_MAJOR, "actual PostgreSQL major version mismatch")
    _require(evidence.get("database_name") == endpoint["database"], "actual database identity mismatch")
    _require(evidence.get("marker_environment") == "integration-test", "database environment marker missing or wrong")
    _require(evidence.get("marker_run_id") == run_id, "database marker run ID mismatch")
    _require(evidence.get("marker_provisioning_id") == run_id, "database marker provisioning ID mismatch")
    _require(evidence.get("marker_expected_major") == PG_MAJOR, "database marker version mismatch")
    _require(bool(evidence.get("marker_instance_creation_identity")), "database instance creation identity missing")
    _require(evidence.get("marker_instance_creation_identity") == manifest.get("instance_creation_identity"),
             "database instance creation identity mismatch")
    _require(evidence.get("sqlx_role_login") is True and evidence.get("sqlx_role_createdb") is True,
             "SQLx role lacks required LOGIN/CREATEDB attributes")
    for privilege in ("superuser", "createrole", "replication", "bypassrls"):
        _require(evidence.get(f"sqlx_role_{privilege}") is False, f"SQLx role must not have {privilege.upper()}")
    _require(evidence.get("database_url_password_present") is True, "database credentials are missing")
    _require(evidence.get("database_url_password_matches_secret") is True,
             "database URL credentials do not match the provisioned test credentials")
    return endpoint


def docker_evidence(manifest: dict[str, object], url: str, password_file: Path) -> dict[str, object]:
    endpoint = validate_manifest_endpoint(manifest, url)
    password = password_file.read_text(encoding="utf-8").strip()
    _require(bool(password), "database credentials are missing")
    _require(endpoint["password"] == password, "database URL credentials do not match the provisioned test credentials")

    def inspect(kind: str, name: str) -> dict[str, object]:
        command = ["docker", "inspect", name] if kind == "container" else ["docker", kind, "inspect", name]
        result = subprocess.run(command, check=True, capture_output=True, text=True)
        data = json.loads(result.stdout)
        return data[0]

    container = inspect("container", str(manifest["container_id"]))
    labels = container["Config"]["Labels"] or {}
    mounts = container.get("Mounts", [])
    volume_mounts = [item for item in mounts if item.get("Destination") == "/var/lib/postgresql/data"]
    networks = list(container["NetworkSettings"]["Networks"].keys())
    _require(container["Id"] == manifest["container_id"], "container ID changed")
    _require(container["Config"]["Image"] == manifest["image_reference"], "container image reference changed")
    _require(container["Image"] == manifest["image_id"], "container image identity changed")
    _require(container["State"]["Running"] is True, "QOS-020 container is not running")
    _require(labels.get("com.docker.compose.project") == manifest["project"], "container Compose project mismatch")
    _require(labels.get("com.docker.compose.service") == "postgres", "unexpected Compose service")
    _require(labels.get("qos020.environment") == "integration-test", "container environment label mismatch")
    _require(labels.get("qos020.run_id") == manifest["run_id"] and labels.get("qos020.provisioning_id") == manifest["run_id"],
             "container run or provisioning identity mismatch")
    _require(labels.get("com.docker.compose.project.working_dir") == manifest["compose_working_dir"],
             "Compose working directory mismatch")
    _require(labels.get("com.docker.compose.project.config_files") == manifest["compose_file"],
             "Compose config file mismatch")
    _require(len(volume_mounts) == 1, "expected exactly one PostgreSQL data volume")
    _require(len(networks) == 1, "expected exactly one dedicated Docker network")
    container_ports = container["HostConfig"]["PortBindings"]["5432/tcp"]
    _require(bool(container_ports), "container does not publish PostgreSQL port")
    actual_port = int(container_ports[0]["HostPort"])
    _require(container_ports[0]["HostIp"] == "127.0.0.1", "host port is not loopback-only")
    _require(actual_port == endpoint["port"], "published host port differs from database endpoint")

    volume = inspect("volume", str(manifest["volume"]))
    network = inspect("network", str(manifest["network"]))
    _require(volume["Name"] == manifest["volume"], "volume does not exist")
    _require(network["Name"] == manifest["network"], "network does not exist")
    attached_container_ids = sorted((network.get("Containers") or {}).keys())
    _require(attached_container_ids == [manifest["container_id"]],
             "QOS-020 network has unexpected attached containers")
    volume_labels = volume.get("Labels") or {}
    network_labels = network.get("Labels") or {}
    for labels_to_check, resource in ((volume_labels, "volume"), (network_labels, "network")):
        _require(labels_to_check.get("com.docker.compose.project") == manifest["project"], f"{resource} Compose project mismatch")
        _require(labels_to_check.get("qos020.environment") == "integration-test", f"{resource} environment mismatch")
        _require(labels_to_check.get("qos020.run_id") == manifest["run_id"] and labels_to_check.get("qos020.provisioning_id") == manifest["run_id"],
                 f"{resource} run or provisioning identity mismatch")
    _require(volume_mounts[0]["Name"] == manifest["volume"], "container-mounted volume identity mismatch")
    _require(networks == [manifest["network"]], "container network identity mismatch")

    marker_query = "SELECT environment_type,run_id,provisioning_id,expected_postgres_major,instance_creation_identity FROM qos020_control.environment_identity WHERE singleton"
    sqlx_query = "SELECT rolcanlogin,rolcreatedb,rolsuper,rolcreaterole,rolreplication,rolbypassrls FROM pg_roles WHERE rolname=current_user"
    databases_query = "SELECT datname FROM pg_database WHERE left(datname,11)='_sqlx_test_' ORDER BY datname"
    base = ["psql", "--no-psqlrc", "--no-align", "--tuples-only", "--field-separator=|", "--host", str(endpoint["host"]), "--port", str(endpoint["port"]), "--username", str(endpoint["username"]), "--dbname", str(endpoint["database"]), "--command"]
    env = os.environ.copy()
    env["PGPASSWORD"] = password
    env["PGCONNECT_TIMEOUT"] = "5"

    def query(sql: str) -> list[str]:
        result = subprocess.run(base + [sql], check=True, capture_output=True, text=True, env=env)
        return result.stdout.strip().splitlines()

    version_rows = query("SHOW server_version_num")
    _require(len(version_rows) == 1, "could not determine PostgreSQL version")
    major = int(version_rows[0]) // 10000
    marker_rows = query(marker_query)
    _require(len(marker_rows) == 1, "database environment marker is missing or ambiguous")
    marker = marker_rows[0].split("|")
    _require(len(marker) == 5, "database environment marker is malformed")
    role_rows = query(sqlx_query)
    _require(len(role_rows) == 1, "could not verify SQLx role privileges")
    role = role_rows[0].split("|")
    sqlx_databases = [name for name in query(databases_query) if name]
    _require(len(role) == 6, "SQLx role privilege evidence is malformed")
    return {
        "container_id": container["Id"],
        "container_running": container["State"]["Running"],
        "container_project": labels.get("com.docker.compose.project"),
        "container_service": labels.get("com.docker.compose.service"),
        "container_working_dir": labels.get("com.docker.compose.project.working_dir"),
        "container_config_file": labels.get("com.docker.compose.project.config_files"),
        "container_name": container["Name"].lstrip("/"),
        "container_run_id": labels.get("qos020.run_id"),
        "container_provisioning_id": labels.get("qos020.provisioning_id"),
        "container_environment": labels.get("qos020.environment"),
        "container_mount_volume": volume_mounts[0]["Name"],
        "container_network": networks[0],
        "volume_name": volume["Name"],
        "volume_project": volume_labels.get("com.docker.compose.project"),
        "volume_run_id": volume_labels.get("qos020.run_id"),
        "volume_environment": volume_labels.get("qos020.environment"),
        "volume_provisioning_id": volume_labels.get("qos020.provisioning_id"),
        "network_name": network["Name"],
        "network_project": network_labels.get("com.docker.compose.project"),
        "network_run_id": network_labels.get("qos020.run_id"),
        "network_environment": network_labels.get("qos020.environment"),
        "network_provisioning_id": network_labels.get("qos020.provisioning_id"),
        "network_container_ids": attached_container_ids,
        "postgres_major": major,
        "database_name": endpoint["database"],
        "marker_environment": marker[0],
        "marker_run_id": marker[1],
        "marker_provisioning_id": marker[2],
        "marker_expected_major": int(marker[3]),
        "marker_instance_creation_identity": marker[4],
        "sqlx_role_login": role[0] == "t",
        "sqlx_role_createdb": role[1] == "t",
        "sqlx_role_superuser": role[2] == "t",
        "sqlx_role_createrole": role[3] == "t",
        "sqlx_role_replication": role[4] == "t",
        "sqlx_role_bypassrls": role[5] == "t",
        "database_url_password_present": bool(endpoint["password"]),
        "database_url_password_matches_secret": endpoint["password"] == password,
        "leftover_sqlx_databases": sqlx_databases,
    }


def read_manifest(path: Path) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        _require(isinstance(value, dict), "provisioner manifest must be an object")
        return value
    except (OSError, json.JSONDecodeError) as exc:
        raise GuardError("QOS-020 provisioner manifest is missing or invalid") from exc


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--secrets-dir", type=Path, required=True)
    parser.add_argument("--lane", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--verify-sqlx-cleanup", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        manifest = read_manifest(args.manifest)
        _require(args.lane == "integration-test", "explicit integration-test lane required")
        _require(args.run_id == manifest.get("run_id"), "explicit test-run ID mismatch")
        sqlx_password = (args.secrets_dir / "sqlx_password").read_text(encoding="utf-8").strip()
        _require(bool(sqlx_password), "database credentials are missing")
        url = f"postgresql://qos020_sqlx:{sqlx_password}@{manifest['host']}:{manifest['port']}/{manifest['database']}"
        validate_manifest_endpoint(manifest, url, args.lane, args.run_id)
        evidence = docker_evidence(manifest, url, args.secrets_dir / "sqlx_password")
        endpoint = validate_identity(manifest, url, evidence)
        print(f"QOS-020 identity verified: project={manifest['project']} run_id={manifest['run_id']} host={endpoint['host']} port={endpoint['port']} database={endpoint['database']} role=qos020_sqlx")
        if args.verify_sqlx_cleanup:
            if args.command:
                raise GuardError("cleanup verification cannot run a child command")
            leftovers = evidence["leftover_sqlx_databases"]
            if leftovers:
                print("SQLx databases remain in disposable instance: " + ", ".join(leftovers), file=sys.stderr)
                return 1
            print("SQLx generated test databases: none remain")
            return 0
        if args.command:
            command = args.command[1:] if args.command[0] == "--" else args.command
            if not command:
                raise GuardError("command after guard is empty")
            child_env = os.environ.copy()
            child_env["DATABASE_URL"] = url
            child_env["QOS020_LANE"] = str(manifest["lane"])
            child_env["QOS020_RUN_ID"] = str(manifest["run_id"])
            return subprocess.run(command, env=child_env, check=False).returncode
        return 0
    except (GuardError, OSError, KeyError, ValueError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        print(f"QOS-020 guard rejected command: {exc.__class__.__name__}: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

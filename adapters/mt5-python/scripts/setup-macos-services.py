#!/usr/bin/env python3
"""Prepare protected local macOS MT5 read-only service configuration.

This script creates quantos.env and the two LaunchAgent plists. It does not
load/start services and never prints credentials or the generated provider token.
Local-development only; venue execution remains disabled.
"""
from __future__ import annotations

import getpass
import os
import plistlib
import re
import secrets
import shlex
import shutil
import socket
import subprocess
import sys
from pathlib import Path
from typing import Mapping

PROVIDER = "mt5_adapter.macos_socket_provider:create_terminal_provider"
SERVICE_ROOT = Path.home() / "Library" / "Application Support" / "QuantOS"
CERT_ROOT = SERVICE_ROOT / "certs"
LAUNCH_AGENTS = Path.home() / "Library" / "LaunchAgents"
BRIDGE_LABEL = "com.quantos.mt5-bridge"
WORKER_LABEL = "com.quantos.mt5-worker"


def ask(label: str, *, default: str = "", secret: bool = False) -> str:
    prompt = f"{label}"
    if default:
        prompt += f" [{default}]"
    prompt += ": "
    while True:
        value = getpass.getpass(prompt) if secret else input(prompt)
        value = value.strip()
        if not value:
            value = default
        if value:
            return value
        print("A value is required.")


def shell_quote(value: str) -> str:
    """Return one safe shell-format assignment value."""
    return shlex.quote(value)


def build_plist(label: str, wrapper: Path, env_file: Path, repo: Path,
                executable_args: list[str], working_directory: Path,
                stdout_path: Path, stderr_path: Path) -> bytes:
    return plistlib.dumps({
        "Label": label,
        "ProgramArguments": ["/bin/bash", str(wrapper), *executable_args],
        "WorkingDirectory": str(working_directory),
        "EnvironmentVariables": {"QUANTOS_ENV_FILE": str(env_file)},
        "RunAtLoad": True,
        "KeepAlive": True,
        "ThrottleInterval": 10,
        "StandardOutPath": str(stdout_path),
        "StandardErrorPath": str(stderr_path),
    }, fmt=plistlib.FMT_XML, sort_keys=False)


def ensure_private_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    path.chmod(0o700)


def certificate_fingerprint(client_cert: Path) -> str:
    result = subprocess.run(
        ["openssl", "x509", "-in", str(client_cert), "-outform", "DER"],
        check=True, capture_output=True,
    )
    digest = subprocess.run(
        ["shasum", "-a", "256"], input=result.stdout,
        check=True, capture_output=True, text=True,
    ).stdout.split()[0].lower()
    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise RuntimeError("could not calculate the client certificate fingerprint")
    return digest


def main() -> int:
    if sys.platform != "darwin":
        print("BLOCKED: this setup helper is for macOS only.", file=sys.stderr)
        return 2

    repo = Path(__file__).resolve().parents[3]
    adapter = repo / "adapters" / "mt5-python"
    wrapper = adapter / "scripts" / "mt5-macos-launch-wrapper.sh"
    daemon = repo / "target" / "release" / "mt5-bridge-daemon"
    python_exe = Path(sys.executable).resolve()
    env_file = SERVICE_ROOT / "quantos.env"
    bridge_plist = LAUNCH_AGENTS / f"{BRIDGE_LABEL}.plist"
    worker_plist = LAUNCH_AGENTS / f"{WORKER_LABEL}.plist"
    token_file = CERT_ROOT / "provider-token.txt"

    required_files = [
        wrapper, daemon,
        CERT_ROOT / "dev-ca.pem", CERT_ROOT / "dev-ca.crl.pem",
        CERT_ROOT / "server.pem", CERT_ROOT / "server.key.pem",
        CERT_ROOT / "client.pem", CERT_ROOT / "client.key.pem",
    ]
    missing = [str(path) for path in required_files if not path.is_file()]
    if missing:
        print("BLOCKED: required build/certificate files are missing:", file=sys.stderr)
        for path in missing:
            print(f"  - {path}", file=sys.stderr)
        return 2
    if not os.access(wrapper, os.X_OK):
        print("BLOCKED: launch wrapper is not executable.", file=sys.stderr)
        return 2

    conflicts = [path for path in (env_file, bridge_plist, worker_plist, token_file) if path.exists()]
    if conflicts:
        print("BLOCKED: refusing to overwrite existing configuration or token files:", file=sys.stderr)
        for path in conflicts:
            print(f"  - {path}", file=sys.stderr)
        print("Review/back up existing files manually before retrying.", file=sys.stderr)
        return 2

    try:
        commit = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD"],
            check=True, capture_output=True, text=True,
        ).stdout.strip()
        if not re.fullmatch(r"(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})", commit):
            raise RuntimeError("repository HEAD is not a full commit SHA")
        fingerprint = certificate_fingerprint(CERT_ROOT / "client.pem")
    except (OSError, subprocess.SubprocessError, RuntimeError, IndexError) as exc:
        print(f"BLOCKED: unable to inspect local build/certificates ({type(exc).__name__}).", file=sys.stderr)
        return 2

    print("Quant OS local macOS service setup (read-only; development only)")
    print(f"Repository commit: {commit}")
    print("Enter the exact identity for your logged-in broker DEMO account.")
    terminal_path = ask("MetaTrader 5 application path/label")
    login = ask("MT5 demo login number")
    if not login.isdigit() or int(login) <= 0:
        print("BLOCKED: MT5 login must be a positive numeric login.", file=sys.stderr)
        return 2
    server = ask("Exact MT5 demo server name")
    account_id = ask("Rust account ID", default=login)
    default_terminal_id = "macos-demo-" + re.sub(r"[^A-Za-z0-9_-]", "-", socket.gethostname())[:48]
    terminal_id = ask("Stable terminal ID", default=default_terminal_id)
    symbols = ask("Exact comma-separated broker symbols", default="EURUSD")
    symbol_list = [part.strip() for part in symbols.split(",")]
    if not symbol_list or any(not item for item in symbol_list) or len(set(symbol_list)) != len(symbol_list):
        print("BLOCKED: symbols must be a non-empty comma-separated unique list.", file=sys.stderr)
        return 2
    currency = ask("Expected demo account currency (for example USD)")
    database_url = os.environ.get("DATABASE_URL", "").strip()
    if not database_url:
        database_url = ask("PostgreSQL DATABASE_URL (input hidden)", secret=True)
    if not database_url.startswith(("postgres://", "postgresql://")):
        print("BLOCKED: DATABASE_URL must use postgres:// or postgresql://.", file=sys.stderr)
        return 2

    token = secrets.token_urlsafe(48)
    values: dict[str, str] = {
        "QUANTOS_REPOSITORY_COMMIT": commit,
        "MT5_TERMINAL_PROVIDER": PROVIDER,
        "MT5_TERMINAL_PATH": terminal_path,
        "MT5_LOGIN": login,
        "MT5_SERVER": server,
        "MT5_ACCOUNT_ID": account_id,
        "MT5_TERMINAL_ID": terminal_id,
        "MT5_SYMBOLS": ",".join(symbol_list),
        "MT5_EXPECTED_ACCOUNT_CURRENCY": currency,
        "MT5_MACOS_PROVIDER_TOKEN": token,
        "MT5_MACOS_PROVIDER_HOST": "127.0.0.1",
        "MT5_MACOS_PROVIDER_PORT": "8765",
        "MT5_MACOS_PROVIDER_CONNECT_TIMEOUT_SECONDS": "60",
        "MT5_BRIDGE_HOST": "127.0.0.1",
        "MT5_BRIDGE_PORT": "9443",
        "MT5_BRIDGE_SERVER_NAME": "localhost",
        "MT5_BRIDGE_CA_FILE": str(CERT_ROOT / "dev-ca.pem"),
        "MT5_BRIDGE_CLIENT_CERT": str(CERT_ROOT / "client.pem"),
        "MT5_BRIDGE_CLIENT_KEY": str(CERT_ROOT / "client.key.pem"),
        "MT5_SEQUENCE_FILE": str(SERVICE_ROOT / "state" / "mt5-sequence.json"),
        "MT5_POLL_INTERVAL_SECONDS": "10",
        "DATABASE_URL": database_url,
        "MT5_BRIDGE_BIND": "127.0.0.1:9443",
        "MT5_BRIDGE_SERVER_CERT": str(CERT_ROOT / "server.pem"),
        "MT5_BRIDGE_SERVER_KEY": str(CERT_ROOT / "server.key.pem"),
        "MT5_BRIDGE_CLIENT_CA": str(CERT_ROOT / "dev-ca.pem"),
        "MT5_BRIDGE_PRINCIPAL_SHA256": fingerprint,
        "MT5_BRIDGE_ALLOW_REMOTE_BIND": "false",
        "PYTHONPATH": str(adapter),
    }

    try:
        ensure_private_directory(SERVICE_ROOT)
        ensure_private_directory(CERT_ROOT)
        ensure_private_directory(SERVICE_ROOT / "logs")
        ensure_private_directory(SERVICE_ROOT / "state")
        LAUNCH_AGENTS.mkdir(parents=True, exist_ok=True)

        env_contents = "\n".join(f"{key}={shell_quote(value)}" for key, value in values.items()) + "\n"
        env_file.write_text(env_contents, encoding="utf-8")
        env_file.chmod(0o600)
        token_file.write_text(token + "\n", encoding="utf-8")
        token_file.chmod(0o600)

        bridge_bytes = build_plist(
            BRIDGE_LABEL, wrapper, env_file, repo, [str(daemon)], repo,
            SERVICE_ROOT / "logs" / "mt5-bridge.stdout.log",
            SERVICE_ROOT / "logs" / "mt5-bridge.stderr.log",
        )
        worker_bytes = build_plist(
            WORKER_LABEL, wrapper, env_file, repo,
            [str(python_exe), "-m", "mt5_adapter.worker"], adapter,
            SERVICE_ROOT / "logs" / "mt5-worker.stdout.log",
            SERVICE_ROOT / "logs" / "mt5-worker.stderr.log",
        )
        bridge_plist.write_bytes(bridge_bytes)
        worker_plist.write_bytes(worker_bytes)
        bridge_plist.chmod(0o600)
        worker_plist.chmod(0o600)

        for plist_path in (bridge_plist, worker_plist):
            subprocess.run(["plutil", "-lint", str(plist_path)], check=True, capture_output=True, text=True)
    except Exception as exc:
        print(f"BLOCKED: setup failed while writing protected files ({type(exc).__name__}).", file=sys.stderr)
        print("Inspect the QuantOS directory before retrying; this helper does not overwrite files.", file=sys.stderr)
        return 1

    try:
        subprocess.run(["pbcopy"], input=token, check=True, text=True, capture_output=True)
        clipboard_ready = True
    except (OSError, subprocess.SubprocessError):
        clipboard_ready = False

    print("\nCONFIGURATION PREPARED (services were NOT started).")
    print(f"Environment file: {env_file} (0600)")
    print(f"Bridge LaunchAgent: {bridge_plist}")
    print(f"Worker LaunchAgent: {worker_plist}")
    print(f"Private EA token backup: {token_file} (0600; do not share)")
    print("Certificate fingerprint configured locally; it was not printed.")
    if clipboard_ready:
        print("A NEW provider token was copied to your clipboard. Paste it into the EA's InpSharedToken input in MetaTrader, then save/apply the EA inputs.")
    else:
        print(f"Clipboard copy was unavailable. Open the private token file locally: {token_file}")
    print("Do not load either LaunchAgent until the EA has the new token and the account identity/settings have been checked.")
    print("Venue execution remains disabled; this does not establish runtime TLS/revocation or release approval.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

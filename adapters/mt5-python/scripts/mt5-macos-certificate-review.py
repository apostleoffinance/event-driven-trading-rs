#!/usr/bin/env python3
"""macOS certificate/key preflight for Quant OS MT5 bridge.

This checks local files, certificate/key pairing, pin equality, validity, CA
chain, optional CRL verification, and filesystem permissions. It does NOT prove
that the running process enforces revocation or that a real MT5 terminal works.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


class ReviewError(RuntimeError):
    pass


def run_openssl(*args: str) -> str:
    result = subprocess.run(
        ["openssl", *args],
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, "LC_ALL": "C"},
    )
    if result.returncode != 0:
        raise ReviewError("OpenSSL check failed: " + " ".join(args[:2]))
    return result.stdout.strip()


def sha256_certificate(cert_path: Path) -> str:
    # DER is binary, so use capture_output as bytes rather than a text decoder.
    result = subprocess.run(
        ["openssl", "x509", "-in", str(cert_path), "-outform", "DER"],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise ReviewError("Unable to decode certificate DER.")
    return hashlib.sha256(result.stdout).hexdigest()


def public_key_from_cert(cert_path: Path) -> bytes:
    result = subprocess.run(
        ["openssl", "x509", "-in", str(cert_path), "-pubkey", "-noout"],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise ReviewError("Unable to read certificate public key.")
    return result.stdout


def public_key_from_private_key(key_path: Path) -> bytes:
    result = subprocess.run(
        ["openssl", "pkey", "-in", str(key_path), "-pubout"],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise ReviewError("Unable to load private key or derive its public key.")
    return result.stdout


def check_private_key_permissions(path: Path) -> dict[str, Any]:
    info = path.stat()
    mode = stat.S_IMODE(info.st_mode)
    parent_info = path.parent.stat()
    parent_mode = stat.S_IMODE(parent_info.st_mode)
    problems: list[str] = []
    if info.st_uid != os.getuid():
        problems.append("private key is not owned by the current user")
    if mode & 0o077:
        problems.append("private key grants group/other permissions; expected owner-only access")
    if parent_mode & 0o077:
        problems.append("private-key parent directory grants group/other permissions; expected owner-only access")
    return {
        "owner_is_current_user": info.st_uid == os.getuid(),
        "mode_octal": oct(mode),
        "parent_mode_octal": oct(parent_mode),
        "permissions_pass": not problems,
        "problems": problems,
    }


def inspect_certificate(path: Path, ca_file: Path, crl_file: Path | None) -> dict[str, Any]:
    run_openssl("x509", "-in", str(path), "-noout", "-checkend", "0")
    subject = run_openssl("x509", "-in", str(path), "-noout", "-subject")
    issuer = run_openssl("x509", "-in", str(path), "-noout", "-issuer")
    end_date = run_openssl("x509", "-in", str(path), "-noout", "-enddate")
    fingerprint = sha256_certificate(path)
    run_openssl("verify", "-CAfile", str(ca_file), str(path))
    revocation = "NOT_VERIFIED"
    if crl_file is not None:
        run_openssl("verify", "-crl_check", "-CRLfile", str(crl_file), "-CAfile", str(ca_file), str(path))
        revocation = "CRL_CHECK_PASS"
    return {
        "subject": subject,
        "issuer": issuer,
        "not_after": end_date.removeprefix("notAfter="),
        "sha256_der": fingerprint,
        "ca_chain": "PASS",
        "revocation_check": revocation,
    }


def review(args: argparse.Namespace) -> dict[str, Any]:
    client_cert = Path(args.client_cert).expanduser().resolve(strict=True)
    client_key = Path(args.client_key).expanduser().resolve(strict=True)
    server_cert = Path(args.server_cert).expanduser().resolve(strict=True)
    server_key = Path(args.server_key).expanduser().resolve(strict=True)
    ca_file = Path(args.ca_file).expanduser().resolve(strict=True)
    crl_file = Path(args.crl_file).expanduser().resolve(strict=True) if args.crl_file else None
    for path in (client_cert, client_key, server_cert, server_key, ca_file):
        if not path.is_file():
            raise ReviewError("A required certificate/key/CA path is not a regular file.")
    if crl_file is not None and not crl_file.is_file():
        raise ReviewError("CRL path is not a regular file.")

    expected = args.expected_client_sha256.lower().replace(":", "")
    if len(expected) != 64 or any(char not in "0123456789abcdef" for char in expected):
        raise ReviewError("Expected client SHA-256 fingerprint must be 64 hex characters.")

    client = inspect_certificate(client_cert, ca_file, crl_file)
    server = inspect_certificate(server_cert, ca_file, crl_file)
    client_pair = public_key_from_cert(client_cert) == public_key_from_private_key(client_key)
    server_pair = public_key_from_cert(server_cert) == public_key_from_private_key(server_key)
    pin_matches = client["sha256_der"] == expected
    client_acl = check_private_key_permissions(client_key)
    server_acl = check_private_key_permissions(server_key)

    checks = {
        "client_fingerprint_pin_matches": pin_matches,
        "client_certificate_key_pair_matches": client_pair,
        "server_certificate_key_pair_matches": server_pair,
        "client_private_key_permissions": client_acl["permissions_pass"],
        "server_private_key_permissions": server_acl["permissions_pass"],
        "client_ca_chain": client["ca_chain"] == "PASS",
        "server_ca_chain": server["ca_chain"] == "PASS",
        "revocation_checked_for_both": crl_file is not None
        and client["revocation_check"] == "CRL_CHECK_PASS"
        and server["revocation_check"] == "CRL_CHECK_PASS",
    }
    # This is intentionally not a complete release PASS: runtime TLS revocation
    # enforcement and rotation/rollback require separate integration evidence.
    return {
        "schema_version": 1,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "platform": "macOS",
        "client_certificate": client,
        "server_certificate": server,
        "client_key_permissions": client_acl,
        "server_key_permissions": server_acl,
        "checks": checks,
        "runtime_revocation_enforcement": "NOT_VERIFIED",
        "rotation_rollback": "NOT_RUN",
        "real_terminal_smoke": "NOT_RUN",
        "release_gate": "BLOCKED",
        "note": "Local certificate inspection is not proof of runtime TLS enforcement or MT5 demo conformance.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--client-cert", required=True)
    parser.add_argument("--client-key", required=True)
    parser.add_argument("--server-cert", required=True)
    parser.add_argument("--server-key", required=True)
    parser.add_argument("--ca-file", required=True)
    parser.add_argument("--expected-client-sha256", required=True)
    parser.add_argument("--crl-file", help="Required to attempt local CRL revocation checks; runtime enforcement remains separate.")
    parser.add_argument("--evidence-out", help="Write redacted JSON evidence to this path.")
    args = parser.parse_args()
    try:
        report = review(args)
    except (OSError, ReviewError, subprocess.SubprocessError) as exc:
        print(f"BLOCKED: certificate review failed ({type(exc).__name__}).", file=sys.stderr)
        return 2

    rendered = json.dumps(report, indent=2, sort_keys=True)
    if args.evidence_out:
        output = Path(args.evidence_out).expanduser()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered + "\n", encoding="utf-8")
        try:
            output.chmod(0o600)
        except OSError:
            print("WARNING: unable to restrict evidence file permissions.", file=sys.stderr)
        print(f"Evidence written to: {output}")
    print(rendered)
    failed = [name for name, passed in report["checks"].items() if passed is not True]
    if failed:
        print("BLOCKED: checks incomplete or failed: " + ", ".join(failed), file=sys.stderr)
        return 1
    print("PARTIAL PASS: file-level checks passed; runtime TLS/revocation, rotation, restart and terminal gates remain BLOCKED.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Create a LOCAL-ONLY development CA and mTLS certificates for Quant OS MT5.

This is not a production PKI and must not be used for a live trading host.
Private keys remain in the chosen local output directory. The script refuses to
overwrite any existing output-directory contents.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path


def run(*args: str) -> str:
    result = subprocess.run(
        ["openssl", *args], text=True, capture_output=True, check=False,
        env={**os.environ, "LC_ALL": "C"},
    )
    if result.returncode:
        detail = result.stderr.strip()[-500:]
        raise RuntimeError(f"OpenSSL command failed: {' '.join(args[:3])}; {detail}")
    return result.stdout.strip()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        default=str(Path.home() / "Library" / "Application Support" / "QuantOS" / "certs"),
        help="Empty protected local directory for development certificates.",
    )
    args = parser.parse_args()
    if shutil.which("openssl") is None:
        print("BLOCKED: openssl was not found on PATH.", file=sys.stderr)
        return 2

    out = Path(args.output_dir).expanduser().resolve()
    out.mkdir(parents=True, exist_ok=True)
    out.chmod(0o700)
    existing = sorted(path.name for path in out.iterdir())
    if existing:
        print("BLOCKED: output directory is not empty; refusing to overwrite: " + ", ".join(existing), file=sys.stderr)
        print("Choose a new empty directory or securely move the existing files first.", file=sys.stderr)
        return 2

    issued = out / "issued"
    issued.mkdir(mode=0o700)
    for name, value in (("index.txt", ""), ("serial", "1000\n"), ("crlnumber", "1000\n")):
        path = out / name
        path.write_text(value, encoding="ascii")
        path.chmod(0o600)

    config = f"""[ ca ]
default_ca = CA_default
[ CA_default ]
dir = {out}
database = $dir/index.txt
new_certs_dir = $dir/issued
certificate = $dir/dev-ca.pem
private_key = $dir/dev-ca.key.pem
serial = $dir/serial
crlnumber = $dir/crlnumber
default_md = sha256
default_days = 365
default_crl_days = 7
policy = policy_loose
x509_extensions = server_cert
copy_extensions = none
unique_subject = no
[ policy_loose ]
commonName = supplied
[ req ]
default_bits = 3072
default_md = sha256
prompt = no
distinguished_name = ca_dn
x509_extensions = v3_ca
[ ca_dn ]
CN = Quant OS LOCAL DEVELOPMENT ONLY CA
[ v3_ca ]
basicConstraints = critical,CA:true,pathlen:0
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
[ server_cert ]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature,keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @server_alt
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
[ server_alt ]
DNS.1 = localhost
IP.1 = 127.0.0.1
[ client_cert ]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature,keyEncipherment
extendedKeyUsage = clientAuth
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
"""
    config_path = out / "ca.cnf"
    config_path.write_text(config, encoding="utf-8")
    config_path.chmod(0o600)

    try:
        run("genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:3072", "-out", str(out / "dev-ca.key.pem"))
        (out / "dev-ca.key.pem").chmod(0o600)
        run("req", "-new", "-x509", "-key", str(out / "dev-ca.key.pem"), "-sha256", "-days", "3650",
            "-config", str(config_path), "-extensions", "v3_ca", "-out", str(out / "dev-ca.pem"))

        for role, common_name, extension in (
            ("server", "localhost", "server_cert"),
            ("client", "Quant OS MT5 local worker", "client_cert"),
        ):
            key = out / f"{role}.key.pem"
            csr = out / f"{role}.csr.pem"
            cert = out / f"{role}.pem"
            run("genpkey", "-algorithm", "RSA", "-pkeyopt", "rsa_keygen_bits:2048", "-out", str(key))
            key.chmod(0o600)
            run("req", "-new", "-key", str(key), "-subj", f"/CN={common_name}", "-out", str(csr))
            csr.chmod(0o600)
            run("ca", "-batch", "-config", str(config_path), "-extensions", extension,
                "-in", str(csr), "-out", str(cert), "-notext", "-md", "sha256")
            cert.chmod(0o644)

        run("ca", "-gencrl", "-config", str(config_path), "-out", str(out / "dev-ca.crl.pem"))
        (out / "dev-ca.crl.pem").chmod(0o644)

        # Validate chain + CRL, certificate/key pairing, hostname/IP SAN, and EKU.
        for role in ("server", "client"):
            run("verify", "-crl_check", "-CRLfile", str(out / "dev-ca.crl.pem"),
                "-CAfile", str(out / "dev-ca.pem"), str(out / f"{role}.pem"))
            cert_pub = run("x509", "-in", str(out / f"{role}.pem"), "-pubkey", "-noout")
            key_pub = run("pkey", "-in", str(out / f"{role}.key.pem"), "-pubout")
            if cert_pub != key_pub:
                raise RuntimeError(f"{role} certificate does not match its private key")
        run("x509", "-in", str(out / "server.pem"), "-noout", "-checkip", "127.0.0.1")
        run("x509", "-in", str(out / "server.pem"), "-noout", "-checkhost", "localhost")
        if "SSL server : Yes" not in run("x509", "-in", str(out / "server.pem"), "-noout", "-purpose"):
            raise RuntimeError("server certificate does not pass OpenSSL server-purpose check")
        if "SSL client : Yes" not in run("x509", "-in", str(out / "client.pem"), "-noout", "-purpose"):
            raise RuntimeError("client certificate does not pass OpenSSL client-purpose check")

        der = subprocess.run(
            ["openssl", "x509", "-in", str(out / "client.pem"), "-outform", "DER"],
            capture_output=True, check=True,
        ).stdout
        fingerprint = hashlib.sha256(der).hexdigest()
        print("LOCAL DEVELOPMENT CERTIFICATES CREATED AND VERIFIED")
        print("Scope: local development only; not production-issued and not release approval.")
        print(f"Output directory: {out}")
        print(f"CA certificate: {out / 'dev-ca.pem'}")
        print(f"CA CRL: {out / 'dev-ca.crl.pem'}")
        print(f"Server certificate: {out / 'server.pem'}")
        print(f"Server private key: {out / 'server.key.pem'} (owner-only)")
        print(f"Client certificate: {out / 'client.pem'}")
        print(f"Client private key: {out / 'client.key.pem'} (owner-only)")
        print(f"Client SHA-256 DER fingerprint (MT5_BRIDGE_PRINCIPAL_SHA256): {fingerprint}")
        print("For this local-only setup use dev-ca.pem as BOTH MT5_BRIDGE_CA_FILE and MT5_BRIDGE_CLIENT_CA.")
        print("Use server.pem/server.key.pem for the daemon; use client.pem/client.key.pem for the worker.")
        print("Use dev-ca.crl.pem with the certificate-review helper's --crl-file option.")
        print("Do not copy, upload, or commit the CA private key.")
        return 0
    except (OSError, subprocess.CalledProcessError, RuntimeError) as exc:
        print(f"BLOCKED: certificate generation/verification failed ({type(exc).__name__}): {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from qos020_guard import GuardError, parse_url, validate_identity  # noqa: E402


RUN = "20261110-214200-a1b2c3d4e5f6"
PORT = 49173
URL = f"postgresql://qos020_sqlx:abc123@127.0.0.1:{PORT}/qos020_test_master"
PROJECT = f"qos020-integration-{RUN}"
VOLUME = "qos020_it_20261110_214200_a1b2c3d4e5f6"
NETWORK = f"qos020-integration-net-{RUN}"
IMAGE = "postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777"


def fixtures() -> tuple[dict[str, object], dict[str, object]]:
    manifest = {
        "lane": "integration-test", "environment_type": "integration-test", "run_id": RUN,
        "provisioning_id": RUN, "postgres_major": 16, "project": PROJECT,
        "instance_creation_identity": "creation-id",
        "image_reference": IMAGE, "container_id": "sha256:container", "container_name": PROJECT + "-postgres-1",
        "volume": VOLUME, "network": NETWORK, "compose_working_dir": "/tmp/qos020",
        "compose_file": "/tmp/qos020/docker-compose.qos020.yml", "host": "127.0.0.1",
        "port": PORT, "database": "qos020_test_master",
    }
    evidence = {
        "container_id": manifest["container_id"], "container_running": True,
        "container_project": PROJECT, "container_run_id": RUN, "container_environment": "integration-test",
        "container_provisioning_id": RUN,
        "container_service": "postgres", "container_name": manifest["container_name"],
        "container_working_dir": manifest["compose_working_dir"], "container_config_file": manifest["compose_file"],
        "container_mount_volume": VOLUME, "container_network": NETWORK,
        "volume_name": VOLUME, "volume_project": PROJECT, "volume_run_id": RUN, "volume_environment": "integration-test",
        "volume_provisioning_id": RUN,
        "network_name": NETWORK, "network_project": PROJECT, "network_run_id": RUN, "network_environment": "integration-test",
        "network_provisioning_id": RUN,
        "network_container_ids": ["sha256:container"],
        "postgres_major": 16, "database_name": "qos020_test_master", "marker_environment": "integration-test",
        "marker_run_id": RUN, "marker_provisioning_id": RUN, "marker_expected_major": 16,
        "marker_instance_creation_identity": "creation-id", "sqlx_role_login": True,
        "sqlx_role_createdb": True, "sqlx_role_superuser": False, "sqlx_role_createrole": False,
        "sqlx_role_replication": False, "sqlx_role_bypassrls": False,
        "database_url_password_present": True, "database_url_password_matches_secret": True,
    }
    return manifest, evidence


class GuardTests(unittest.TestCase):
    def assert_rejected(self, manifest=None, url=URL, evidence=None):
        base_manifest, base_evidence = fixtures()
        with self.assertRaises(GuardError):
            validate_identity(manifest or base_manifest, url, evidence or base_evidence)

    def test_valid_registered_instance_passes(self):
        manifest, evidence = fixtures()
        self.assertEqual(validate_identity(manifest, URL, evidence)["port"], PORT)

    def test_bridge_staging_database_name_rejected(self):
        self.assert_rejected(url=f"postgresql://qos020_sqlx:abc123@127.0.0.1:{PORT}/mt5_v2_staging")

    def test_protected_port_15543_rejected(self):
        self.assert_rejected(url="postgresql://qos020_sqlx:abc123@127.0.0.1:15543/qos020_test_master")

    def test_legacy_port_15433_rejected(self):
        self.assert_rejected(url="postgresql://qos020_sqlx:abc123@127.0.0.1:15433/qos020_test_master")

    def test_production_like_endpoint_rejected(self):
        self.assert_rejected(url=f"postgresql://qos020_sqlx:abc123@prod-db.example.com:{PORT}/qos020_test_master")

    def test_missing_marker_rejected(self):
        evidence = fixtures()[1]
        evidence["marker_environment"] = None
        self.assert_rejected(evidence=evidence)

    def test_wrong_marker_rejected(self):
        evidence = fixtures()[1]
        evidence["marker_environment"] = "staging"
        self.assert_rejected(evidence=evidence)

    def test_wrong_run_id_rejected(self):
        evidence = fixtures()[1]
        evidence["marker_run_id"] = "another-run"
        self.assert_rejected(evidence=evidence)

    def test_wrong_compose_project_rejected(self):
        evidence = fixtures()[1]
        evidence["container_project"] = "event-driven-trading-rs"
        self.assert_rejected(evidence=evidence)

    def test_wrong_volume_rejected(self):
        evidence = fixtures()[1]
        evidence["container_mount_volume"] = "edgenest_mt5_bridge_v2_staging_data"
        self.assert_rejected(evidence=evidence)

    def test_wrong_container_rejected(self):
        evidence = fixtures()[1]
        evidence["container_id"] = "protected-container"
        self.assert_rejected(evidence=evidence)

    def test_unexpected_postgres_version_rejected(self):
        evidence = fixtures()[1]
        evidence["postgres_major"] = 17
        self.assert_rejected(evidence=evidence)

    def test_missing_credentials_rejected(self):
        self.assert_rejected(url=f"postgresql://qos020_sqlx@127.0.0.1:{PORT}/qos020_test_master")

    def test_unknown_endpoint_rejected(self):
        self.assert_rejected(url=f"postgresql://qos020_sqlx:abc123@127.0.0.1:{PORT + 1}/qos020_test_master")

    def test_malformed_url_rejected(self):
        self.assert_rejected(url="postgres://not a URL")

    def test_valid_url_without_docker_identity_rejected(self):
        evidence = fixtures()[1]
        evidence["container_id"] = "some-postgres-container"
        self.assert_rejected(evidence=evidence)

    def test_wrong_lane_and_run_id_rejected(self):
        manifest = fixtures()[0]
        manifest["lane"] = "staging"
        self.assert_rejected(manifest=manifest)

    def test_sqlx_role_privilege_escalation_rejected(self):
        evidence = fixtures()[1]
        evidence["sqlx_role_superuser"] = True
        self.assert_rejected(evidence=evidence)

    def test_database_url_password_must_match_secret_file(self):
        evidence = fixtures()[1]
        evidence["database_url_password_matches_secret"] = False
        self.assert_rejected(evidence=evidence)


if __name__ == "__main__":
    unittest.main(verbosity=2)

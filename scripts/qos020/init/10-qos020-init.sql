SELECT format('CREATE ROLE qos020_sqlx LOGIN CREATEDB PASSWORD %L', :'qos020_sqlx_password')
\gexec
GRANT CONNECT, CREATE ON DATABASE qos020_test_master TO qos020_sqlx;
GRANT USAGE, CREATE ON SCHEMA public TO qos020_sqlx;

CREATE SCHEMA qos020_control AUTHORIZATION qos020_bootstrap;
REVOKE ALL ON SCHEMA qos020_control FROM PUBLIC;
CREATE TABLE qos020_control.environment_identity (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    environment_type text NOT NULL,
    run_id text NOT NULL,
    provisioning_id text NOT NULL,
    expected_postgres_major integer NOT NULL,
    instance_creation_identity text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
INSERT INTO qos020_control.environment_identity (
    environment_type, run_id, provisioning_id, expected_postgres_major,
    instance_creation_identity
) VALUES (
    'integration-test', :'qos020_run_id', :'qos020_provisioning_id',
    :'qos020_expected_pg_major'::integer, :'qos020_instance_id'
);
GRANT USAGE ON SCHEMA qos020_control TO qos020_sqlx;
GRANT SELECT ON qos020_control.environment_identity TO qos020_sqlx;

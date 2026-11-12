-- Preserve legacy environment values while making their provenance explicit.
ALTER TABLE accounts ALTER COLUMN environment DROP DEFAULT;
ALTER TABLE accounts ALTER COLUMN environment DROP NOT NULL;

ALTER TABLE accounts
    ADD COLUMN environment_verification TEXT,
    ADD COLUMN environment_revision BIGINT,
    ADD COLUMN environment_verified_at TIMESTAMPTZ,
    ADD COLUMN environment_verified_by TEXT,
    ADD COLUMN environment_verification_reason TEXT;

UPDATE accounts SET
    environment_verification = CASE WHEN environment IS NULL THEN 'UNCLASSIFIED' ELSE 'UNVERIFIED_LEGACY' END,
    environment_revision = 0
WHERE environment_verification IS NULL;

ALTER TABLE accounts ALTER COLUMN environment_verification SET NOT NULL;
ALTER TABLE accounts ALTER COLUMN environment_revision SET NOT NULL;
ALTER TABLE accounts ALTER COLUMN environment_verification SET DEFAULT 'UNVERIFIED';
ALTER TABLE accounts ALTER COLUMN environment_revision SET DEFAULT 1;

ALTER TABLE accounts ADD CONSTRAINT accounts_environment_valid
    CHECK (environment IS NULL OR environment IN ('PAPER','SHADOW','LIVE'));
ALTER TABLE accounts ADD CONSTRAINT accounts_environment_verification_valid
    CHECK (environment_verification IN ('UNCLASSIFIED','UNVERIFIED','UNVERIFIED_LEGACY','VERIFIED'));
ALTER TABLE accounts ADD CONSTRAINT accounts_environment_revision_valid
    CHECK (environment_revision >= 0);
ALTER TABLE accounts ADD CONSTRAINT accounts_environment_provenance_consistent CHECK (
    (environment_verification = 'UNCLASSIFIED' AND environment IS NULL
        AND environment_verified_at IS NULL AND environment_verified_by IS NULL)
    OR (environment_verification IN ('UNVERIFIED','UNVERIFIED_LEGACY') AND environment IS NOT NULL
        AND environment_verified_at IS NULL AND environment_verified_by IS NULL)
    OR (environment_verification = 'VERIFIED' AND environment IS NOT NULL
        AND environment_revision > 0 AND environment_verified_at IS NOT NULL
        AND environment_verified_by IS NOT NULL AND length(trim(environment_verified_by)) > 0)
);
ALTER TABLE accounts ADD CONSTRAINT accounts_environment_verifier_fk
    FOREIGN KEY (environment_verified_by) REFERENCES identity_users(id) ON DELETE RESTRICT;

-- Existing rows receive UNVERIFIED_LEGACY; INSERT callers must explicitly provide environment.
CREATE FUNCTION require_explicit_account_environment() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.environment IS NULL THEN
        RAISE EXCEPTION 'new trading accounts require an explicit environment';
    END IF;
    IF NEW.environment_verification = 'UNCLASSIFIED' THEN
        RAISE EXCEPTION 'new trading accounts cannot be UNCLASSIFIED';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER accounts_require_explicit_environment
    BEFORE INSERT ON accounts FOR EACH ROW EXECUTE FUNCTION require_explicit_account_environment();

CREATE FUNCTION protect_account_environment_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF ROW(NEW.environment, NEW.environment_verification, NEW.environment_verified_at, NEW.environment_verified_by)
       IS DISTINCT FROM ROW(OLD.environment, OLD.environment_verification, OLD.environment_verified_at, OLD.environment_verified_by)
       AND NEW.environment_revision <= OLD.environment_revision THEN
        RAISE EXCEPTION 'environment revision must increase when classification or verification changes';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER accounts_environment_revision_monotonic
    BEFORE UPDATE ON accounts FOR EACH ROW EXECUTE FUNCTION protect_account_environment_revision();

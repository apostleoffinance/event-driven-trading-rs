use std::{collections::BTreeMap, fs, path::Path};

use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessClaims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
    pub token_type: String,
    pub jti: String,
    #[serde(rename = "sid")]
    pub session_id: String,
    #[serde(rename = "srv")]
    pub session_revision: i64,
}

pub struct JwtKeyRing {
    signing_keys: BTreeMap<String, EncodingKey>,
    verification_keys: BTreeMap<String, DecodingKey>,
    active_kid: String,
    validation: Validation,
    issuer: String,
    audience: String,
}

impl JwtKeyRing {
    pub fn verification_only(
        verification_pems: BTreeMap<String, Vec<u8>>,
        issuer: String,
        audience: String,
    ) -> Result<Self, String> {
        if issuer.trim().is_empty() || audience.trim().is_empty() || verification_pems.is_empty() {
            return Err("JWT verification keys, issuer, and audience must be configured".into());
        }
        let verification_keys = verification_pems
            .into_iter()
            .map(|(kid, pem)| {
                validate_kid(&kid)?;
                DecodingKey::from_rsa_pem(&pem)
                    .map(|key| (kid, key))
                    .map_err(|_| "invalid JWT RSA public key".to_owned())
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            signing_keys: BTreeMap::new(),
            verification_keys,
            active_kid: String::new(),
            validation: validation_for(&issuer, &audience),
            issuer,
            audience,
        })
    }

    pub fn from_files(
        active_kid: &str,
        signing_specs: &str,
        verification_specs: &str,
        issuer: String,
        audience: String,
    ) -> Result<Self, String> {
        let signing = read_key_files(signing_specs, true)?;
        let verification = read_key_files(verification_specs, false)?;
        Self::from_pem_maps(active_kid, signing, verification, issuer, audience)
    }

    pub fn from_pem_maps(
        active_kid: &str,
        signing_pems: BTreeMap<String, Vec<u8>>,
        verification_pems: BTreeMap<String, Vec<u8>>,
        issuer: String,
        audience: String,
    ) -> Result<Self, String> {
        validate_kid(active_kid)?;
        if issuer.trim().is_empty() || audience.trim().is_empty() {
            return Err("JWT issuer and audience must be non-empty".into());
        }
        if signing_pems.len() != 1 || verification_pems.is_empty() {
            return Err("exactly one JWT private signing key and at least one verification key are required".into());
        }
        if signing_pems.keys().next().map(String::as_str) != Some(active_kid)
            || !verification_pems.contains_key(active_kid)
        {
            return Err("active JWT key id must have a signing and verification key".into());
        }
        for kid in verification_pems.keys() {
            validate_kid(kid)?;
        }

        let signing_keys = signing_pems
            .into_iter()
            .map(|(kid, pem)| {
                EncodingKey::from_rsa_pem(&pem)
                    .map(|key| (kid, key))
                    .map_err(|_| "invalid JWT RSA private key".to_owned())
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let verification_keys = verification_pems
            .into_iter()
            .map(|(kid, pem)| {
                DecodingKey::from_rsa_pem(&pem)
                    .map(|key| (kid, key))
                    .map_err(|_| "invalid JWT RSA public key".to_owned())
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        let mut ring = Self {
            signing_keys,
            verification_keys,
            active_kid: active_kid.to_owned(),
            validation: validation_for(&issuer, &audience),
            issuer,
            audience,
        };
        ring.verify_active_pair()?;
        Ok(ring)
    }

    pub fn issue(&self, claims: &AccessClaims) -> Result<String, String> {
        let key = self
            .signing_keys
            .get(&self.active_kid)
            .ok_or_else(|| "active JWT signing key is unavailable".to_owned())?;
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.active_kid.clone());
        encode(&header, claims, key).map_err(|_| "JWT signing failed".to_owned())
    }

    pub fn verify(&self, token: &str) -> Result<AccessClaims, String> {
        let header = decode_header(token).map_err(|_| "invalid JWT header".to_owned())?;
        if header.alg != Algorithm::RS256 {
            return Err("unsupported JWT algorithm".into());
        }
        let kid = header
            .kid
            .ok_or_else(|| "JWT key id is required".to_owned())?;
        let key = self
            .verification_keys
            .get(&kid)
            .ok_or_else(|| "JWT key id is not trusted".to_owned())?;
        let claims = decode::<AccessClaims>(token, key, &self.validation)
            .map_err(|_| "JWT validation failed".to_owned())?
            .claims;
        if claims.iss != self.issuer
            || claims.aud != self.audience
            || claims.token_type != "access"
            || claims.sub.trim().is_empty()
            || claims.jti.trim().is_empty()
            || claims.session_id.trim().is_empty()
            || claims.session_revision < 1
        {
            return Err("JWT claims are invalid".into());
        }
        Ok(claims)
    }

    pub fn active_kid(&self) -> &str {
        &self.active_kid
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn audience(&self) -> &str {
        &self.audience
    }

    pub fn verification_key_ids(&self) -> impl Iterator<Item = &str> {
        self.verification_keys.keys().map(String::as_str)
    }

    fn verify_active_pair(&mut self) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "system clock is before Unix epoch".to_owned())?
            .as_secs() as usize;
        let probe = AccessClaims {
            sub: "startup-key-pair-check".into(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            exp: now + 60,
            iat: now,
            token_type: "access".into(),
            jti: "startup-key-pair-check".into(),
            session_id: "startup-key-pair-check".into(),
            session_revision: 1,
        };
        let token = self.issue(&probe)?;
        self.validation.validate_exp = true;
        let key = self
            .verification_keys
            .get(&self.active_kid)
            .ok_or_else(|| "active JWT verification key is unavailable".to_owned())?;
        let checked = decode::<AccessClaims>(&token, key, &self.validation)
            .map_err(|_| "active JWT signing and verification keys do not match".to_owned())?;
        if checked.claims != probe {
            return Err("active JWT key pair validation failed".into());
        }
        Ok(())
    }
}

fn validation_for(issuer: &str, audience: &str) -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = 0;
    validation
        .required_spec_claims
        .extend(["sub", "exp", "iat", "iss", "aud"].map(str::to_owned));
    validation
}

fn validate_kid(kid: &str) -> Result<(), String> {
    if kid.is_empty()
        || kid.len() > 64
        || !kid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("JWT key ids must be 1-64 ASCII letters, digits, '.', '_' or '-'".into());
    }
    Ok(())
}

fn read_key_files(spec: &str, private: bool) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut keys = BTreeMap::new();
    for entry in spec.split(';').filter(|entry| !entry.trim().is_empty()) {
        let (kid, path) = entry
            .split_once('=')
            .ok_or_else(|| "JWT key entries must use kid=path format".to_owned())?;
        let kid = kid.trim();
        validate_kid(kid)?;
        let path = Path::new(path.trim());
        if keys.contains_key(kid) {
            return Err("duplicate JWT key id".into());
        }
        if private && !private_key_file_is_restricted(path) {
            return Err(
                "JWT private key file permissions must exclude group and other access".into(),
            );
        }
        let pem =
            fs::read(path).map_err(|_| "unable to read configured JWT key file".to_owned())?;
        keys.insert(kid.to_owned(), pem);
    }
    Ok(keys)
}

fn private_key_file_is_restricted(path: &Path) -> bool {
    private_key_file_is_restricted_in(path, Path::new("/etc/secrets"))
}

fn private_key_file_is_restricted_in(path: &Path, secret_dir: &Path) -> bool {
    // Render-managed Secret Files live under /etc/secrets. Some secret mounts
    // expose entries as symlinks, so inspect the target while ensuring it
    // remains inside the trusted mount. Never extend this exception to
    // symlinks or broadly-permissioned files elsewhere on the filesystem.
    let is_secret_path = path.starts_with(secret_dir)
        && path
            .components()
            .all(|component| !matches!(component, std::path::Component::ParentDir));

    let Ok(entry_metadata) = fs::symlink_metadata(path) else {
        return false;
    };

    if entry_metadata.file_type().is_symlink() {
        if !is_secret_path {
            return false;
        }
        let (Ok(secret_root), Ok(resolved_path)) =
            (fs::canonicalize(secret_dir), fs::canonicalize(path))
        else {
            return false;
        };
        if !resolved_path.starts_with(secret_root) {
            return false;
        }
        return fs::metadata(path).is_ok_and(|metadata| metadata.is_file());
    }

    if !entry_metadata.file_type().is_file() {
        return false;
    }
    if is_secret_path {
        return true;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        entry_metadata.permissions().mode() & 0o077 == 0
    }
    #[cfg(not(unix))]
    {
        // ACL enforcement is platform-specific; deployment policy must supply protected files.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE: &[u8] = include_bytes!("../../journal-api/tests/fixtures/r2_test_private.pem");
    const PUBLIC: &[u8] = include_bytes!("../../journal-api/tests/fixtures/r2_test_public.pem");
    const ROTATED_PRIVATE: &[u8] = include_bytes!("../tests/fixtures/rotation_test_private.pem");
    const ROTATED_PUBLIC: &[u8] = include_bytes!("../tests/fixtures/rotation_test_public.pem");

    fn keys(active: &str) -> JwtKeyRing {
        configured(active, &["key-1", "key-2"])
    }

    fn configured(active: &str, trusted: &[&str]) -> JwtKeyRing {
        let mut signing = BTreeMap::new();
        let active_private = if active == "key-2" {
            ROTATED_PRIVATE
        } else {
            PRIVATE
        };
        signing.insert(active.to_owned(), active_private.to_vec());
        let mut verification = BTreeMap::new();
        for kid in trusted {
            let public = if *kid == "key-2" {
                ROTATED_PUBLIC
            } else {
                PUBLIC
            };
            verification.insert((*kid).to_owned(), public.to_vec());
        }
        JwtKeyRing::from_pem_maps(
            active,
            signing,
            verification,
            "test-issuer".into(),
            "test-audience".into(),
        )
        .unwrap()
    }

    fn claims() -> AccessClaims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        AccessClaims {
            sub: "user-1".into(),
            iss: "test-issuer".into(),
            aud: "test-audience".into(),
            exp: now + 60,
            iat: now,
            token_type: "access".into(),
            jti: "token-1".into(),
            session_id: "session-1".into(),
            session_revision: 1,
        }
    }

    #[test]
    fn active_key_id_is_signed_and_old_key_remains_valid_during_overlap() {
        let old = keys("key-1");
        let token = old.issue(&claims()).unwrap();
        assert_eq!(old.verify(&token).unwrap(), claims());

        let rotating = keys("key-2");
        assert_eq!(rotating.active_kid(), "key-2");
        assert_eq!(rotating.verify(&token).unwrap(), claims());
        assert_eq!(rotating.verification_key_ids().count(), 2);
    }

    #[test]
    fn expired_token_is_rejected() {
        let keyring = keys("key-1");
        let mut expired = claims();
        expired.exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize
            - 1;
        let token = keyring.issue(&expired).unwrap();
        assert!(keyring.verify(&token).is_err());
    }

    #[test]
    fn rotation_rollback_restores_old_signer_while_trusting_both_kids() {
        let before = configured("key-1", &["key-1"]);
        let old_token = before.issue(&claims()).unwrap();
        let rotated = keys("key-2");
        let rotated_token = rotated.issue(&claims()).unwrap();
        assert!(rotated.verify(&old_token).is_ok());

        let rollback = configured("key-1", &["key-1", "key-2"]);
        assert_eq!(rollback.active_kid(), "key-1");
        assert!(rollback.verify(&old_token).is_ok());
        assert!(rollback.verify(&rotated_token).is_ok());
        let rollback_token = rollback.issue(&claims()).unwrap();
        assert!(before.verify(&rollback_token).is_ok());
    }

    #[test]
    fn retirement_removes_trust_for_old_key_id() {
        let before = keys("key-1");
        let token = before.issue(&claims()).unwrap();
        let after = keys("key-2");
        // The test's rotated ring includes both keys; a one-key ring without key-1
        // rejects the retired key id.
        let mut signing = BTreeMap::new();
        signing.insert("key-2".into(), ROTATED_PRIVATE.to_vec());
        let mut verification = BTreeMap::new();
        verification.insert("key-2".into(), ROTATED_PUBLIC.to_vec());
        let retired = JwtKeyRing::from_pem_maps(
            "key-2",
            signing,
            verification,
            "test-issuer".into(),
            "test-audience".into(),
        )
        .unwrap();
        assert!(retired.verify(&token).is_err());
        assert_eq!(after.active_kid(), "key-2");
    }

    #[test]
    fn invalid_key_material_duplicate_ids_and_unknown_kid_fail_closed() {
        let mut signing = BTreeMap::new();
        signing.insert("key-1".into(), b"not a key".to_vec());
        let mut verification = BTreeMap::new();
        verification.insert("key-1".into(), PUBLIC.to_vec());
        assert!(
            JwtKeyRing::from_pem_maps("key-1", signing, verification, "i".into(), "a".into())
                .is_err()
        );
        let mut mismatched_signing = BTreeMap::new();
        mismatched_signing.insert("key-1".into(), PRIVATE.to_vec());
        let mut mismatched_verification = BTreeMap::new();
        mismatched_verification.insert("key-1".into(), ROTATED_PUBLIC.to_vec());
        assert!(JwtKeyRing::from_pem_maps(
            "key-1",
            mismatched_signing,
            mismatched_verification,
            "i".into(),
            "a".into()
        )
        .is_err());
        assert!(validate_kid("bad kid").is_err());
        assert!(read_key_files("key-1=/missing;key-1=/also-missing", false).is_err());
    }

    #[test]
    fn jwt_unknown_kid_signature_issuer_audience_and_type_are_rejected() {
        let ring = configured("key-1", &["key-1"]);
        let mut unknown_header = Header::new(Algorithm::RS256);
        unknown_header.kid = Some("retired-or-unknown".into());
        let unknown = encode(
            &unknown_header,
            &claims(),
            &EncodingKey::from_rsa_pem(PRIVATE).unwrap(),
        )
        .unwrap();
        assert!(ring.verify(&unknown).is_err());

        let trusted = ring.issue(&claims()).unwrap();
        let mut parts = trusted.split('.').map(str::to_owned).collect::<Vec<_>>();
        let replacement = if parts[2].starts_with('A') { "B" } else { "A" };
        parts[2].replace_range(0..1, replacement);
        assert!(ring.verify(&parts.join(".")).is_err());

        for (issuer, audience, token_type) in [
            ("other-issuer", "test-audience", "access"),
            ("test-issuer", "other-audience", "access"),
            ("test-issuer", "test-audience", "refresh"),
        ] {
            let mut invalid_claims = claims();
            invalid_claims.iss = issuer.into();
            invalid_claims.aud = audience.into();
            invalid_claims.token_type = token_type.into();
            let token = ring.issue(&invalid_claims).unwrap();
            assert!(ring.verify(&token).is_err());
        }

        let mut incomplete_header = Header::new(Algorithm::RS256);
        incomplete_header.kid = Some("key-1".into());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        #[derive(Serialize)]
        struct IncompleteClaims {
            sub: &'static str,
            iss: &'static str,
            aud: &'static str,
            exp: usize,
            iat: usize,
            token_type: &'static str,
        }
        let incomplete = IncompleteClaims {
            sub: "user-1",
            iss: "test-issuer",
            aud: "test-audience",
            exp: now + 60,
            iat: now,
            token_type: "access",
        };
        let token = encode(
            &incomplete_header,
            &incomplete,
            &EncodingKey::from_rsa_pem(PRIVATE).unwrap(),
        )
        .unwrap();
        assert!(ring.verify(&token).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_key_file_permissions_are_enforced() {
        use std::{fs, os::unix::fs::PermissionsExt};

        let path = std::env::temp_dir().join(format!(
            "edgenest-jwt-private-key-{}-{}",
            std::process::id(),
            uuid_for_test()
        ));
        fs::write(&path, PRIVATE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read_key_files(&format!("key-1={}", path.display()), true).is_ok());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_key_files(&format!("key-1={}", path.display()), true).is_err());
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn render_secret_symlink_to_file_inside_mount_is_allowed() {
        use std::{
            fs,
            os::unix::fs::{symlink, PermissionsExt},
        };

        let root = std::env::temp_dir().join(format!(
            "edgenest-render-secret-{}-{}",
            std::process::id(),
            uuid_for_test()
        ));
        let secret_dir = root.join("secrets");
        fs::create_dir_all(&secret_dir).unwrap();

        let target = secret_dir.join("private-key-target.pem");
        let link = secret_dir.join("private-key.pem");
        fs::write(&target, PRIVATE).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        symlink(&target, &link).unwrap();

        assert!(private_key_file_is_restricted_in(&link, &secret_dir));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn render_secret_symlink_cannot_escape_secret_mount() {
        use std::{fs, os::unix::fs::symlink};

        let root = std::env::temp_dir().join(format!(
            "edgenest-render-secret-escape-{}-{}",
            std::process::id(),
            uuid_for_test()
        ));
        let secret_dir = root.join("secrets");
        fs::create_dir_all(&secret_dir).unwrap();

        let outside_target = root.join("outside-private-key.pem");
        let link = secret_dir.join("private-key.pem");
        fs::write(&outside_target, PRIVATE).unwrap();
        symlink(&outside_target, &link).unwrap();

        assert!(!private_key_file_is_restricted_in(&link, &secret_dir));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_private_key_outside_secret_mount_is_rejected() {
        use std::{fs, os::unix::fs::symlink};

        let root = std::env::temp_dir().join(format!(
            "edgenest-private-key-link-{}-{}",
            std::process::id(),
            uuid_for_test()
        ));
        fs::create_dir_all(&root).unwrap();

        let target = root.join("private-key-target.pem");
        let link = root.join("private-key.pem");
        fs::write(&target, PRIVATE).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &link).unwrap();

        assert!(!private_key_file_is_restricted_in(&link, &root));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    fn uuid_for_test() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string()
    }
}

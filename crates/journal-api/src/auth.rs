use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use identity_token::{AccessClaims, JwtKeyRing};
use std::{collections::BTreeMap, sync::Arc};

pub type Claims = AccessClaims;

#[derive(Clone)]
pub struct JwtConfig {
    ring: Arc<JwtKeyRing>,
}

impl JwtConfig {
    pub fn new(public_key_pem: &[u8], issuer: String, audience: String) -> Result<Self, String> {
        let mut verification = BTreeMap::new();
        verification.insert("test-key".to_owned(), public_key_pem.to_vec());
        let ring = JwtKeyRing::verification_only(verification, issuer, audience)?;
        Ok(Self {
            ring: Arc::new(ring),
        })
    }

    pub fn from_files(
        verification_specs: &str,
        issuer: String,
        audience: String,
    ) -> Result<Self, String> {
        let mut verification = BTreeMap::new();
        for entry in verification_specs
            .split(';')
            .filter(|entry| !entry.trim().is_empty())
        {
            let (kid, path) = entry
                .split_once('=')
                .ok_or_else(|| "JWT verification entries must use kid=path format".to_owned())?;
            if verification
                .insert(
                    kid.trim().to_owned(),
                    std::fs::read(path.trim())
                        .map_err(|_| "unable to read configured JWT verification key".to_owned())?,
                )
                .is_some()
            {
                return Err("duplicate JWT verification key id".into());
            }
        }
        let ring = JwtKeyRing::verification_only(verification, issuer, audience)?;
        Ok(Self {
            ring: Arc::new(ring),
        })
    }

    pub fn authenticate(&self, headers: &HeaderMap) -> Result<Claims, ApiAuthError> {
        let raw = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiAuthError)?;
        let token = raw.strip_prefix("Bearer ").ok_or(ApiAuthError)?;
        self.ring.verify(token).map_err(|_| ApiAuthError)
    }

    pub fn key_ring(&self) -> &JwtKeyRing {
        &self.ring
    }
}

#[derive(Debug)]
pub struct ApiAuthError;

impl axum::response::IntoResponse for ApiAuthError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::UNAUTHORIZED, axum::Json(serde_json::json!({
            "error": {"code": "unauthorized", "message": "A valid EdgeNest bearer token is required"}
        }))).into_response()
    }
}

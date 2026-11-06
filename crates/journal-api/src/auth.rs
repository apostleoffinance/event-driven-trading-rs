use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub token_type: String,
    #[serde(default)]
    pub iat: Option<usize>,
}

#[derive(Clone)]
pub struct JwtConfig {
    key: DecodingKey,
    keys: HashMap<String, DecodingKey>,
    validation: Validation,
}

impl JwtConfig {
    pub fn new(
        public_key_pem: &[u8],
        active_kid: String,
        public_keys_json: Option<&str>,
        issuer: String,
        audience: String,
    ) -> Result<Self, String> {
        if active_kid.trim().is_empty() { return Err("EDGENEST_JWT_KEY_ID cannot be empty".into()); }
        let key = DecodingKey::from_rsa_pem(public_key_pem)
            .map_err(|e| format!("EDGENEST_JWT_PUBLIC_KEY_PEM must be an RSA public key: {e}"))?;
        let active_pem = std::str::from_utf8(public_key_pem).map_err(|e|e.to_string())?;
        #[derive(Deserialize)]
        struct KeyInput {
            kid: String,
            pem: String,
        }
        let mut keys = HashMap::new();
        let inputs = match public_keys_json {
            Some(json) => serde_json::from_str::<Vec<KeyInput>>(json)
                .map_err(|e| format!("invalid EDGENEST_JWT_PUBLIC_KEYS_JSON: {e}"))?,
            None => Vec::new(),
        };
        for item in inputs {
            let pem = item.pem.replace("\\n", "\n");
            let decoder = DecodingKey::from_rsa_pem(pem.as_bytes())
                .map_err(|e| format!("invalid public key {}: {e}", item.kid))?;
            if item.kid == active_kid && pem != active_pem { return Err("active JWT KID PEM does not match EDGENEST_JWT_PUBLIC_KEY_PEM".into()); }
            if keys.contains_key(&item.kid) { return Err(format!("duplicate JWT KID: {}", item.kid)); }
            keys.insert(item.kid, decoder);
        }
        if keys.is_empty() {
            keys.insert(active_kid, key.clone());
        }
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.required_spec_claims.insert("sub".to_owned());
        validation.required_spec_claims.insert("exp".to_owned());
        Ok(Self {
            key,
            keys,
            validation,
        })
    }

    pub fn authenticate(&self, headers: &HeaderMap) -> Result<Claims, ApiAuthError> {
        let raw = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiAuthError)?;
        let token = raw.strip_prefix("Bearer ").ok_or(ApiAuthError)?;
        let header = jsonwebtoken::decode_header(token).map_err(|_| ApiAuthError)?;
        let claims = if let Some(kid) = header.kid.as_deref() {
            let key = self.keys.get(kid).ok_or(ApiAuthError)?;
            decode::<Claims>(token, key, &self.validation)
                .map_err(|_| ApiAuthError)?
                .claims
        } else {
            let mut result = decode::<Claims>(token, &self.key, &self.validation)
                .ok()
                .map(|d| d.claims);
            if result.is_none() {
                for key in self.keys.values() {
                    if let Ok(data) = decode::<Claims>(token, key, &self.validation) {
                        result = Some(data.claims);
                        break;
                    }
                }
            }
            result.ok_or(ApiAuthError)?
        };
        if claims.sub.trim().is_empty() || claims.token_type != "access" {
            return Err(ApiAuthError);
        }
        Ok(claims)
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

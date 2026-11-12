use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{organizations_api, AppState};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct BrokerIdentityHmac {
    keys: Vec<BrokerIdentifierKey>,
    active_key_version: String,
}

#[derive(Clone)]
struct BrokerIdentifierKey {
    version: String,
    key: [u8; 32],
}

impl BrokerIdentityHmac {
    pub fn from_environment() -> Option<Self> {
        let mut keys = Vec::new();
        if let Ok(keyset) = std::env::var("EDGENEST_BROKER_ID_HMAC_KEYSET") {
            for entry in keyset.split(',') {
                let (version, encoded) = entry.split_once(':')?;
                let version = version.trim();
                if version.is_empty()
                    || version.len() > 64
                    || keys
                        .iter()
                        .any(|key: &BrokerIdentifierKey| key.version == version)
                {
                    return None;
                }
                keys.push(BrokerIdentifierKey {
                    version: version.to_owned(),
                    key: decode_hmac_key(encoded.trim())?,
                });
            }
        } else {
            let version = std::env::var("EDGENEST_BROKER_ID_HMAC_KEY_VERSION").ok()?;
            let version = version.trim();
            if version.is_empty() || version.len() > 64 {
                return None;
            }
            keys.push(BrokerIdentifierKey {
                version: version.to_owned(),
                key: decode_hmac_key(&std::env::var("EDGENEST_BROKER_ID_HMAC_KEY_HEX").ok()?)?,
            });
        }
        if keys.is_empty() {
            return None;
        }
        let active_key_version = std::env::var("EDGENEST_BROKER_ID_HMAC_ACTIVE_KEY_VERSION")
            .ok()
            .or_else(|| (keys.len() == 1).then(|| keys[0].version.clone()))?;
        if !keys.iter().any(|key| key.version == active_key_version) {
            return None;
        }
        Some(Self {
            keys,
            active_key_version,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_key() -> Self {
        Self {
            keys: vec![BrokerIdentifierKey {
                key: [0x42; 32],
                version: "test-v1".into(),
            }],
            active_key_version: "test-v1".into(),
        }
    }

    #[cfg(test)]
    fn test_rotated_keyset() -> Self {
        Self {
            keys: vec![
                BrokerIdentifierKey {
                    key: [0x42; 32],
                    version: "test-v1".into(),
                },
                BrokerIdentifierKey {
                    key: [0x24; 32],
                    version: "test-v2".into(),
                },
            ],
            active_key_version: "test-v2".into(),
        }
    }

    fn lookup_tokens(
        &self,
        broker_id: &str,
        server_id: &str,
        normalized_identifier: &str,
    ) -> Option<Vec<(String, [u8; 32])>> {
        self.keys
            .iter()
            .map(|key| {
                let mut mac = HmacSha256::new_from_slice(&key.key).ok()?;
                mac.update(b"edgenest/external-broker-account/v1\0");
                mac.update(broker_id.as_bytes());
                mac.update(b"\0");
                mac.update(server_id.as_bytes());
                mac.update(b"\0");
                mac.update(normalized_identifier.as_bytes());
                Some((key.version.clone(), mac.finalize().into_bytes().into()))
            })
            .collect()
    }
}

fn decode_hmac_key(encoded: &str) -> Option<[u8; 32]> {
    let bytes = encoded.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut key = [0_u8; 32];
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return None;
    }
    for (index, pair) in pairs.iter().enumerate() {
        key[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(key)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterBrokerAccount {
    pub quant_account_id: String,
    pub broker_id: String,
    pub broker_server_id: String,
    pub external_account_identifier: String,
    pub reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingLifecycleRequest {
    pub expected_revision: i64,
    pub status: String,
    pub reason: String,
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}

fn not_found() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "not_found",
        "Broker account resource not found",
    )
}

#[allow(clippy::result_large_err)]
async fn path_principal(
    state: &AppState,
    headers: &HeaderMap,
    path_organization: &str,
    owner_required: bool,
) -> Result<organizations_api::TenantPrincipal, Response> {
    let principal = organizations_api::tenant_context(state, headers).await?;
    if principal.organization_id != path_organization {
        return Err(not_found());
    }
    if owner_required && !principal.role.may_manage_owner() {
        return Err(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Organization role does not permit this operation",
        ));
    }
    Ok(principal)
}

pub async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) =
        organizations_api::authorize(&state, &headers, organizations_api::Permission::Read).await
    {
        return response;
    }
    match state.store.broker_catalog().await {
        Ok(entries) => Json(
            serde_json::json!({"data":entries.into_iter().map(|entry| serde_json::json!({
            "brokerId":entry.broker_id,
            "name":entry.broker_name,
            "displayName":entry.broker_display_name,
            "serverId":entry.server_id,
            "serverDisplayName":entry.server_display_name,
            "providerType":entry.provider_type,
            "environment":entry.environment,
        })).collect::<Vec<_>>()}),
        )
        .into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Unable to load broker catalogue",
        ),
    }
}

pub async fn bindings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path_organization): Path<String>,
) -> Response {
    let principal = match path_principal(&state, &headers, &path_organization, false).await {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    match state
        .store
        .broker_bindings(&principal.organization_id)
        .await
    {
        Ok(rows) => Json(
            serde_json::json!({"data":rows.into_iter().map(|binding| serde_json::json!({
            "bindingId":binding.binding_id,
            "quantAccountId":binding.quant_account_id,
            "externalBrokerAccountId":binding.external_broker_account_id,
            "brokerId":binding.broker_id,
            "brokerDisplayName":binding.broker_display_name,
            "brokerServerId":binding.broker_server_id,
            "serverDisplayName":binding.server_display_name,
            "status":binding.binding_status,
            "verificationStatus":binding.verification_status,
            "revision":binding.revision,
        })).collect::<Vec<_>>()}),
        )
        .into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Unable to load broker bindings",
        ),
    }
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(path_organization): Path<String>,
    Json(input): Json<RegisterBrokerAccount>,
) -> Response {
    let RegisterBrokerAccount {
        quant_account_id,
        broker_id,
        broker_server_id,
        external_account_identifier,
        reason,
    } = input;
    let raw_identifier = Zeroizing::new(external_account_identifier);
    let principal = match path_principal(&state, &headers, &path_organization, true).await {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let Some(protector) = state.broker_identity_hmac.as_ref() else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_protection_unavailable",
            "Broker account registration is unavailable",
        );
    };
    if reason.trim().is_empty() || reason.len() > 1000 {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Registration reason is invalid",
        );
    }
    let broker = match domain::normalize_broker_id(&broker_id) {
        Ok(value) if value.canonical == broker_id => value.canonical,
        _ => return not_found(),
    };
    let normalized_identifier = match domain::normalize_external_account_identifier(&raw_identifier)
    {
        Ok(value) => value,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Broker account registration is invalid",
            )
        }
    };
    let Some(tokens) = protector.lookup_tokens(&broker, &broker_server_id, &normalized_identifier)
    else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_protection_unavailable",
            "Broker account registration is unavailable",
        );
    };
    if !tokens
        .iter()
        .any(|(version, _)| version == &protector.active_key_version)
    {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_protection_unavailable",
            "Broker account registration is unavailable",
        );
    }
    let normalized_identifier = Zeroizing::new(normalized_identifier);
    if reason.contains(raw_identifier.as_str()) || reason.contains(normalized_identifier.as_str()) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Registration reason must not include the broker account identifier",
        );
    }
    let account_id = format!("ext-{}", uuid::Uuid::new_v4());
    let binding_id = format!("bind-{}", uuid::Uuid::new_v4());
    match state
        .store
        .register_pending_broker_binding(
            &principal.organization_id,
            &principal.user_id,
            &quant_account_id,
            &broker,
            &broker_server_id,
            &account_id,
            &binding_id,
            domain::EXTERNAL_ACCOUNT_IDENTIFIER_NORMALIZATION_VERSION,
            &protector.active_key_version,
            &tokens,
            reason.trim(),
        )
        .await
    {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({"data":{
                "bindingId":binding_id,
                "status":"PENDING_EVIDENCE",
                "verificationStatus":"PENDING_EVIDENCE"
            }})),
        )
            .into_response(),
        Err(persistence::PersistenceError::NotFound(_)) => not_found(),
        Err(persistence::PersistenceError::Db(sqlx::Error::Database(db_error)))
            if db_error.code().as_deref() == Some("23505") =>
        {
            error(
                StatusCode::CONFLICT,
                "registration_unavailable",
                "Broker account registration is unavailable",
            )
        }
        Err(persistence::PersistenceError::Other(message))
            if message == "broker account registration conflict" =>
        {
            error(
                StatusCode::CONFLICT,
                "registration_unavailable",
                "Broker account registration is unavailable",
            )
        }
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Unable to register broker account",
        ),
    }
}

pub async fn transition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((path_organization, binding_id)): Path<(String, String)>,
    Json(input): Json<BindingLifecycleRequest>,
) -> Response {
    let principal = match path_principal(&state, &headers, &path_organization, true).await {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    if input.expected_revision < 1 || input.reason.trim().is_empty() || input.reason.len() > 1000 {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Broker binding update is invalid",
        );
    }
    let transition = match input.status.as_str() {
        "SUSPENDED" => persistence::BrokerBindingTransition::Suspended,
        "REVOKED" => persistence::BrokerBindingTransition::Revoked,
        // Only platform-authorized approval may set VERIFIED. No such authority
        // exists in this service, so ordinary organization roles cannot do so.
        _ => {
            return error(
                StatusCode::FORBIDDEN,
                "forbidden",
                "This broker binding transition is unavailable",
            )
        }
    };
    match state.store.transition_broker_binding(
        &principal.organization_id,
        &principal.user_id,
        &binding_id,
        input.expected_revision,
        transition,
        input.reason.trim(),
    ).await {
        Ok(revision) => Json(serde_json::json!({"data":{"bindingId":binding_id,"status":input.status,"revision":revision}})).into_response(),
        Err(persistence::PersistenceError::NotFound(_)) => not_found(),
        Err(persistence::PersistenceError::Other(_)) => error(StatusCode::CONFLICT, "revision_conflict", "Broker binding revision is stale or terminal"),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Unable to update broker binding"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_lookup_hmac_is_scoped_and_deterministic() {
        let protector = BrokerIdentityHmac::test_key();
        let first = protector
            .lookup_tokens("broker-a", "server-a", "00123")
            .unwrap();
        assert_eq!(
            first,
            protector
                .lookup_tokens("broker-a", "server-a", "00123")
                .unwrap()
        );
        assert_ne!(
            first,
            protector
                .lookup_tokens("broker-b", "server-a", "00123")
                .unwrap()
        );
        assert_ne!(
            first,
            protector
                .lookup_tokens("broker-a", "server-b", "00123")
                .unwrap()
        );
        assert_ne!(
            first,
            protector
                .lookup_tokens("broker-a", "server-a", "123")
                .unwrap()
        );
    }

    #[test]
    fn overlapping_key_rotation_retains_old_duplicate_detection_token() {
        let original = BrokerIdentityHmac::test_key()
            .lookup_tokens("broker-a", "server-a", "00123")
            .unwrap();
        let rotated = BrokerIdentityHmac::test_rotated_keyset()
            .lookup_tokens("broker-a", "server-a", "00123")
            .unwrap();
        assert_eq!(rotated.len(), 2);
        assert_eq!(original[0], rotated[0]);
        assert_eq!(rotated[1].0, "test-v2");
    }

    #[test]
    fn key_hex_parser_rejects_malformed_values() {
        assert_eq!(hex_nibble(b'f'), Some(15));
        assert_eq!(hex_nibble(b'G'), None);
    }
}

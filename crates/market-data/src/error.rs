use serde::Serialize;
use thiserror::Error;

use crate::model::{DataClass, ProviderId};

/// Structured, provider-agnostic errors. Provider-specific failures are mapped
/// to one of these so the frontend never sees an exchange's own error format.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MarketDataError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("unknown instrument '{0}'")]
    UnknownInstrument(String),
    #[error("provider '{provider}' does not support this request: {reason}")]
    Unsupported {
        provider: ProviderId,
        reason: String,
    },
    #[error("provider '{provider}' is disabled: {reason}")]
    ProviderDisabled {
        provider: ProviderId,
        reason: String,
    },
    #[error("provider '{provider}' capability is not implemented: {reason}")]
    NotImplemented {
        provider: ProviderId,
        reason: String,
    },
    #[error("provider '{provider}' is unavailable: {reason}")]
    ProviderUnavailable {
        provider: ProviderId,
        reason: String,
    },
    #[error("provider '{provider}' rate limited the request")]
    RateLimited {
        provider: ProviderId,
        retry_after_seconds: Option<u64>,
    },
    #[error("provider '{provider}' did not answer in time")]
    Timeout { provider: ProviderId },
    #[error("provider '{provider}' returned invalid data: {reason}")]
    InvalidProviderData {
        provider: ProviderId,
        reason: String,
    },
    #[error("provider '{provider}' upstream error: {message}")]
    Upstream {
        provider: ProviderId,
        status: Option<u16>,
        message: String,
    },
}

impl MarketDataError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::UnknownInstrument(_) => "unknown_instrument",
            Self::Unsupported { .. } => "unsupported_by_provider",
            Self::ProviderDisabled { .. } => "provider_disabled",
            Self::NotImplemented { .. } => "capability_not_implemented",
            Self::ProviderUnavailable { .. } => "provider_unavailable",
            Self::RateLimited { .. } => "rate_limited",
            Self::Timeout { .. } => "provider_timeout",
            Self::InvalidProviderData { .. } => "invalid_provider_data",
            Self::Upstream { .. } => "upstream_error",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::InvalidRequest(_) => 400,
            Self::UnknownInstrument(_) => 404,
            Self::Unsupported { .. } => 422,
            Self::ProviderDisabled { .. } | Self::ProviderUnavailable { .. } => 503,
            Self::NotImplemented { .. } => 501,
            Self::RateLimited { .. } => 429,
            Self::Timeout { .. } => 504,
            Self::InvalidProviderData { .. } | Self::Upstream { .. } => 502,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderUnavailable { .. }
                | Self::RateLimited { .. }
                | Self::Timeout { .. }
                | Self::Upstream { .. }
        )
    }

    pub fn provider(&self) -> Option<ProviderId> {
        match self {
            Self::Unsupported { provider, .. }
            | Self::ProviderDisabled { provider, .. }
            | Self::NotImplemented { provider, .. }
            | Self::ProviderUnavailable { provider, .. }
            | Self::RateLimited { provider, .. }
            | Self::Timeout { provider }
            | Self::InvalidProviderData { provider, .. }
            | Self::Upstream { provider, .. } => Some(*provider),
            Self::InvalidRequest(_) | Self::UnknownInstrument(_) => None,
        }
    }

    pub fn body(&self) -> ErrorEnvelope {
        ErrorEnvelope {
            error: ErrorBody {
                code: self.code(),
                message: self.to_string(),
                classification: DataClass::Unavailable,
                provider: self.provider(),
                retryable: self.retryable(),
                retry_after_seconds: match self {
                    Self::RateLimited {
                        retry_after_seconds,
                        ..
                    } => *retry_after_seconds,
                    _ => None,
                },
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    /// Always `unavailable`: an error never carries demo or live data.
    pub classification: DataClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_and_retryability() {
        let rl = MarketDataError::RateLimited {
            provider: ProviderId::BinancePublic,
            retry_after_seconds: Some(7),
        };
        assert_eq!(
            (rl.http_status(), rl.code(), rl.retryable()),
            (429, "rate_limited", true)
        );
        let body = serde_json::to_value(rl.body()).unwrap();
        assert_eq!(body["error"]["retry_after_seconds"], 7);
        assert_eq!(body["error"]["classification"], "unavailable");

        let bad = MarketDataError::InvalidRequest("x".into());
        assert_eq!((bad.http_status(), bad.retryable()), (400, false));
        let ni = MarketDataError::NotImplemented {
            provider: ProviderId::Mt5Bridge,
            reason: "r".into(),
        };
        assert_eq!((ni.http_status(), ni.retryable()), (501, false));
    }
}

use std::net::SocketAddr;
use std::time::Duration;

/// Runtime configuration, read from the environment. Invalid values fail
/// startup instead of silently falling back.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub binance_enabled: bool,
    pub binance_base_url: String,
    pub request_timeout: Duration,
    pub cors_origins: Vec<String>,
    pub mt5: Mt5Config,
}

/// Optional MT5 latest-quote settings. Absent or partial configuration does
/// not fail startup; the endpoint answers 503 instead.
#[derive(Clone)]
pub struct Mt5Config {
    pub database_url: Option<String>,
    pub account_id: Option<String>,
    pub terminal_id: Option<String>,
    pub stale_after_seconds: u64,
}

impl Mt5Config {
    pub fn is_fully_configured(&self) -> bool {
        self.database_url.is_some() && self.account_id.is_some() && self.terminal_id.is_some()
    }
}

// Manual Debug so the DSN and scope identifiers can never reach logs.
impl std::fmt::Debug for Mt5Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mt5Config")
            .field(
                "database_url",
                &self.database_url.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "account_id",
                &self.account_id.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "terminal_id",
                &self.terminal_id.as_ref().map(|_| "<redacted>"),
            )
            .field("stale_after_seconds", &self.stale_after_seconds)
            .finish()
    }
}

impl Config {
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let bind = get("MARKET_DATA_BIND")
            .unwrap_or_else(|| "127.0.0.1:8787".into())
            .parse::<SocketAddr>()
            .map_err(|e| format!("MARKET_DATA_BIND: {e}"))?;

        let binance_enabled = match get("MARKET_DATA_BINANCE_ENABLED").as_deref() {
            None | Some("") | Some("false") | Some("0") => false,
            Some("true") | Some("1") => true,
            Some(_) => return Err("MARKET_DATA_BINANCE_ENABLED must be true or false".into()),
        };

        let binance_base_url =
            get("MARKET_DATA_BINANCE_BASE_URL").unwrap_or_else(|| "https://api.binance.com".into());
        if !binance_base_url.starts_with("http://") && !binance_base_url.starts_with("https://") {
            return Err("MARKET_DATA_BINANCE_BASE_URL must be an http(s) URL".into());
        }

        let timeout_ms = match get("MARKET_DATA_REQUEST_TIMEOUT_MS") {
            None => 5000,
            Some(v) => v
                .parse::<u64>()
                .ok()
                .filter(|n| (100..=30_000).contains(n))
                .ok_or("MARKET_DATA_REQUEST_TIMEOUT_MS must be 100..=30000")?,
        };

        let cors_origins = get("MARKET_DATA_CORS_ORIGINS")
            .unwrap_or_else(|| "http://localhost:3000,http://localhost:3107".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let non_empty = |key: &str| {
            get(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let database_url = non_empty("MARKET_DATA_DATABASE_URL");
        if let Some(url) = &database_url {
            if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
                return Err("MARKET_DATA_DATABASE_URL must be a postgres:// URL".into());
            }
            crate::mt5_latest::connect_options(url)
                .map_err(|e| format!("MARKET_DATA_DATABASE_URL: {e}"))?;
        }
        let stale_after_seconds = match non_empty("MARKET_DATA_MT5_STALE_AFTER_SECONDS") {
            None => 60,
            Some(v) => v
                .parse::<u64>()
                .ok()
                .filter(|n| (1..=3600).contains(n))
                .ok_or("MARKET_DATA_MT5_STALE_AFTER_SECONDS must be 1..=3600")?,
        };
        let mt5 = Mt5Config {
            database_url,
            account_id: non_empty("MARKET_DATA_MT5_ACCOUNT_ID"),
            terminal_id: non_empty("MARKET_DATA_MT5_TERMINAL_ID"),
            stale_after_seconds,
        };

        Ok(Self {
            bind,
            binance_enabled,
            binance_base_url,
            request_timeout: Duration::from_millis(timeout_ms),
            cors_origins,
            mt5,
        })
    }

    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, String> {
        Config::from_lookup(|k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn defaults_are_local_and_binance_disabled() {
        let c = cfg(&[]).unwrap();
        assert_eq!(c.bind.to_string(), "127.0.0.1:8787");
        assert!(!c.binance_enabled);
        assert_eq!(c.request_timeout, Duration::from_millis(5000));
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(cfg(&[("MARKET_DATA_BINANCE_ENABLED", "yes")]).is_err());
        assert!(cfg(&[("MARKET_DATA_BIND", "nope")]).is_err());
        assert!(cfg(&[("MARKET_DATA_REQUEST_TIMEOUT_MS", "5")]).is_err());
        assert!(cfg(&[("MARKET_DATA_BINANCE_BASE_URL", "ftp://x")]).is_err());
    }

    #[test]
    fn mt5_defaults_to_unconfigured_with_60s_threshold() {
        let c = cfg(&[]).unwrap();
        assert!(!c.mt5.is_fully_configured());
        assert_eq!(c.mt5.stale_after_seconds, 60);
    }

    #[test]
    fn mt5_partial_config_is_not_fully_configured() {
        let c = cfg(&[("MARKET_DATA_DATABASE_URL", "postgres://localhost/d")]).unwrap();
        assert!(!c.mt5.is_fully_configured());
        let c = cfg(&[
            ("MARKET_DATA_DATABASE_URL", "postgres://localhost/d"),
            ("MARKET_DATA_MT5_ACCOUNT_ID", "a"),
            ("MARKET_DATA_MT5_TERMINAL_ID", "  "),
        ])
        .unwrap();
        assert!(!c.mt5.is_fully_configured());
    }

    #[test]
    fn mt5_rejects_bad_values_without_echoing_them() {
        let err = cfg(&[
            ("MARKET_DATA_DATABASE_URL", "mysql://user:secret@localhost/d"),
        ])
        .unwrap_err();
        assert!(!err.contains("secret"));
        assert!(cfg(&[("MARKET_DATA_MT5_STALE_AFTER_SECONDS", "0")]).is_err());
        assert!(cfg(&[("MARKET_DATA_MT5_STALE_AFTER_SECONDS", "x")]).is_err());
        assert!(cfg(&[("MARKET_DATA_MT5_STALE_AFTER_SECONDS", "3601")]).is_err());
    }

    #[test]
    fn mt5_rejects_plaintext_remote_database_without_echoing_it() {
        let err = cfg(&[("MARKET_DATA_DATABASE_URL", "postgres://u:pw@db.example.com/d")])
            .unwrap_err();
        assert!(!err.contains("pw") && !err.contains("example.com"));
        assert!(cfg(&[(
            "MARKET_DATA_DATABASE_URL",
            "postgres://u:pw@db.example.com/d?sslmode=verify-full"
        )])
        .is_ok());
    }

    #[test]
    fn mt5_debug_redacts_secrets() {
        let c = cfg(&[
            (
                "MARKET_DATA_DATABASE_URL",
                "postgres://user:pw123@localhost/d",
            ),
            ("MARKET_DATA_MT5_ACCOUNT_ID", "acct-777"),
            ("MARKET_DATA_MT5_TERMINAL_ID", "term-888"),
        ])
        .unwrap();
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("pw123") && !dbg.contains("acct-777") && !dbg.contains("term-888"));
    }

    #[test]
    fn enables_binance_only_when_explicit() {
        assert!(
            cfg(&[("MARKET_DATA_BINANCE_ENABLED", "true")])
                .unwrap()
                .binance_enabled
        );
    }
}

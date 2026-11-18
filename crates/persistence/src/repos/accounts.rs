use chrono::Utc;
use domain::{
    Account, AccountEnvironment, AccountId, AccountState, AccountStatus, AccountType,
    EnvironmentVerification,
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::{PersistenceError, PersistenceResult};

fn account_type_str(t: AccountType) -> &'static str {
    match t {
        AccountType::Simulated => "simulated",
        AccountType::Prop => "prop",
        AccountType::Personal => "personal",
        AccountType::Cex => "cex",
        AccountType::Vault => "vault",
    }
}

fn status_str(s: AccountStatus) -> &'static str {
    match s {
        AccountStatus::Active => "active",
        AccountStatus::Suspended => "suspended",
        AccountStatus::Halted => "halted",
        AccountStatus::Closed => "closed",
    }
}

fn environment_str(environment: AccountEnvironment) -> &'static str {
    match environment {
        AccountEnvironment::Paper => "PAPER",
        AccountEnvironment::Shadow => "SHADOW",
        AccountEnvironment::Live => "LIVE",
    }
}

pub async fn upsert_account(
    pool: &PgPool,
    account: &Account,
    starting_capital: Decimal,
    label: Option<&str>,
    state: &AccountState,
) -> PersistenceResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO accounts (id, account_type, status, venue_id, starting_capital, label, created_at, environment)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (id) DO UPDATE SET
            account_type = EXCLUDED.account_type,
            status = EXCLUDED.status,
            venue_id = EXCLUDED.venue_id,
            starting_capital = EXCLUDED.starting_capital,
            label = EXCLUDED.label
        "#,
    )
    .bind(account.id.as_str())
    .bind(account_type_str(account.account_type))
    .bind(status_str(account.status))
    .bind(account.venue_id.as_str())
    .bind(starting_capital)
    .bind(label)
    .bind(account.created_at)
    .bind(account.environment.map(environment_str))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO account_states (
            account_id, balance, equity, used_margin, open_position_count,
            daily_realized_pnl, peak_equity, kill_switch, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
        ON CONFLICT (account_id) DO UPDATE SET
            balance = EXCLUDED.balance,
            equity = EXCLUDED.equity,
            used_margin = EXCLUDED.used_margin,
            open_position_count = EXCLUDED.open_position_count,
            daily_realized_pnl = EXCLUDED.daily_realized_pnl,
            peak_equity = EXCLUDED.peak_equity,
            kill_switch = EXCLUDED.kill_switch,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(state.account_id.as_str())
    .bind(state.balance)
    .bind(state.equity)
    .bind(state.used_margin)
    .bind(state.open_position_count as i32)
    .bind(state.daily_realized_pnl)
    .bind(state.peak_equity)
    .bind(state.kill_switch)
    .bind(state.updated_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn load_state(pool: &PgPool, account_id: &AccountId) -> PersistenceResult<AccountState> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            Decimal,
            Decimal,
            Decimal,
            i32,
            Decimal,
            Decimal,
            bool,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        r#"
        SELECT account_id, balance, equity, used_margin, open_position_count,
               daily_realized_pnl, peak_equity, kill_switch, updated_at
        FROM account_states WHERE account_id = $1
        "#,
    )
    .bind(account_id.as_str())
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PersistenceError::NotFound(account_id.as_str().to_string()))?;

    Ok(AccountState {
        account_id: AccountId::new(row.0)?,
        balance: row.1,
        equity: row.2,
        used_margin: row.3,
        open_position_count: row.4 as u32,
        daily_realized_pnl: row.5,
        peak_equity: row.6,
        kill_switch: row.7,
        updated_at: row.8,
    })
}

pub async fn load_account(pool: &PgPool, account_id: &AccountId) -> PersistenceResult<Account> {
    let row = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, i64, Option<chrono::DateTime<chrono::Utc>>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, account_type, status, venue_id, environment, environment_verification, environment_revision, environment_verified_at, environment_verified_by, environment_verification_reason, created_at FROM accounts WHERE id=$1",
    )
    .bind(account_id.as_str())
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| PersistenceError::NotFound(account_id.as_str().into()))?;
    let account_type = match row.1.as_str() {
        "simulated" => AccountType::Simulated,
        "prop" => AccountType::Prop,
        "personal" => AccountType::Personal,
        "cex" => AccountType::Cex,
        "vault" => AccountType::Vault,
        value => {
            return Err(PersistenceError::Other(format!(
                "unknown account type {value}"
            )));
        }
    };
    let status = match row.2.as_str() {
        "active" => AccountStatus::Active,
        "suspended" => AccountStatus::Suspended,
        "halted" => AccountStatus::Halted,
        "closed" => AccountStatus::Closed,
        value => {
            return Err(PersistenceError::Other(format!(
                "unknown account status {value}"
            )));
        }
    };
    let environment = match row.4.as_deref() {
        Some("PAPER") => Some(AccountEnvironment::Paper),
        Some("SHADOW") => Some(AccountEnvironment::Shadow),
        Some("LIVE") => Some(AccountEnvironment::Live),
        None => None,
        Some(value) => {
            return Err(PersistenceError::Other(format!(
                "unknown account environment {value}"
            )));
        }
    };
    let verification = match row.5.as_str() {
        "UNCLASSIFIED" => EnvironmentVerification::Unclassified,
        "UNVERIFIED" => EnvironmentVerification::Unverified,
        "UNVERIFIED_LEGACY" => EnvironmentVerification::UnverifiedLegacy,
        "VERIFIED" => EnvironmentVerification::Verified {
            revision: u64::try_from(row.6)
                .map_err(|_| PersistenceError::Other("negative environment revision".into()))?,
            verified_at: row.7.ok_or_else(|| {
                PersistenceError::Other("verified account missing verification timestamp".into())
            })?,
        },
        value => {
            return Err(PersistenceError::Other(format!(
                "unknown environment verification state {value}"
            )));
        }
    };
    Ok(Account {
        id: AccountId::new(row.0)?,
        account_type,
        status,
        venue_id: domain::VenueId::new(row.3)?,
        environment,
        environment_revision: u64::try_from(row.6)
            .map_err(|_| PersistenceError::Other("negative environment revision".into()))?,
        environment_verification: verification,
        environment_verified_by: row.8,
        environment_verification_reason: row.9,
        created_at: row.10,
    })
}

/// Explicitly verifies and revises an account's environment under a row lock.
pub async fn verify_environment(
    pool: &PgPool,
    account_id: &AccountId,
    environment: AccountEnvironment,
    verifier_id: &str,
    expected_revision: u64,
    reason: Option<&str>,
) -> PersistenceResult<u64> {
    if verifier_id.trim().is_empty() {
        return Err(PersistenceError::Other(
            "verifier identity is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let current: Option<i64> =
        sqlx::query_scalar("SELECT environment_revision FROM accounts WHERE id=$1 FOR UPDATE")
            .bind(account_id.as_str())
            .fetch_optional(&mut *tx)
            .await?;
    let current = current.ok_or_else(|| PersistenceError::NotFound(account_id.as_str().into()))?;
    if current < 0 || current as u64 != expected_revision {
        return Err(PersistenceError::Other(format!(
            "environment revision conflict: expected {expected_revision}, found {current}"
        )));
    }
    let next = current
        .checked_add(1)
        .ok_or_else(|| PersistenceError::Other("environment revision overflow".into()))?;
    sqlx::query(
        "UPDATE accounts SET environment=$2, environment_verification='VERIFIED', environment_revision=$3, environment_verified_at=$4, environment_verified_by=$5, environment_verification_reason=$6 WHERE id=$1",
    )
    .bind(account_id.as_str())
    .bind(environment_str(environment))
    .bind(next)
    .bind(Utc::now())
    .bind(verifier_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(next as u64)
}

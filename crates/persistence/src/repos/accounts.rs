use domain::{Account, AccountId, AccountState, AccountStatus, AccountType};
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
        INSERT INTO accounts (id, account_type, status, venue_id, starting_capital, label, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
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

//! Idempotent user-scoped journal history import endpoint.
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Row;

use crate::{
    http::{trade_from, TradeWrite},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub journal_account_id: String,
    pub source_system: String,
    pub trades: Vec<TradeWrite>,
}

pub async fn import_trades(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ImportRequest>,
) -> Response {
    let claims = match state.auth.authenticate(&headers) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let source_system = input.source_system.trim();
    if source_system.is_empty()
        || source_system.len() > 100
        || input.trades.is_empty()
        || input.trades.len() > 10_000
    {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":{"code":"invalid_import","message":"source_system is required and import must contain 1–10000 trades"}}))).into_response();
    }
    let account_exists=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM journal_accounts WHERE owner_id=$1 AND id=$2 AND archived=FALSE)")
        .bind(&claims.sub).bind(&input.journal_account_id).fetch_one(state.store.pool()).await.unwrap_or(false);
    if !account_exists {
        return (StatusCode::NOT_FOUND,Json(serde_json::json!({"error":{"code":"account_not_found","message":"Journal account not found"}}))).into_response();
    }

    let batch_id = uuid::Uuid::new_v4().to_string();
    let mut tx = match state.store.pool().begin().await {
        Ok(tx) => tx,
        Err(_) => return internal(),
    };
    if sqlx::query("INSERT INTO journal_import_batches(id,owner_id,journal_account_id,source_system,status,source_record_count) VALUES($1,$2,$3,$4,'running',$5)")
        .bind(&batch_id).bind(&claims.sub).bind(&input.journal_account_id).bind(source_system).bind(input.trades.len() as i64)
        .execute(&mut *tx).await.is_err() { return internal(); }

    let mut imported = 0i64;
    let mut skipped = 0i64;
    for write in input.trades {
        let external_id=match write.source_trade_id.as_deref().map(str::trim).filter(|s|!s.is_empty()) {
            Some(id)=>id.to_owned(),
            None=>return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":{"code":"source_id_required","message":"Every imported trade must contain source_trade_id for idempotency"}}))).into_response(),
        };
        let mut write = write;
        let checklist_payload = write.checklist.clone();
        let source_payload = write.source_payload.clone();
        write.journal_account_id = input.journal_account_id.clone();
        write.source_system = Some(source_system.to_owned());
        write.source_trade_id = Some(external_id.clone());
        write.source = domain::JournalTradeSource::LegacyImport;
        let trade = trade_from(claims.sub.clone(), uuid::Uuid::new_v4().to_string(), write);
        if let Some(setup_id) = trade.setup_id.as_deref() {
            let owned = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM journal_setups WHERE owner_id=$1 AND id=$2 AND archived=FALSE)",
            )
            .bind(&claims.sub)
            .bind(setup_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);
            if !owned {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error":{"code":"setup_not_found","message":"Journal setup not found"}})),
                )
                    .into_response();
            }
        }
        if let Err(e) = trade.validate() {
            return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":{"code":"invalid_trade","message":e.to_string(),"source_trade_id":external_id}}))).into_response();
        }
        let direction = match trade.direction {
            domain::JournalDirection::Long => "long",
            domain::JournalDirection::Short => "short",
        };
        let status = match trade.status {
            domain::JournalTradeStatus::Open => "open",
            domain::JournalTradeStatus::Closed => "closed",
            domain::JournalTradeStatus::Cancelled => "cancelled",
        };
        let inserted=sqlx::query(r#"
            INSERT INTO journal_trades(id,owner_id,journal_account_id,setup_id,symbol,direction,status,source,
              source_system,source_trade_id,opened_at,closed_at,quantity,entry_price,exit_price,stop_loss,take_profit,
              gross_pnl,commission,swap,other_fees,net_pnl,pnl_currency,risk_amount,r_multiple,session,timeframe,timezone,mfe_r,mae_r,notes,tags,setup_valid,rules_followed,emotional_trade,mistake,mistake_notes,discipline_score,acknowledged_warnings,checklist,source_payload)
            VALUES($1,$2,$3,$4,$5,$6,$7,'legacy_import',$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33,$34,$35,$36,$37,$38,$39,$40)
            ON CONFLICT (journal_account_id,source_system,source_trade_id)
              WHERE source_system IS NOT NULL AND source_trade_id IS NOT NULL DO NOTHING
            RETURNING id
        "#)
        .bind(&trade.id).bind(&trade.owner_id).bind(&trade.journal_account_id).bind(&trade.setup_id).bind(&trade.symbol)
        .bind(direction).bind(status).bind(source_system).bind(&external_id).bind(trade.opened_at).bind(trade.closed_at)
        .bind(trade.quantity).bind(trade.entry_price).bind(trade.exit_price).bind(trade.stop_loss).bind(trade.take_profit)
        .bind(trade.gross_pnl).bind(trade.commission).bind(trade.swap).bind(trade.other_fees).bind(trade.net_pnl)
        .bind(&trade.pnl_currency).bind(trade.risk_amount).bind(trade.r_multiple).bind(&trade.session).bind(&trade.timeframe).bind(&trade.timezone).bind(trade.mfe_r).bind(trade.mae_r).bind(&trade.notes).bind(sqlx::types::Json(&trade.tags)).bind(trade.setup_valid).bind(trade.rules_followed).bind(trade.emotional_trade).bind(trade.mistake).bind(&trade.mistake_notes).bind(trade.discipline_score).bind(sqlx::types::Json(&trade.acknowledged_warnings))
        .bind(sqlx::types::Json(&checklist_payload)).bind(sqlx::types::Json(&source_payload))
        .fetch_optional(&mut *tx).await;
        match inserted {
            Ok(Some(row)) => {
                let trade_id: String = row.get("id");
                let payload =
                    serde_json::to_value(&trade).unwrap_or_else(|_| serde_json::json!({}));
                if sqlx::query("INSERT INTO journal_trade_source_records(id,owner_id,journal_account_id,trade_id,import_batch_id,source_system,source_record_id,source_position_id,source_deal_id,raw_symbol,source_opened_at,source_closed_at,source_payload) VALUES($1,$2,$3,$4,$5,$6,$7,NULL,NULL,$8,$9,$10,$11)")
                    .bind(uuid::Uuid::new_v4().to_string()).bind(&claims.sub).bind(&input.journal_account_id).bind(&trade_id)
                    .bind(&batch_id).bind(source_system).bind(&external_id).bind(&trade.symbol).bind(trade.opened_at).bind(trade.closed_at).bind(payload)
                    .execute(&mut *tx).await.is_err() { return internal(); }
                imported += 1;
            }
            Ok(None) => skipped += 1,
            Err(_) => return internal(),
        }
    }
    let completed = Utc::now();
    if sqlx::query("UPDATE journal_import_batches SET status='completed',imported_count=$2,skipped_count=$3,failed_count=0,completed_at=$4,reconciliation=$5 WHERE owner_id=$1 AND id=$6")
        .bind(&claims.sub).bind(imported).bind(skipped).bind(completed)
        .bind(serde_json::json!({"source_count":imported+skipped,"inserted":imported,"duplicates":skipped}))
        .bind(&batch_id).execute(&mut *tx).await.is_err() { return internal(); }
    if tx.commit().await.is_err() {
        return internal();
    }
    (StatusCode::CREATED,Json(serde_json::json!({"data":{"batch_id":batch_id,"source_system":source_system,"source_count":imported+skipped,"imported_count":imported,"skipped_count":skipped,"failed_count":0,"status":"completed"}}))).into_response()
}

fn internal() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":{"code":"internal_error","message":"Journal import failed"}}))).into_response()
}

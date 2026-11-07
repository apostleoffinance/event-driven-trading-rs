//! Owner-scoped Journal psychology notes and rating persistence.
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use sqlx::Row;

use crate::{AppState};

#[derive(Debug, Deserialize)]
pub struct PsychologyWrite {
    pub emotion_before: Option<String>,
    pub emotion_during: Option<String>,
    pub emotion_after: Option<String>,
    pub confidence: Option<i16>,
    pub fear: Option<i16>,
    pub fomo: Option<i16>,
    pub frustration: Option<i16>,
    pub revenge_intensity: Option<i16>,
    pub boredom: Option<i16>,
    pub notes: Option<String>,
}

fn invalid(message: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({
        "error":{"code":"invalid_psychology","message":message}
    }))).into_response()
}
fn internal() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
        "error":{"code":"internal_error","message":"Unable to save Journal psychology"}
    }))).into_response()
}
fn ratings_valid(input: &PsychologyWrite) -> bool {
    [input.confidence,input.fear,input.fomo,input.frustration,input.revenge_intensity,input.boredom]
        .into_iter().flatten().all(|v|(0..=10).contains(&v))
    && [input.emotion_before.as_deref(),input.emotion_during.as_deref(),input.emotion_after.as_deref()]
        .into_iter().flatten().all(|s|s.len()<=100)
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
) -> Response {
    let claims=match state.auth.authenticate(&headers) {
        Ok(c)=>c,
        Err(e)=>return e.into_response(),
    };
    let exists=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM journal_trades WHERE owner_id=$1 AND id=$2)")
        .bind(&claims.sub).bind(&trade_id).fetch_one(state.store.pool()).await.unwrap_or(false);
    if !exists {
        return (StatusCode::NOT_FOUND,Json(serde_json::json!({"error":{"code":"not_found","message":"Journal trade not found"}}))).into_response();
    }
    match sqlx::query("SELECT emotion_before,emotion_during,emotion_after,confidence,fear,fomo,frustration,revenge_intensity,boredom,notes FROM journal_trade_psychology WHERE owner_id=$1 AND trade_id=$2")
        .bind(&claims.sub).bind(&trade_id).fetch_optional(state.store.pool()).await {
        Ok(Some(row))=>Json(serde_json::json!({"data":{
            "emotion_before":row.get::<Option<String>,_>("emotion_before"),
            "emotion_during":row.get::<Option<String>,_>("emotion_during"),
            "emotion_after":row.get::<Option<String>,_>("emotion_after"),
            "confidence":row.get::<Option<i16>,_>("confidence"),
            "fear":row.get::<Option<i16>,_>("fear"),
            "fomo":row.get::<Option<i16>,_>("fomo"),
            "frustration":row.get::<Option<i16>,_>("frustration"),
            "revenge_intensity":row.get::<Option<i16>,_>("revenge_intensity"),
            "boredom":row.get::<Option<i16>,_>("boredom"),
            "notes":row.get::<Option<String>,_>("notes")
        }})).into_response(),
        Ok(None)=>Json(serde_json::json!({"data":null})).into_response(),
        Err(_)=>internal(),
    }
}

pub async fn upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(trade_id): Path<String>,
    Json(input): Json<PsychologyWrite>,
) -> Response {
    let claims=match state.auth.authenticate(&headers) {
        Ok(c)=>c,
        Err(e)=>return e.into_response(),
    };
    if !ratings_valid(&input) {
        return invalid("Psychology ratings must be between 0 and 10; emotion labels are limited to 100 characters.");
    }
    let exists=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM journal_trades WHERE owner_id=$1 AND id=$2)")
        .bind(&claims.sub).bind(&trade_id).fetch_one(state.store.pool()).await.unwrap_or(false);
    if !exists {
        return (StatusCode::NOT_FOUND,Json(serde_json::json!({"error":{"code":"not_found","message":"Journal trade not found"}}))).into_response();
    }
    match sqlx::query(r#"
        INSERT INTO journal_trade_psychology(
            trade_id,owner_id,emotion_before,emotion_during,emotion_after,confidence,fear,fomo,frustration,revenge_intensity,boredom,notes
        ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        ON CONFLICT(trade_id) DO UPDATE SET
            emotion_before=EXCLUDED.emotion_before,emotion_during=EXCLUDED.emotion_during,emotion_after=EXCLUDED.emotion_after,
            confidence=EXCLUDED.confidence,fear=EXCLUDED.fear,fomo=EXCLUDED.fomo,frustration=EXCLUDED.frustration,
            revenge_intensity=EXCLUDED.revenge_intensity,boredom=EXCLUDED.boredom,notes=EXCLUDED.notes,updated_at=NOW()
        WHERE journal_trade_psychology.owner_id=EXCLUDED.owner_id
    "#)
        .bind(&trade_id).bind(&claims.sub).bind(&input.emotion_before).bind(&input.emotion_during).bind(&input.emotion_after)
        .bind(input.confidence).bind(input.fear).bind(input.fomo).bind(input.frustration).bind(input.revenge_intensity)
        .bind(input.boredom).bind(&input.notes).execute(state.store.pool()).await {
        Ok(result) if result.rows_affected()==1=>(StatusCode::OK,Json(serde_json::json!({"data":{"trade_id":trade_id,"saved":true}}))).into_response(),
        Ok(_)=>(StatusCode::NOT_FOUND,Json(serde_json::json!({"error":{"code":"not_found","message":"Journal trade not found"}}))).into_response(),
        Err(_)=>internal(),
    }
}

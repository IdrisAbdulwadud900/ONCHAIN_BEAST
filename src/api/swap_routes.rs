/// Swap Query API Routes
/// Provides endpoints for querying DEX swap data
use actix_web::{get, web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::errors::BeastResult;
use crate::storage::DatabaseManager;

#[derive(Debug, Serialize)]
pub struct SwapResponse {
    pub signature: String,
    pub event_index: i32,
    pub slot: i64,
    pub block_time: i64,
    pub wallet: String,
    pub dex_program: String,
    pub dex_name: String,
    pub token_in: String,
    pub amount_in: i64,
    pub token_out: String,
    pub amount_out: i64,
    pub price: f64,
}

#[derive(Debug, Deserialize)]
pub struct SwapQueryParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /api/v1/swaps/wallet/{address}
/// Get swap history for a wallet
#[get("/wallet/{address}")]
pub async fn get_wallet_swaps(
    db: web::Data<Arc<DatabaseManager>>,
    path: web::Path<String>,
    query: web::Query<SwapQueryParams>,
) -> HttpResponse {
    let wallet = path.into_inner();
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    match get_wallet_swaps_internal(&db, &wallet, limit, offset).await {
        Ok(swaps) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "wallet": wallet,
            "count": swaps.len(),
            "data": swaps,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

async fn get_wallet_swaps_internal(
    db: &DatabaseManager,
    wallet: &str,
    limit: i64,
    offset: i64,
) -> BeastResult<Vec<SwapResponse>> {
    let rows = db.get_wallet_swaps(wallet, limit, offset).await?;

    let swaps = rows
        .iter()
        .map(|row| SwapResponse {
            signature: row.get(0),
            event_index: row.get(1),
            slot: row.get(2),
            block_time: row.get(3),
            wallet: row.get(4),
            dex_program: row.get(5),
            dex_name: row.get(6),
            token_in: row.get(7),
            amount_in: row.get(8),
            token_out: row.get(9),
            amount_out: row.get(10),
            price: row.get(11),
        })
        .collect();

    Ok(swaps)
}

/// GET /api/v1/swaps/token/{mint}
/// Get swap history for a token
#[get("/token/{mint}")]
pub async fn get_token_swaps(
    db: web::Data<Arc<DatabaseManager>>,
    path: web::Path<String>,
    query: web::Query<SwapQueryParams>,
) -> HttpResponse {
    let token = path.into_inner();
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    match get_token_swaps_internal(&db, &token, limit, offset).await {
        Ok(swaps) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "token": token,
            "count": swaps.len(),
            "data": swaps,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

async fn get_token_swaps_internal(
    db: &DatabaseManager,
    token: &str,
    limit: i64,
    offset: i64,
) -> BeastResult<Vec<SwapResponse>> {
    let rows = db.get_token_swaps(token, limit, offset).await?;

    let swaps = rows
        .iter()
        .map(|row| SwapResponse {
            signature: row.get(0),
            event_index: row.get(1),
            slot: row.get(2),
            block_time: row.get(3),
            wallet: row.get(4),
            dex_program: row.get(5),
            dex_name: row.get(6),
            token_in: row.get(7),
            amount_in: row.get(8),
            token_out: row.get(9),
            amount_out: row.get(10),
            price: row.get(11),
        })
        .collect();

    Ok(swaps)
}

/// GET /api/v1/swaps/stats/{wallet}
/// Get swap statistics for a wallet
#[get("/stats/{wallet}")]
pub async fn get_wallet_swap_stats(
    db: web::Data<Arc<DatabaseManager>>,
    path: web::Path<String>,
) -> HttpResponse {
    let wallet = path.into_inner();

    match get_swap_stats_internal(&db, &wallet).await {
        Ok(stats) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "wallet": wallet,
            "data": stats,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

#[derive(Debug, Serialize)]
pub struct SwapStats {
    pub total_swaps: i64,
    pub unique_tokens: i64,
    pub dex_breakdown: Vec<DexUsage>,
    pub first_swap: Option<i64>,
    pub last_swap: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct DexUsage {
    pub dex_name: String,
    pub swap_count: i64,
}

async fn get_swap_stats_internal(db: &DatabaseManager, wallet: &str) -> BeastResult<SwapStats> {
    let (total_swaps, unique_tokens, dex_breakdown, first_swap, last_swap) =
        db.get_wallet_swap_stats(wallet).await?;

    let dex_breakdown = dex_breakdown
        .into_iter()
        .map(|(dex_name, swap_count)| DexUsage {
            dex_name,
            swap_count,
        })
        .collect();

    Ok(SwapStats {
        total_swaps,
        unique_tokens,
        dex_breakdown,
        first_swap,
        last_swap,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/swaps")
            .service(get_wallet_swaps)
            .service(get_token_swaps)
            .service(get_wallet_swap_stats),
    );
}

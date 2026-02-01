/// Price API Routes
/// REST endpoints for token price queries
use crate::price::JupiterPriceOracle;
use crate::storage::DatabaseManager;
use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct PriceQuery {
    pub token_mint: String,
}

#[derive(Deserialize)]
pub struct BatchPriceQuery {
    pub token_mints: Vec<String>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub token_mint: String,
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Serialize)]
pub struct PriceResponse {
    pub token_mint: String,
    pub price_usd: f64,
    pub timestamp: i64,
    pub source: String,
}

#[derive(Serialize)]
pub struct BatchPriceResponse {
    pub prices: Vec<PriceResponse>,
}

#[derive(Serialize)]
pub struct HistoryResponse {
    pub token_mint: String,
    pub prices: Vec<PricePoint>,
}

#[derive(Serialize)]
pub struct PricePoint {
    pub price_usd: f64,
    pub timestamp: i64,
}

/// Get current price for a token
#[get("/price/{token_mint}")]
pub async fn get_token_price(
    token_mint: web::Path<String>,
    oracle: web::Data<Arc<JupiterPriceOracle>>,
) -> impl Responder {
    match oracle.get_price(&token_mint).await {
        Ok(quote) => HttpResponse::Ok().json(PriceResponse {
            token_mint: quote.token_mint,
            price_usd: quote.price_usd,
            timestamp: quote.timestamp,
            source: quote.source,
        }),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get prices for multiple tokens
#[post("/price/batch")]
pub async fn get_batch_prices(
    query: web::Json<BatchPriceQuery>,
    oracle: web::Data<Arc<JupiterPriceOracle>>,
) -> impl Responder {
    match oracle.get_prices(&query.token_mints).await {
        Ok(quotes) => {
            let prices: Vec<PriceResponse> = quotes
                .into_iter()
                .map(|q| PriceResponse {
                    token_mint: q.token_mint,
                    price_usd: q.price_usd,
                    timestamp: q.timestamp,
                    source: q.source,
                })
                .collect();

            HttpResponse::Ok().json(BatchPriceResponse { prices })
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get historical prices for a token
#[get("/price/{token_mint}/history")]
pub async fn get_price_history(
    token_mint: web::Path<String>,
    query: web::Query<HistoryQuery>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    match db
        .get_price_history(&token_mint, query.start_time, query.end_time)
        .await
    {
        Ok(prices) => {
            let price_points: Vec<PricePoint> = prices
                .into_iter()
                .map(|(price_usd, timestamp)| PricePoint {
                    price_usd,
                    timestamp,
                })
                .collect();

            HttpResponse::Ok().json(HistoryResponse {
                token_mint: token_mint.to_string(),
                prices: price_points,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get wallet's cumulative PnL
#[get("/wallet/{wallet}/pnl")]
pub async fn get_wallet_pnl(
    wallet: web::Path<String>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    match db.get_wallet_pnl(&wallet).await {
        Ok(pnl) => HttpResponse::Ok().json(serde_json::json!({
            "wallet": wallet.to_string(),
            "total_pnl_usd": pnl,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get wallet's PnL for a specific token
#[get("/wallet/{wallet}/pnl/{token}")]
pub async fn get_wallet_token_pnl(
    path: web::Path<(String, String)>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    let (wallet, token) = path.into_inner();

    match db.get_wallet_token_pnl(&wallet, &token).await {
        Ok(pnl) => HttpResponse::Ok().json(serde_json::json!({
            "wallet": wallet,
            "token": token,
            "pnl_usd": pnl,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get wallet swaps with USD values
#[get("/wallet/{wallet}/swaps/usd")]
pub async fn get_wallet_swaps_usd(
    wallet: web::Path<String>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    match db.get_wallet_swaps_with_usd(&wallet, Some(100)).await {
        Ok(swaps) => HttpResponse::Ok().json(serde_json::json!({
            "wallet": wallet.to_string(),
            "swaps": swaps,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Oracle cache stats
#[get("/price/stats/cache")]
pub async fn get_cache_stats(oracle: web::Data<Arc<JupiterPriceOracle>>) -> impl Responder {
    let (total, stale) = oracle.cache_stats().await;
    HttpResponse::Ok().json(serde_json::json!({
        "total_cached": total,
        "stale_entries": stale,
        "fresh_entries": total - stale,
    }))
}

/// Configure price routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(get_token_price)
            .service(get_batch_prices)
            .service(get_price_history)
            .service(get_wallet_pnl)
            .service(get_wallet_token_pnl)
            .service(get_wallet_swaps_usd)
            .service(get_cache_stats),
    );
}

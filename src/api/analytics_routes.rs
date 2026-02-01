/// PnL Analytics API Routes
/// Claim verification, leaderboards, and performance metrics
use crate::price::{ClaimVerificationRequest, PnLEngine};
use crate::storage::DatabaseManager;
use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<i64>,
    pub token_mint: Option<String>,
}

/// Verify a PnL claim
#[post("/claim/verify")]
pub async fn verify_claim(
    request: web::Json<ClaimVerificationRequest>,
    pnl_engine: web::Data<Arc<PnLEngine>>,
) -> impl Responder {
    match pnl_engine.verify_claim(&request).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get position for wallet-token pair
#[get("/position/{wallet}/{token}")]
pub async fn get_position(
    path: web::Path<(String, String)>,
    pnl_engine: web::Data<Arc<PnLEngine>>,
) -> impl Responder {
    let (wallet, token) = path.into_inner();

    match pnl_engine.calculate_position(&wallet, &token).await {
        Ok(position) => HttpResponse::Ok().json(position),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get performance metrics for a wallet
#[get("/performance/{wallet}")]
pub async fn get_performance(
    wallet: web::Path<String>,
    pnl_engine: web::Data<Arc<PnLEngine>>,
) -> impl Responder {
    match pnl_engine.calculate_performance(&wallet).await {
        Ok(metrics) => HttpResponse::Ok().json(metrics),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get top performers leaderboard
#[get("/leaderboard/top-pnl")]
pub async fn get_top_performers(
    query: web::Query<LeaderboardQuery>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(10).min(100);
    let token_mint = query.token_mint.as_deref();

    match db.get_top_performers(limit, token_mint).await {
        Ok(results) => HttpResponse::Ok().json(serde_json::json!({
            "leaderboard": results,
            "limit": limit
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get most profitable tokens
#[get("/leaderboard/top-tokens")]
pub async fn get_top_tokens(
    query: web::Query<LeaderboardQuery>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(10).min(100);

    match db.get_top_tokens(limit).await {
        Ok(results) => HttpResponse::Ok().json(serde_json::json!({
            "top_tokens": results,
            "limit": limit
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get recent big wins
#[get("/analytics/big-wins")]
pub async fn get_big_wins(
    query: web::Query<LeaderboardQuery>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(10).min(100);

    match db.get_big_wins(limit).await {
        Ok(results) => HttpResponse::Ok().json(serde_json::json!({
            "big_wins": results,
            "limit": limit
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Get wallet win/loss statistics
#[get("/analytics/win-loss/{wallet}")]
pub async fn get_win_loss_stats(
    wallet: web::Path<String>,
    db: web::Data<Arc<DatabaseManager>>,
) -> impl Responder {
    match db.get_win_loss_stats(&wallet).await {
        Ok(stats) => HttpResponse::Ok().json(stats),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

/// Configure analytics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(verify_claim)
            .service(get_position)
            .service(get_performance)
            .service(get_top_performers)
            .service(get_top_tokens)
            .service(get_big_wins)
            .service(get_win_loss_stats),
    );
}

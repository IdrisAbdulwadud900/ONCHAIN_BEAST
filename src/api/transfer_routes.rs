use crate::api::server::ApiState;
use crate::metrics::{Timer, HTTP_REQUESTS_TOTAL, HTTP_REQUEST_DURATION};
use crate::modules::{EventIngestionWorker, IngestionConfig, TransferAnalytics};
/// Transfer Analytics API Endpoints
/// Provides detailed transfer analysis and statistics
use actix_web::{get, post, web, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletStatsRequest {
    pub wallet: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionSummaryRequest {
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchAnalyzeRequest {
    pub signatures: Vec<String>,
}

/// GET /transfer/wallet-stats/{wallet}
/// Get comprehensive transfer statistics for a wallet
#[get("/wallet-stats/{wallet}")]
async fn get_wallet_transfer_stats(
    wallet: web::Path<String>,
    state: web::Data<ApiState>,
) -> HttpResponse {
    let timer = Timer::new();

    let analytics = TransferAnalytics::new(state.db_manager.clone(), state.redis_cache.clone());

    match analytics.get_wallet_stats(&wallet).await {
        Ok(stats) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["GET", "/transfer/wallet-stats", "200"])
                .inc();
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "/transfer/wallet-stats"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "wallet": wallet.as_str(),
                "stats": stats,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["GET", "/transfer/wallet-stats", "500"])
                .inc();

            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// GET /transfer/summary/{signature}
/// Get transfer summary for a transaction
#[get("/summary/{signature}")]
async fn get_transaction_summary(
    signature: web::Path<String>,
    state: web::Data<ApiState>,
) -> HttpResponse {
    let timer = Timer::new();
    let handler = state.transaction_handler.read().await;

    // Get the transaction
    match handler.process_transaction(&signature, None).await {
        Ok(tx) => {
            let analytics =
                TransferAnalytics::new(state.db_manager.clone(), state.redis_cache.clone());

            match analytics.get_or_compute_summary(&tx).await {
                Ok(summary) => {
                    HTTP_REQUESTS_TOTAL
                        .with_label_values(&["GET", "/transfer/summary", "200"])
                        .inc();
                    HTTP_REQUEST_DURATION
                        .with_label_values(&["GET", "/transfer/summary"])
                        .observe(timer.elapsed_secs());

                    HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "signature": signature.as_str(),
                        "summary": summary,
                        "transaction": {
                            "slot": tx.slot,
                            "block_time": tx.block_time,
                            "success": tx.success,
                            "sol_transfers_count": tx.sol_transfers.len(),
                            "token_transfers_count": tx.token_transfers.len(),
                        },
                    }))
                }
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to compute summary: {}", e),
                })),
            }
        }
        Err(e) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["GET", "/transfer/summary", "400"])
                .inc();

            HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// POST /transfer/batch-analyze
/// Analyze multiple transactions and return summaries
#[post("/batch-analyze")]
async fn batch_analyze_transfers(
    req: web::Json<BatchAnalyzeRequest>,
    state: web::Data<ApiState>,
) -> HttpResponse {
    let timer = Timer::new();
    let handler = state.transaction_handler.read().await;

    // Fetch all transactions
    let mut transactions = Vec::new();
    for sig in &req.signatures {
        match handler.process_transaction(sig, None).await {
            Ok(tx) => transactions.push(tx),
            Err(e) => {
                tracing::warn!("Failed to fetch transaction {}: {}", sig, e);
            }
        }
    }

    let analytics = TransferAnalytics::new(state.db_manager.clone(), state.redis_cache.clone());

    match analytics.batch_analyze(&transactions).await {
        Ok(summaries) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/batch-analyze", "200"])
                .inc();
            HTTP_REQUEST_DURATION
                .with_label_values(&["POST", "/transfer/batch-analyze"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "analyzed_count": transactions.len(),
                "requested_count": req.signatures.len(),
                "summaries": summaries,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

/// GET /transfer/top-transfers/{wallet}
/// Get top transfers for a wallet
#[get("/top-transfers/{wallet}")]
async fn get_top_transfers(wallet: web::Path<String>, state: web::Data<ApiState>) -> HttpResponse {
    let timer = Timer::new();

    // Get wallet connections from database
    match state.db_manager.get_wallet_connections(&wallet).await {
        Ok(connections) => {
            // Sort by total transferred
            let mut sorted = connections;
            sorted.sort_by(|a, b| {
                let a_total = a.total_sol_transferred + (a.total_token_transferred as f64 * 0.001);
                let b_total = b.total_sol_transferred + (b.total_token_transferred as f64 * 0.001);
                b_total
                    .partial_cmp(&a_total)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let top_10 = sorted.iter().take(10).collect::<Vec<_>>();

            HTTP_REQUESTS_TOTAL
                .with_label_values(&["GET", "/transfer/top-transfers", "200"])
                .inc();
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "/transfer/top-transfers"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "wallet": wallet.as_str(),
                "total_connections": sorted.len(),
                "top_transfers": top_10,
            }))
        }
        Err(e) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["GET", "/transfer/top-transfers", "500"])
                .inc();

            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// POST /transfer/ingest/wallet
/// Ingest transaction events for a specific wallet
#[derive(Debug, Deserialize)]
struct IngestWalletRequest {
    wallet: String,
    #[serde(default)]
    batch_size: Option<usize>,
    #[serde(default)]
    max_age_days: Option<u64>,
}

#[post("/ingest/wallet")]
async fn ingest_wallet_events(
    req: web::Json<IngestWalletRequest>,
    state: web::Data<ApiState>,
) -> HttpResponse {
    let timer = Timer::new();

    let config = IngestionConfig {
        batch_size: req.batch_size.unwrap_or(100),
        max_age_days: req.max_age_days.unwrap_or(30),
        ..Default::default()
    };

    let worker = EventIngestionWorker::new(
        state.db_manager.clone(),
        state.rpc_client.clone(),
        state.transaction_handler.clone(),
        state.transfer_analytics.clone(),
        config,
    );

    match worker.ingest_wallet(&req.wallet).await {
        Ok(stats) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/ingest/wallet", "200"])
                .inc();
            HTTP_REQUEST_DURATION
                .with_label_values(&["POST", "/transfer/ingest/wallet"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "wallet": req.wallet,
                "stats": {
                    "total_signatures": stats.total_signatures,
                    "ingested_ok": stats.ingested_ok,
                    "ingested_failed": stats.ingested_failed,
                    "parse_failed": stats.parse_failed,
                    "skipped_duplicate": stats.skipped_duplicate,
                },
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/ingest/wallet", "500"])
                .inc();

            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// POST /transfer/ingest/batch
/// Ingest events for multiple wallets
#[derive(Debug, Deserialize)]
struct IngestBatchRequest {
    wallets: Vec<String>,
    #[serde(default)]
    batch_size: Option<usize>,
    #[serde(default)]
    max_concurrent: Option<usize>,
    #[serde(default)]
    max_age_days: Option<u64>,
}

#[post("/ingest/batch")]
async fn ingest_batch_events(
    req: web::Json<IngestBatchRequest>,
    state: web::Data<ApiState>,
) -> HttpResponse {
    let timer = Timer::new();

    let config = IngestionConfig {
        batch_size: req.batch_size.unwrap_or(100),
        max_concurrent: req.max_concurrent.unwrap_or(5),
        max_age_days: req.max_age_days.unwrap_or(30),
        ..Default::default()
    };

    let worker = EventIngestionWorker::new(
        state.db_manager.clone(),
        state.rpc_client.clone(),
        state.transaction_handler.clone(),
        state.transfer_analytics.clone(),
        config,
    );

    match worker.ingest_wallets(req.wallets.clone()).await {
        Ok(stats) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/ingest/batch", "200"])
                .inc();
            HTTP_REQUEST_DURATION
                .with_label_values(&["POST", "/transfer/ingest/batch"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "wallets_processed": req.wallets.len(),
                "stats": {
                    "wallets_success": stats.wallets_success,
                    "wallets_failed": stats.wallets_failed,
                    "total_signatures": stats.total_signatures,
                    "ingested_ok": stats.ingested_ok,
                    "ingested_failed": stats.ingested_failed,
                    "parse_failed": stats.parse_failed,
                    "skipped_duplicate": stats.skipped_duplicate,
                },
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/ingest/batch", "500"])
                .inc();

            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// POST /transfer/ingest/backfill
/// Backfill events from wallet_relationships table
#[derive(Debug, Deserialize)]
struct BackfillRequest {
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    batch_size: Option<usize>,
    #[serde(default)]
    max_concurrent: Option<usize>,
}

fn default_limit() -> usize {
    100
}

#[post("/ingest/backfill")]
async fn backfill_events(
    req: web::Json<BackfillRequest>,
    state: web::Data<ApiState>,
) -> HttpResponse {
    let timer = Timer::new();

    let config = IngestionConfig {
        batch_size: req.batch_size.unwrap_or(100),
        max_concurrent: req.max_concurrent.unwrap_or(5),
        ..Default::default()
    };

    let worker = EventIngestionWorker::new(
        state.db_manager.clone(),
        state.rpc_client.clone(),
        state.transaction_handler.clone(),
        state.transfer_analytics.clone(),
        config,
    );

    match worker.backfill_from_relationships(req.limit).await {
        Ok(stats) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/ingest/backfill", "200"])
                .inc();
            HTTP_REQUEST_DURATION
                .with_label_values(&["POST", "/transfer/ingest/backfill"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "stats": {
                    "wallets_success": stats.wallets_success,
                    "wallets_failed": stats.wallets_failed,
                    "total_signatures": stats.total_signatures,
                    "ingested_ok": stats.ingested_ok,
                    "ingested_failed": stats.ingested_failed,
                    "parse_failed": stats.parse_failed,
                    "skipped_duplicate": stats.skipped_duplicate,
                },
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            HTTP_REQUESTS_TOTAL
                .with_label_values(&["POST", "/transfer/ingest/backfill", "500"])
                .inc();

            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            }))
        }
    }
}

/// Configure transfer analytics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/transfer")
            .service(get_wallet_transfer_stats)
            .service(get_transaction_summary)
            .service(batch_analyze_transfers)
            .service(get_top_transfers)
            .service(ingest_wallet_events)
            .service(ingest_batch_events)
            .service(backfill_events),
    );
}

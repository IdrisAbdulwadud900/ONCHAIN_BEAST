/// Transaction Parsing Endpoints
/// Real-time transaction parsing and analysis

use actix_web::{web, HttpResponse, get, post, middleware::Logger};
use serde::{Deserialize, Serialize};
use crate::modules::TransactionHandler;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
pub struct ParseTransactionRequest {
    pub signature: String,
    #[serde(default)]
    pub commitment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParseTransactionResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParseWalletTransactionsRequest {
    pub wallet: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    10
}

/// Parse a single transaction in detail
#[get("/parse/transaction/{signature}")]
async fn parse_transaction(
    signature: web::Path<String>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;
    
    match handler.process_transaction(&signature, None).await {
        Ok(parsed) => {
            HttpResponse::Ok().json(ParseTransactionResponse {
                success: true,
                data: serde_json::to_value(&parsed).ok(),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("Failed to parse transaction {}: {}", signature, e);
            HttpResponse::BadRequest().json(ParseTransactionResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Parse a wallet's transaction history
#[post("/parse/wallet-transactions")]
async fn parse_wallet_transactions(
    req: web::Json<ParseWalletTransactionsRequest>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    if req.limit > 100 {
        return HttpResponse::BadRequest().json(ParseTransactionResponse {
            success: false,
            data: None,
            error: Some("Limit cannot exceed 100".to_string()),
        });
    }

    let handler = handler.read().await;

    match handler.process_wallet_transactions(&req.wallet, req.limit).await {
        Ok(transactions) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "wallet": req.wallet,
                "transactions_parsed": transactions.len(),
                "data": transactions,
                "error": null
            }))
        }
        Err(e) => {
            tracing::error!("Failed to parse wallet transactions: {}", e);
            HttpResponse::BadRequest().json(ParseTransactionResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Batch parse multiple transactions
#[post("/parse/batch")]
async fn parse_batch(
    req: web::Json<Vec<String>>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    if req.len() > 50 {
        return HttpResponse::BadRequest().json(ParseTransactionResponse {
            success: false,
            data: None,
            error: Some("Batch size cannot exceed 50".to_string()),
        });
    }

    let handler = handler.read().await;

    match handler.process_transactions_batch(req.into_inner()).await {
        Ok(transactions) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "transactions_parsed": transactions.len(),
                "data": transactions,
                "error": null
            }))
        }
        Err(e) => {
            tracing::error!("Failed to parse batch: {}", e);
            HttpResponse::BadRequest().json(ParseTransactionResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Get SOL transfers from a transaction
#[get("/parse/transaction/{signature}/sol-transfers")]
async fn get_sol_transfers(
    signature: web::Path<String>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    match handler.process_transaction(&signature, None).await {
        Ok(parsed) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "signature": signature.to_string(),
                "sol_transfers": parsed.sol_transfers,
                "transfer_count": parsed.sol_transfers.len(),
                "total_sol_moved": parsed.sol_transfers.iter().map(|t| t.amount_sol).sum::<f64>(),
            }))
        }
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

/// Get token transfers from a transaction
#[get("/parse/transaction/{signature}/token-transfers")]
async fn get_token_transfers(
    signature: web::Path<String>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    match handler.process_transaction(&signature, None).await {
        Ok(parsed) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "signature": signature.to_string(),
                "token_transfers": parsed.token_transfers,
                "transfer_count": parsed.token_transfers.len(),
                "unique_mints": parsed.token_transfers.iter()
                    .map(|t| t.mint.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
            }))
        }
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

/// Get transaction summary (high-level overview)
#[get("/parse/transaction/{signature}/summary")]
async fn get_transaction_summary(
    signature: web::Path<String>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    match handler.process_transaction(&signature, None).await {
        Ok(parsed) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "signature": parsed.signature,
                "slot": parsed.slot,
                "block_time": parsed.block_time,
                "fee": parsed.fee,
                "fee_sol": parsed.fee as f64 / 1_000_000_000.0,
                "success": parsed.success,
                "error": parsed.error,
                "transaction_type": format!("{:?}", parsed.tx_type),
                "is_versioned": parsed.is_versioned,
                "accounts_involved": parsed.accounts.len(),
                "signers": parsed.signers.len(),
                "programs_called": parsed.program_names,
                "sol_transfers": {
                    "count": parsed.sol_transfers.len(),
                    "total_amount_sol": parsed.sol_transfers.iter().map(|t| t.amount_sol).sum::<f64>(),
                },
                "token_transfers": {
                    "count": parsed.token_transfers.len(),
                    "unique_mints": parsed.token_transfers.iter()
                        .map(|t| t.mint.clone())
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                },
                "balance_changes": {
                    "count": parsed.balance_changes.len(),
                    "net_changes": parsed.balance_changes.iter()
                        .filter(|bc| bc.change_lamports != 0)
                        .count(),
                },
            }))
        }
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": e.to_string(),
        })),
    }
}

/// Cache statistics
#[get("/parse/cache-stats")]
async fn get_cache_stats(
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;
    let size = handler.cache_size().await;

    HttpResponse::Ok().json(serde_json::json!({
        "cached_transactions": size,
        "cache_memory_estimate_mb": (size * 8) / 1024, // Rough estimate
    }))
}

/// Clear cache (admin endpoint)
#[post("/parse/clear-cache")]
async fn clear_cache(
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;
    handler.clear_cache().await;

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Cache cleared",
    }))
}

/// Configure transaction parser routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/parse")
            .wrap(Logger::default())
            .service(parse_transaction)
            .service(parse_wallet_transactions)
            .service(parse_batch)
            .service(get_sol_transfers)
            .service(get_token_transfers)
            .service(get_transaction_summary)
            .service(get_cache_stats)
            .service(clear_cache)
    );
}

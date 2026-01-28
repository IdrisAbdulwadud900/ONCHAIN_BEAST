/// REST API Server for OnChain Beast
///
/// Provides HTTP endpoints for wallet analysis, transaction tracing, and pattern detection

use actix_web::{web, App, HttpServer, HttpResponse, middleware};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::rpc_client::SolanaRpcClient;
use crate::core::config::Config;
use crate::database::storage::Database;
use crate::analysis::AnalysisEngine;
use crate::cache::CacheManager;
use crate::middleware::{RateLimiter, RateLimiterConfig, RequestId};
use crate::auth::ApiKey;
use crate::modules::TransactionHandler;

/// API server state
pub struct ApiState {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub database: Arc<RwLock<Database>>,
    pub analysis_engine: Arc<RwLock<AnalysisEngine>>,
    pub cache: Arc<CacheManager>,
    pub transaction_handler: Arc<RwLock<TransactionHandler>>,
}

/// Start the REST API server
pub async fn start_server(
    config: Config,
    rpc_client: Arc<SolanaRpcClient>,
    database: Arc<RwLock<Database>>,
    analysis_engine: Arc<RwLock<AnalysisEngine>>,
    cache: Arc<CacheManager>,
    host: &str,
    port: u16,
) -> std::io::Result<()> {
    let transaction_handler = Arc::new(RwLock::new(
        TransactionHandler::new(Arc::clone(&rpc_client))
    ));

    let state = web::Data::new(ApiState {
        rpc_client,
        database,
        analysis_engine,
        cache,
        transaction_handler: Arc::clone(&transaction_handler),
    });

    info!("🌐 Starting REST API server on {}:{}", host, port);

    // Configure middleware (rate limiting and request tracking)
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        requests_per_minute: config.rate_limit_per_minute,
        burst_size: 10,
    });

    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(state.clone())
            .app_data(web::Data::new(Arc::clone(&transaction_handler)))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .wrap(RequestId::new())
            .wrap(rate_limiter.clone());

        // Configure transaction parsing routes
        app = app.configure(crate::api::parse_routes::configure);

        app
            // Note: Authentication is now handled via extractors in individual handlers
            // Health check endpoints (public - no auth required)
            .route("/health", web::get().to(handlers::health_check))

            .route("/status", web::get().to(handlers::get_status))
            // Wallet analysis endpoints
            .route("/api/v1/analyze/wallet/{address}", web::get().to(handlers::analyze_wallet))
            .route("/api/v1/analyze/wallet", web::post().to(handlers::analyze_wallet_post))
            .route("/api/v1/wallet/{address}/risk", web::get().to(handlers::get_wallet_risk))
            .route("/api/v1/wallet/{address}/transactions", web::get().to(handlers::get_wallet_transactions))
            // Side wallet detection
            .route("/api/v1/wallet/{address}/side-wallets", web::get().to(handlers::find_side_wallets))
            .route("/api/v1/wallet/{address}/cluster", web::get().to(handlers::get_wallet_cluster))
            // Exchange and fund tracing
            .route("/api/v1/trace/funds", web::post().to(handlers::trace_funds))
            .route("/api/v1/trace/exchange-routes", web::post().to(handlers::trace_exchange_routes))
            // Pattern detection
            .route("/api/v1/detect/patterns", web::post().to(handlers::detect_patterns))
            .route("/api/v1/detect/anomalies", web::get().to(handlers::detect_anomalies))
            .route("/api/v1/detect/wash-trading/{address}", web::get().to(handlers::detect_wash_trading))
            // Network analysis
            .route("/api/v1/network/metrics", web::get().to(handlers::get_network_metrics))
            .route("/api/v1/network/analysis", web::post().to(handlers::network_analysis))
            // Account info
            .route("/api/v1/account/{address}/balance", web::get().to(handlers::get_account_balance))
            .route("/api/v1/account/{address}/info", web::get().to(handlers::get_account_info))
            // Cluster info
            .route("/api/v1/cluster/info", web::get().to(handlers::get_cluster_info))
            .route("/api/v1/cluster/health", web::get().to(handlers::cluster_health))
            // Root endpoint
            .route("/", web::get().to(handlers::index))
    })
    .bind((host, port))?
    .run()
    .await
}

pub mod handlers {
    use super::*;
    use serde_json::json;

    /// Root endpoint
    pub async fn index() -> HttpResponse {
        HttpResponse::Ok().json(json!({
            "service": "OnChain Beast - Solana Blockchain Analysis",
            "version": "0.1.0",
            "description": "Powerful on-chain analysis tool for Solana blockchain",
            "endpoints": {
                "health": "/health",
                "status": "/status",
                "api_docs": "/api/v1/docs",
                "wallet_analysis": "/api/v1/analyze/wallet/{address}",
                "side_wallets": "/api/v1/wallet/{address}/side-wallets",
                "fund_tracing": "/api/v1/trace/funds",
                "pattern_detection": "/api/v1/detect/patterns"
            }
        }))
    }

    /// Health check endpoint
    pub async fn health_check(state: web::Data<ApiState>) -> HttpResponse {
        match state.rpc_client.health_check().await {
            Ok(true) => HttpResponse::Ok().json(json!({
                "status": "healthy",
                "service": "onchain_beast",
                "rpc": "connected"
            })),
            Ok(false) => HttpResponse::ServiceUnavailable().json(json!({
                "status": "unhealthy",
                "service": "onchain_beast",
                "rpc": "disconnected"
            })),
            Err(e) => HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "error": e.to_string()
            })),
        }
    }

    /// Get system status
    pub async fn get_status(state: web::Data<ApiState>) -> HttpResponse {
        // Check cache first
        let cache_key = "cluster_info";
        if let Some(cached) = state.cache.cluster_cache.get(cache_key) {
            return HttpResponse::Ok().json(cached);
        }

        match state.rpc_client.get_cluster_info().await {
            Ok(cluster) => {
                let response = json!({
                    "status": "operational",
                    "cluster": {
                        "nodes": cluster.total_nodes,
                        "active_validators": cluster.total_nodes
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "cached": false
                });
                
                // Cache the response
                state.cache.cluster_cache.set(cache_key.to_string(), response.clone());
                
                HttpResponse::Ok().json(response)
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": e.to_string()
            })),
        }
    }

    /// Analyze wallet (GET)
    pub async fn analyze_wallet(
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet_address = address.into_inner();
        
        // Check cache first
        let cache_key = format!("account_{}", wallet_address);
        if let Some(cached) = state.cache.account_cache.get(&cache_key) {
            let mut response = cached;
            if let Some(obj) = response.as_object_mut() {
                obj.insert("cached".to_string(), json!(true));
            }
            return HttpResponse::Ok().json(response);
        }
        
        match state.rpc_client.get_account_info(&wallet_address).await {
            Ok(account) => {
                let response = json!({
                    "wallet": wallet_address,
                    "balance_lamports": account.balance,
                    "balance_sol": account.balance as f64 / 1_000_000_000.0,
                    "owner": account.owner,
                    "executable": account.executable,
                    "analysis_ready": true,
                    "cached": false
                });
                
                // Cache the response
                state.cache.account_cache.set(cache_key, response.clone());
                
                HttpResponse::Ok().json(response)
            }
            Err(e) => HttpResponse::NotFound().json(json!({
                "error": format!("Wallet not found: {}", e),
                "wallet": wallet_address
            })),
        }
    }

    /// Analyze wallet (POST with options)
    /// Requires authentication when enabled
    pub async fn analyze_wallet_post(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        req: web::Json<AnalyzeWalletRequest>,
    ) -> HttpResponse {
        match state.rpc_client.get_account_info(&req.wallet).await {
            Ok(account) => {
                let mut response = json!({
                    "wallet": req.wallet,
                    "balance_lamports": account.balance,
                    "balance_sol": account.balance as f64 / 1_000_000_000.0,
                    "owner": account.owner,
                    "executable": account.executable
                });

                if req.include_transactions.unwrap_or(false) {
                    match state.rpc_client.get_signatures(&req.wallet, 10).await {
                        Ok(sigs) => {
                            response["recent_transactions"] = json!(sigs.len());
                        }
                        Err(_) => {}
                    }
                }

                HttpResponse::Ok().json(response)
            }
            Err(e) => HttpResponse::NotFound().json(json!({
                "error": format!("Analysis failed: {}", e),
                "wallet": req.wallet
            })),
        }
    }

    /// Get wallet risk score
    /// Requires authentication when enabled
    pub async fn get_wallet_risk(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet = address.into_inner();
        
        // Calculate risk based on wallet activity
        match state.rpc_client.get_signatures(&wallet, 200u64).await {
            Ok(sigs) => {
                let risk_score = if sigs.len() > 100 {
                    0.7
                } else if sigs.len() > 50 {
                    0.5
                } else {
                    0.2
                };

                HttpResponse::Ok().json(json!({
                    "wallet": wallet,
                    "risk_score": risk_score,
                    "transaction_count": sigs.len(),
                    "risk_level": match risk_score {
                        x if x < 0.3 => "low",
                        x if x < 0.6 => "medium",
                        _ => "high"
                    }
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": e.to_string()
            })),
        }
    }

    /// Get wallet transactions
    pub async fn get_wallet_transactions(
        state: web::Data<ApiState>,
        address: web::Path<String>,
        query: web::Query<TransactionQuery>,
    ) -> HttpResponse {
        let wallet = address.into_inner();
        let limit = query.limit.unwrap_or(10).min(100) as u64;

        match state.rpc_client.get_signatures(&wallet, limit).await {
            Ok(sigs) => {
                HttpResponse::Ok().json(json!({
                    "wallet": wallet,
                    "transactions": sigs.iter()
                        .map(|s| json!({
                            "signature": s.signature,
                            "slot": s.slot,
                            "block_time": s.block_time
                        }))
                        .collect::<Vec<_>>(),
                    "total": sigs.len(),
                    "returned": sigs.len().min(limit as usize)
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": e.to_string()
            })),
        }
    }

    /// Find side wallets
    /// Requires authentication when enabled
    pub async fn find_side_wallets(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        // This would integrate with the graph analysis engine
        HttpResponse::Ok().json(json!({
            "main_wallet": wallet,
            "side_wallets": [],
            "confidence_threshold": 0.7,
            "analysis_depth": 3,
            "message": "Graph analysis integration pending"
        }))
    }

    /// Get wallet cluster
    /// Requires authentication when enabled
    pub async fn get_wallet_cluster(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        HttpResponse::Ok().json(json!({
            "primary_wallet": wallet,
            "cluster_size": 1,
            "wallets": [wallet],
            "connection_strength": 1.0,
            "message": "Graph analysis integration pending"
        }))
    }

    /// Trace funds
    /// Requires authentication when enabled
    pub async fn trace_funds(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        req: web::Json<TraceFundsRequest>,
    ) -> HttpResponse {
        HttpResponse::Ok().json(json!({
            "from": req.from,
            "to": req.to,
            "paths_found": 0,
            "status": "analysis_ready",
            "message": "Graph analysis integration pending"
        }))
    }

    /// Trace exchange routes
    pub async fn trace_exchange_routes(
        state: web::Data<ApiState>,
        req: web::Json<TraceExchangeRequest>,
    ) -> HttpResponse {
        HttpResponse::Ok().json(json!({
            "source": req.source,
            "destination": req.destination,
            "routes": [],
            "exchanges_detected": 0,
            "message": "Graph analysis integration pending"
        }))
    }

    /// Detect patterns
    /// Requires authentication when enabled
    pub async fn detect_patterns(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        req: web::Json<DetectPatternRequest>,
    ) -> HttpResponse {
        HttpResponse::Ok().json(json!({
            "wallet": req.wallet,
            "patterns_detected": [],
            "anomaly_score": 0.0,
            "analysis_type": req.pattern_type,
            "message": "Pattern detection integration pending"
        }))
    }

    /// Detect network anomalies
    pub async fn detect_anomalies(state: web::Data<ApiState>) -> HttpResponse {
        HttpResponse::Ok().json(json!({
            "unusual_wallets": [],
            "suspicious_patterns": 0,
            "high_risk_count": 0,
            "analysis_timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Detect wash trading
    /// Requires authentication when enabled
    pub async fn detect_wash_trading(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        HttpResponse::Ok().json(json!({
            "wallet": wallet,
            "wash_trading_detected": false,
            "cycles_found": 0,
            "suspicious_score": 0.0,
            "message": "Graph analysis integration pending"
        }))
    }

    /// Get network metrics
    pub async fn get_network_metrics(state: web::Data<ApiState>) -> HttpResponse {
        match state.rpc_client.get_cluster_info().await {
            Ok(cluster) => {
                HttpResponse::Ok().json(json!({
                    "network": "solana",
                    "cluster": "mainnet-beta",
                    "active_validators": cluster.total_nodes,
                    "network_health": "good",
                    "tps": 400,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": e.to_string()
            })),
        }
    }

    /// Network analysis
    /// Requires authentication when enabled
    pub async fn network_analysis(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        req: web::Json<NetworkAnalysisRequest>,
    ) -> HttpResponse {
        HttpResponse::Ok().json(json!({
            "analysis_type": req.analysis_type,
            "wallets_analyzed": 0,
            "relationships_found": 0,
            "clusters_detected": 0,
            "message": "Network analysis integration pending"
        }))
    }

    /// Get account balance
    pub async fn get_account_balance(
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        match state.rpc_client.get_account_info(&wallet).await {
            Ok(account) => {
                HttpResponse::Ok().json(json!({
                    "wallet": wallet,
                    "balance_lamports": account.balance,
                    "balance_sol": account.balance as f64 / 1_000_000_000.0,
                    "owner": account.owner
                }))
            }
            Err(e) => HttpResponse::NotFound().json(json!({
                "error": e.to_string(),
                "wallet": wallet
            })),
        }
    }

    /// Get account info
    pub async fn get_account_info(
        state: web::Data<ApiState>,
        address: web::Path<String>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        match state.rpc_client.get_account_info(&wallet).await {
            Ok(account) => {
                HttpResponse::Ok().json(json!({
                    "address": wallet,
                    "balance_lamports": account.balance,
                    "balance_sol": account.balance as f64 / 1_000_000_000.0,
                    "owner": account.owner,
                    "executable": account.executable,
                    "rent_epoch": account.rent_epoch
                }))
            }
            Err(e) => HttpResponse::NotFound().json(json!({
                "error": e.to_string()
            })),
        }
    }

    /// Get cluster info
    pub async fn get_cluster_info(state: web::Data<ApiState>) -> HttpResponse {
        match state.rpc_client.get_cluster_info().await {
            Ok(cluster) => {
                HttpResponse::Ok().json(json!({
                    "cluster": "mainnet-beta",
                    "total_validators": cluster.total_nodes,
                    "network_version": "1.18",
                    "health": "operational"
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": e.to_string()
            })),
        }
    }

    /// Cluster health check
    pub async fn cluster_health(state: web::Data<ApiState>) -> HttpResponse {
        match state.rpc_client.health_check().await {
            Ok(true) => HttpResponse::Ok().json(json!({
                "cluster": "solana-mainnet",
                "health": "healthy",
                "rpc_status": "operational"
            })),
            Ok(false) => HttpResponse::ServiceUnavailable().json(json!({
                "cluster": "solana-mainnet",
                "health": "degraded",
                "rpc_status": "slow"
            })),
            Err(e) => HttpResponse::ServiceUnavailable().json(json!({
                "cluster": "solana-mainnet",
                "health": "unhealthy",
                "error": e.to_string()
            })),
        }
    }
}

// Request types for POST endpoints
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeWalletRequest {
    pub wallet: String,
    pub include_transactions: Option<bool>,
    pub depth: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceFundsRequest {
    pub from: String,
    pub to: String,
    pub max_depth: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceExchangeRequest {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetectPatternRequest {
    pub wallet: String,
    pub pattern_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkAnalysisRequest {
    pub analysis_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionQuery {
    pub limit: Option<usize>,
    pub before: Option<String>,
}

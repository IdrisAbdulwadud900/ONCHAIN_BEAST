/// REST API Server for OnChain Beast
///
/// Provides HTTP endpoints for wallet analysis, transaction tracing, and pattern detection
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::analysis::AnalysisEngine;
use crate::auth::ApiKey;
use crate::cache::CacheManager;
use crate::core::config::Config;
use crate::core::rpc_client::SolanaRpcClient;
use crate::core::TokenMetadataService;
use crate::database::storage::Database;
use crate::middleware::{RateLimiter, RateLimiterConfig, RequestId};
use crate::modules::{
    AnalysisService, PatternDetector, TokenMetadataServiceEnhanced, TransactionGraphBuilder,
    TransactionHandler, TransferAnalytics,
};
use crate::storage::{BehavioralProfile, DatabaseManager, RedisCache};

/// API server state
pub struct ApiState {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub database: Arc<RwLock<Database>>,
    pub analysis_engine: Arc<RwLock<AnalysisEngine>>,
    pub cache: Arc<CacheManager>,
    pub transaction_handler: Arc<RwLock<TransactionHandler>>,
    pub db_manager: Arc<DatabaseManager>,
    pub redis_cache: Arc<RedisCache>,
    pub token_metadata_service: Arc<TokenMetadataServiceEnhanced>,
    pub analysis_service: Arc<AnalysisService>,
    pub transfer_analytics: Arc<TransferAnalytics>,
}

/// Start the REST API server
pub async fn start_server(
    config: Config,
    rpc_client: Arc<SolanaRpcClient>,
    database: Arc<RwLock<Database>>,
    analysis_engine: Arc<RwLock<AnalysisEngine>>,
    cache: Arc<CacheManager>,
    db_manager: Arc<DatabaseManager>,
    redis_cache: Arc<RedisCache>,
    price_oracle: Arc<crate::price::JupiterPriceOracle>,
    host: &str,
    port: u16,
) -> std::io::Result<()> {
    let rpc_url = config.rpc_endpoint.clone();
    let transaction_handler = Arc::new(RwLock::new(TransactionHandler::new(
        Arc::clone(&rpc_client),
        rpc_url.clone(),
    )));

    // Initialize token metadata service with Phase 5 infrastructure
    let metadata_service = TokenMetadataService::new(rpc_url);
    metadata_service.preload_common_tokens().await;
    let token_metadata_service = Arc::new(TokenMetadataServiceEnhanced::new(
        metadata_service,
        Arc::clone(&db_manager),
        Arc::clone(&redis_cache),
    ));
    info!("✅ Initialized enhanced token metadata service with caching and persistence");

    // Initialize analysis service with Phase 5 infrastructure
    let pattern_detector = Arc::new(PatternDetector::new());
    let graph_builder = Arc::new(TransactionGraphBuilder::new());
    let analysis_service = Arc::new(AnalysisService::new(
        pattern_detector,
        graph_builder,
        Arc::clone(&db_manager),
        Arc::clone(&redis_cache),
    ));
    info!("✅ Initialized analysis service with caching and metrics");

    // Initialize transfer analytics
    let transfer_analytics = Arc::new(TransferAnalytics::new(
        Arc::clone(&db_manager),
        Arc::clone(&redis_cache),
    ));

    // Initialize PnL engine
    let pnl_engine = Arc::new(crate::price::PnLEngine::new(
        Arc::clone(&db_manager),
        Arc::clone(&price_oracle),
    ));
    info!("✅ Initialized PnL calculation engine");

    let state = web::Data::new(ApiState {
        rpc_client,
        database,
        analysis_engine,
        cache,
        transaction_handler: Arc::clone(&transaction_handler),
        db_manager: Arc::clone(&db_manager),
        redis_cache: Arc::clone(&redis_cache),
        token_metadata_service,
        analysis_service,
        transfer_analytics,
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
            // Expose raw shared services for route modules that use Data<Arc<T>> directly.
            .app_data(web::Data::new(Arc::clone(&db_manager)))
            .app_data(web::Data::new(Arc::clone(&redis_cache)))
            .app_data(web::Data::new(Arc::clone(&price_oracle)))
            .app_data(web::Data::new(Arc::clone(&pnl_engine)))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .wrap(RequestId::new())
            .wrap(rate_limiter.clone());

        // Configure transaction parsing routes
        app = app.configure(crate::api::parse_routes::configure);

        // Configure analysis routes
        app = app.configure(crate::api::analysis_routes::configure);

        // Configure metrics routes
        app = app.configure(crate::api::metrics_routes::configure_metrics_routes);

        // Configure transfer analytics routes
        app = app.configure(crate::api::transfer_routes::configure);

        // Configure swap query routes
        app = app.configure(crate::api::swap_routes::configure);

        // Configure price query routes
        app = app.configure(crate::api::price_routes::configure);

        // Configure analytics routes (PnL, claims, leaderboards)
        app = app.configure(crate::api::analytics_routes::configure);

        // Configure token metadata routes
        app = app.configure(crate::api::metadata_routes::configure);

        // Configure enhanced analysis API routes
        app = app.configure(crate::api::analysis_api_enhanced::configure);

        app
            // Note: Authentication is now handled via extractors in individual handlers
            // Health check endpoints (public - no auth required)
            .route("/health", web::get().to(handlers::health_check))
            .route("/status", web::get().to(handlers::get_status))
            // Wallet analysis endpoints
            .route(
                "/api/v1/analyze/wallet/{address}",
                web::get().to(handlers::analyze_wallet),
            )
            .route(
                "/api/v1/analyze/wallet",
                web::post().to(handlers::analyze_wallet_post),
            )
            .route(
                "/api/v1/wallet/{address}/risk",
                web::get().to(handlers::get_wallet_risk),
            )
            .route(
                "/api/v1/wallet/{address}/transactions",
                web::get().to(handlers::get_wallet_transactions),
            )
            // Side wallet detection
            .route(
                "/api/v1/wallet/{address}/side-wallets",
                web::get().to(handlers::find_side_wallets),
            )
            .route(
                "/api/v1/wallet/{address}/cluster",
                web::get().to(handlers::get_wallet_cluster),
            )
            // Exchange and fund tracing
            .route("/api/v1/trace/funds", web::post().to(handlers::trace_funds))
            .route(
                "/api/v1/trace/exchange-routes",
                web::post().to(handlers::trace_exchange_routes),
            )
            // Pattern detection
            .route(
                "/api/v1/detect/patterns",
                web::post().to(handlers::detect_patterns),
            )
            .route(
                "/api/v1/detect/anomalies",
                web::get().to(handlers::detect_anomalies),
            )
            .route(
                "/api/v1/detect/wash-trading/{address}",
                web::get().to(handlers::detect_wash_trading),
            )
            // Network analysis
            .route(
                "/api/v1/network/metrics",
                web::get().to(handlers::get_network_metrics),
            )
            .route(
                "/api/v1/network/analysis",
                web::post().to(handlers::network_analysis),
            )
            // Account info
            .route(
                "/api/v1/account/{address}/balance",
                web::get().to(handlers::get_account_balance),
            )
            .route(
                "/api/v1/account/{address}/info",
                web::get().to(handlers::get_account_info),
            )
            // Cluster info
            .route(
                "/api/v1/cluster/info",
                web::get().to(handlers::get_cluster_info),
            )
            .route(
                "/api/v1/cluster/health",
                web::get().to(handlers::cluster_health),
            )
            // Root endpoint
            .route("/", web::get().to(handlers::index))
    })
    .bind((host, port))?
    .run()
    .await
}

pub mod handlers {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;
    use std::collections::{HashMap, HashSet, VecDeque};

    #[derive(Debug, Deserialize)]
    pub struct SideWalletQuery {
        /// Graph expansion depth (1-3 recommended)
        pub depth: Option<usize>,
        /// Minimum score (0.0-1.0)
        pub threshold: Option<f64>,
        /// Max results returned
        pub limit: Option<usize>,
        /// If true, fetch & ingest recent transactions first
        pub bootstrap: Option<bool>,
        /// Number of recent signatures to ingest when bootstrap=true
        pub bootstrap_limit: Option<u64>,
        /// How many days back to consider event-level evidence (transfer_events)
        pub lookback_days: Option<u32>,
    }

    #[derive(Debug, Clone)]
    struct SideWalletCandidate {
        address: String,
        score: f64,
        depth: usize,
        reasons: Vec<String>,
        tx_count: u32,
        total_sol: f64,
        total_token: u64,
        first_seen_epoch: u64,
        last_seen_epoch: u64,
        direction: String,
        shared_funders_count: u32,
        shared_counterparties_count: u32,
        shared_funders: Vec<String>,
        shared_counterparties: Vec<String>,
        behavioral_similarity: f64,
        temporal_overlap_ratio: f64,
        same_block_count: u32,
    }

    fn clamp01(x: f64) -> f64 {
        if x.is_nan() {
            return 0.0;
        }
        x.max(0.0).min(1.0)
    }

    fn edge_score(tx_count: u32, total_sol: f64, total_token: u64) -> f64 {
        // Lightweight heuristic score based on activity and volume.
        // Output is [0,1].
        let tx = (tx_count as f64 + 1.0).ln();
        let sol = (total_sol.abs() + 1.0).ln();
        let token = ((total_token as f64) / 1_000_000.0 + 1.0).ln();
        let raw = tx * 0.65 + sol * 0.30 + token * 0.05;
        clamp01(1.0 - (-raw / 3.0).exp())
    }

    fn recency_score(last_seen_epoch: u64) -> f64 {
        // Favor recent relationships (age decay ~30 days).
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if last_seen_epoch == 0 || last_seen_epoch > now {
            return 0.5;
        }

        let age_days = (now - last_seen_epoch) as f64 / 86_400.0;
        let decay = (-age_days / 30.0).exp();
        clamp01(0.15 + 0.85 * decay)
    }

    fn direction_label(current: &str, conn_from: &str, conn_to: &str) -> String {
        if current == conn_from {
            "outbound".to_string()
        } else if current == conn_to {
            "inbound".to_string()
        } else {
            "unknown".to_string()
        }
    }

    fn since_epoch_from_days(lookback_days: u32) -> u64 {
        let days = lookback_days.clamp(1, 365) as u64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(days.saturating_mul(86_400))
    }

    fn format_signal(wallet: &str, count: u64, last_seen_epoch: u64) -> String {
        if last_seen_epoch > 0 {
            format!("{} ({} events; last_seen={})", wallet, count, last_seen_epoch)
        } else {
            format!("{} ({} events)", wallet, count)
        }
    }

    fn compute_behavioral_similarity(
        profile_a: &BehavioralProfile,
        profile_b: &BehavioralProfile,
    ) -> f64 {
        // Similarity based on transaction patterns (0.0 - 1.0)
        
        // 1. Average SOL amount similarity (normalize by log scale)
        let avg_sol_sim = if profile_a.avg_sol_per_tx > 0.0 && profile_b.avg_sol_per_tx > 0.0 {
            let ratio = (profile_a.avg_sol_per_tx / profile_b.avg_sol_per_tx).max(profile_b.avg_sol_per_tx / profile_a.avg_sol_per_tx);
            let log_ratio = ratio.ln().abs();
            (-log_ratio / 2.0).exp() // Decay factor for differences
        } else {
            0.5
        };

        // 2. Transaction frequency similarity (tx per day)
        let freq_sim = if profile_a.avg_tx_per_day > 0.0 && profile_b.avg_tx_per_day > 0.0 {
            let ratio = (profile_a.avg_tx_per_day / profile_b.avg_tx_per_day).max(profile_b.avg_tx_per_day / profile_a.avg_tx_per_day);
            let log_ratio = ratio.ln().abs();
            (-log_ratio / 1.5).exp()
        } else {
            0.5
        };

        // 3. Most active hour similarity (time-of-day clustering)
        let hour_sim = match (profile_a.most_active_hour_utc, profile_b.most_active_hour_utc) {
            (Some(h_a), Some(h_b)) => {
                let diff = (h_a as i32 - h_b as i32).abs();
                // Hours are circular (0-23), so check both directions
                let circular_diff = diff.min(24 - diff);
                // Strong signal if within 2 hours, moderate if within 4 hours
                if circular_diff <= 2 {
                    1.0
                } else if circular_diff <= 4 {
                    0.7
                } else if circular_diff <= 8 {
                    0.4
                } else {
                    0.1
                }
            }
            _ => 0.3, // Unknown or one has no dominant hour
        };

        // Weighted combination
        let combined = (avg_sol_sim * 0.40) + (freq_sim * 0.35) + (hour_sim * 0.25);
        clamp01(combined)
    }

    async fn enrich_candidates_with_event_signals(
        state: &ApiState,
        main_wallet: &str,
        candidates: &mut [SideWalletCandidate],
        lookback_days: u32,
    ) {
        let since_epoch = since_epoch_from_days(lookback_days);

        // Precompute main wallet counterparties once.
        let main_counterparties = match state
            .db_manager
            .get_top_counterparties(main_wallet, Some(since_epoch), 80)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "Event evidence unavailable (get_top_counterparties failed): {}",
                    e
                );
                return;
            }
        };
        let main_set: HashSet<String> = main_counterparties
            .iter()
            .map(|s| s.wallet.clone())
            .collect();

        for c in candidates.iter_mut() {
            // Shared inbound funders: wallets that sent to both main and candidate.
            match state
                .db_manager
                .get_shared_inbound_senders(main_wallet, &c.address, Some(since_epoch), 3)
                .await
            {
                Ok(shared) => {
                    c.shared_funders_count = shared.len() as u32;
                    c.shared_funders = shared
                        .iter()
                        .map(|s| format_signal(&s.wallet, s.count, s.last_seen_epoch))
                        .collect();
                    for s in shared.iter().take(3) {
                        if c.reasons.len() < 8 {
                            c.reasons.push(format!(
                                "Shared inbound funder: {}",
                                format_signal(&s.wallet, s.count, s.last_seen_epoch)
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("shared funders query failed: {}", e);
                }
            }

            // Shared counterparties: intersection of top counterparties.
            match state
                .db_manager
                .get_top_counterparties(&c.address, Some(since_epoch), 80)
                .await
            {
                Ok(other) => {
                    let mut shared_wallets: Vec<String> = other
                        .iter()
                        .map(|s| s.wallet.clone())
                        .filter(|w| main_set.contains(w))
                        .collect();
                    shared_wallets.sort();
                    shared_wallets.dedup();

                    c.shared_counterparties_count = shared_wallets.len() as u32;
                    c.shared_counterparties = shared_wallets.iter().take(3).cloned().collect();

                    for w in shared_wallets.iter().take(3) {
                        if c.reasons.len() < 8 {
                            c.reasons
                                .push(format!("Shared counterparty: {}", w));
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("counterparty query failed: {}", e);
                }
            }

            // Score bump from higher-signal evidence (kept small).
            let bump = 0.06 * (c.shared_funders_count.min(3) as f64)
                + 0.03 * (c.shared_counterparties_count.min(5) as f64);
            c.score = clamp01(c.score + bump);
        }

        // Behavioral correlation: compare transaction patterns
        let main_profile = state
            .db_manager
            .get_behavioral_profile(main_wallet, Some(since_epoch))
            .await
            .ok()
            .flatten();

        if let Some(main_prof) = main_profile {
            for c in candidates.iter_mut() {
                match state
                    .db_manager
                    .get_behavioral_profile(&c.address, Some(since_epoch))
                    .await
                {
                    Ok(Some(cand_prof)) => {
                        let similarity = compute_behavioral_similarity(&main_prof, &cand_prof);
                        c.behavioral_similarity = similarity;

                        if similarity > 0.65 {
                            if c.reasons.len() < 8 {
                                c.reasons.push(format!(
                                    "Behavioral pattern match (similarity: {:.2})",
                                    similarity
                                ));
                            }
                            // Behavioral boost (15% weight as planned)
                            c.score = clamp01(c.score + similarity * 0.12);
                        }
                    }
                    Ok(None) => {
                        c.behavioral_similarity = 0.0;
                    }
                    Err(e) => {
                        tracing::debug!("behavioral profile query failed for {}: {}", c.address, e);
                        c.behavioral_similarity = 0.0;
                    }
                }
            }
        }

        // Temporal alignment: detect synchronized activity windows
        for c in candidates.iter_mut() {
            match state
                .db_manager
                .get_temporal_overlap(main_wallet, &c.address, Some(since_epoch), 5)
                .await
            {
                Ok(overlap) => {
                    c.temporal_overlap_ratio = overlap.overlap_ratio;
                    c.same_block_count = overlap.same_block_count;

                    // Temporal signals: same-block txs (strong) or high time overlap (moderate)
                    if overlap.same_block_count > 0 {
                        if c.reasons.len() < 8 {
                            c.reasons.push(format!(
                                "Same-block activity ({} shared blocks)",
                                overlap.same_block_count
                            ));
                        }
                        // Same-block is very strong signal
                        c.score = clamp01(c.score + 0.08 * (overlap.same_block_count.min(5) as f64 * 0.2));
                    } else if overlap.overlap_ratio > 0.15 {
                        if c.reasons.len() < 8 {
                            c.reasons.push(format!(
                                "Synchronized activity windows ({:.1}% overlap)",
                                overlap.overlap_ratio * 100.0
                            ));
                        }
                        // High temporal overlap
                        c.score = clamp01(c.score + overlap.overlap_ratio * 0.10);
                    }
                }
                Err(e) => {
                    tracing::debug!("temporal overlap query failed for {}: {}", c.address, e);
                    c.temporal_overlap_ratio = 0.0;
                    c.same_block_count = 0;
                }
            }
        }
    }

    fn build_reason(
        from: &str,
        to: &str,
        tx_count: u32,
        total_sol: f64,
        total_token: u64,
    ) -> String {
        if total_sol > 0.0 {
            format!(
                "Link: {} ↔ {} ({} tx, {:.4} SOL)",
                from, to, tx_count, total_sol
            )
        } else if total_token > 0 {
            format!(
                "Link: {} ↔ {} ({} tx, {} token units)",
                from, to, tx_count, total_token
            )
        } else {
            format!("Link: {} ↔ {} ({} tx)", from, to, tx_count)
        }
    }

    async fn compute_side_wallets(
        state: &ApiState,
        main_wallet: &str,
        max_depth: usize,
        threshold: f64,
        limit: usize,
        lookback_days: u32,
    ) -> Result<Vec<SideWalletCandidate>, String> {
        let max_depth = max_depth.clamp(1, 5);
        let threshold = clamp01(threshold);
        let limit = limit.clamp(1, 100);

        // BFS over wallet_relationships graph.
        let mut queue: VecDeque<(String, usize, f64)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut best: HashMap<String, SideWalletCandidate> = HashMap::new();

        queue.push_back((main_wallet.to_string(), 0, 1.0));
        visited.insert(main_wallet.to_string());

        while let Some((current, depth, parent_score)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let connections = state
                .db_manager
                .get_wallet_connections(&current)
                .await
                .map_err(|e| format!("Failed to get connections: {}", e))?;

            for conn in connections {
                let (from, to) = (&conn.from_wallet, &conn.to_wallet);
                let other = if from == &current { to } else { from };
                if other == main_wallet {
                    continue;
                }

                let mut s = edge_score(
                    conn.transaction_count,
                    conn.total_sol_transferred,
                    conn.total_token_transferred,
                );

                // Penalize very weak, single-touch relationships.
                if conn.transaction_count <= 1
                    && conn.total_sol_transferred.abs() < 0.01
                    && conn.total_token_transferred == 0
                {
                    s *= 0.35;
                }

                // Favor recent relationships.
                s *= recency_score(conn.last_seen_epoch);

                let combined = clamp01(parent_score * s * (0.85_f64).powi((depth as i32) + 1));
                if combined < threshold {
                    continue;
                }

                let dir = direction_label(&current, from, to);

                let reason = build_reason(
                    from,
                    to,
                    conn.transaction_count,
                    conn.total_sol_transferred,
                    conn.total_token_transferred,
                );

                let entry = best
                    .entry(other.to_string())
                    .or_insert_with(|| SideWalletCandidate {
                        address: other.to_string(),
                        score: combined,
                        depth: depth + 1,
                        reasons: Vec::new(),
                        tx_count: conn.transaction_count,
                        total_sol: conn.total_sol_transferred,
                        total_token: conn.total_token_transferred,
                        first_seen_epoch: conn.first_seen_epoch,
                        last_seen_epoch: conn.last_seen_epoch,
                        direction: dir.clone(),
                        shared_funders_count: 0,
                        shared_counterparties_count: 0,
                        shared_funders: Vec::new(),
                        shared_counterparties: Vec::new(),
                        behavioral_similarity: 0.0,
                        temporal_overlap_ratio: 0.0,
                        same_block_count: 0,
                    });

                // Keep best score, but also accumulate evidence.
                if combined > entry.score {
                    entry.score = combined;
                    entry.depth = depth + 1;
                    entry.tx_count = conn.transaction_count;
                    entry.total_sol = conn.total_sol_transferred;
                    entry.total_token = conn.total_token_transferred;
                    entry.first_seen_epoch = conn.first_seen_epoch;
                    entry.last_seen_epoch = conn.last_seen_epoch;
                    entry.direction = dir.clone();
                }
                if entry.reasons.len() < 5 {
                    if conn.last_seen_epoch > 0 {
                        entry
                            .reasons
                            .push(format!("{} ({}; last_seen={})", reason, dir, conn.last_seen_epoch));
                    } else {
                        entry.reasons.push(format!("{} ({})", reason, dir));
                    }
                }

                if !visited.contains(other) {
                    visited.insert(other.to_string());
                    queue.push_back((other.to_string(), depth + 1, combined));
                }
            }
        }

        let mut results: Vec<SideWalletCandidate> = best
            .into_values()
            .filter(|c| c.score >= threshold)
            .collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        // Enrich with higher-signal evidence from transfer_events when available.
        enrich_candidates_with_event_signals(state, main_wallet, &mut results, lookback_days).await;

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

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
                state
                    .cache
                    .cluster_cache
                    .set(cache_key.to_string(), response.clone());

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
            Ok(sigs) => HttpResponse::Ok().json(json!({
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
            })),
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
        query: web::Query<SideWalletQuery>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        let depth = query.depth.unwrap_or(2);
        let threshold = query.threshold.unwrap_or(0.10);
        let limit = query.limit.unwrap_or(15);
        let bootstrap = query.bootstrap.unwrap_or(true);
        let bootstrap_limit = query.bootstrap_limit.unwrap_or(25).min(100);
        let lookback_days = query.lookback_days.unwrap_or(30).clamp(1, 365);

        // Optional bootstrap: fetch recent signatures, parse transactions, and persist relationships.
        let mut ingested = 0usize;
        let mut bootstrap_signatures = 0usize;
        let mut parsed_ok = 0usize;
        let mut parsed_failed = 0usize;
        let mut persisted_failed = 0usize;
        let mut bootstrap_errors: Vec<String> = Vec::new();

        if bootstrap {
            match state
                .rpc_client
                .get_signatures(&wallet, bootstrap_limit)
                .await
            {
                Ok(sigs) => {
                    bootstrap_signatures = sigs.len();
                    let analytics =
                        TransferAnalytics::new(state.db_manager.clone(), state.redis_cache.clone());
                    let handler = state.transaction_handler.read().await;

                    for s in sigs {
                        match handler.process_transaction(&s.signature, None).await {
                            Ok(tx) => {
                                parsed_ok += 1;
                                match analytics.analyze_transaction(&tx).await {
                                    Ok(_) => {
                                        ingested += 1;
                                    }
                                    Err(e) => {
                                        persisted_failed += 1;
                                        if bootstrap_errors.len() < 3 {
                                            bootstrap_errors
                                                .push(format!("persist {}: {}", s.signature, e));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                parsed_failed += 1;
                                if bootstrap_errors.len() < 3 {
                                    bootstrap_errors.push(format!("parse {}: {}", s.signature, e));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Side-wallet bootstrap failed for {}: {}", wallet, e);
                    bootstrap_errors.push(format!("get_signatures: {}", e));
                }
            }
        }

        match compute_side_wallets(&state, &wallet, depth, threshold, limit, lookback_days).await {
            Ok(candidates) => {
                HttpResponse::Ok().json(json!({
                    "main_wallet": wallet,
                    "side_wallets": candidates.iter().map(|c| json!({
                        "address": c.address,
                        "score": c.score,
                        "depth": c.depth,
                        "tx_count": c.tx_count,
                        "total_sol": c.total_sol,
                        "total_token": c.total_token,
                        "first_seen_epoch": c.first_seen_epoch,
                        "last_seen_epoch": c.last_seen_epoch,
                        "direction": c.direction,
                        "shared_funders_count": c.shared_funders_count,
                        "shared_counterparties_count": c.shared_counterparties_count,
                        "shared_funders": c.shared_funders,
                        "shared_counterparties": c.shared_counterparties,
                        "behavioral_similarity": c.behavioral_similarity,
                        "temporal_overlap_ratio": c.temporal_overlap_ratio,
                        "same_block_count": c.same_block_count,
                        "reasons": c.reasons,
                    })).collect::<Vec<_>>(),
                    "confidence_threshold": threshold,
                    "analysis_depth": depth,
                    "lookback_days": lookback_days,
                    "bootstrap": bootstrap,
                    "bootstrap_ingested_transactions": ingested,
                    "bootstrap_signatures": bootstrap_signatures,
                    "bootstrap_parsed_ok": parsed_ok,
                    "bootstrap_parsed_failed": parsed_failed,
                    "bootstrap_persisted_failed": persisted_failed,
                    "bootstrap_errors": bootstrap_errors,
                    "message": if candidates.is_empty() {
                        "No side-wallet candidates yet. Increase bootstrap_limit or try again after ingesting more transactions."
                    } else {
                        "OK"
                    }
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": e
            })),
        }
    }

    /// Get wallet cluster
    /// Requires authentication when enabled
    pub async fn get_wallet_cluster(
        _auth: ApiKey, // Authentication required
        state: web::Data<ApiState>,
        address: web::Path<String>,
        query: web::Query<SideWalletQuery>,
    ) -> HttpResponse {
        let wallet = address.into_inner();

        let depth = query.depth.unwrap_or(2);
        let threshold = query.threshold.unwrap_or(0.10);
        let limit = query.limit.unwrap_or(30);
        let bootstrap = query.bootstrap.unwrap_or(true);
        let bootstrap_limit = query.bootstrap_limit.unwrap_or(25).min(100);
        let lookback_days = query.lookback_days.unwrap_or(30).clamp(1, 365);

        // Optional bootstrap (same as side-wallet endpoint)
        let mut ingested = 0usize;
        let mut bootstrap_signatures = 0usize;
        let mut parsed_ok = 0usize;
        let mut parsed_failed = 0usize;
        let mut persisted_failed = 0usize;
        let mut bootstrap_errors: Vec<String> = Vec::new();

        if bootstrap {
            match state
                .rpc_client
                .get_signatures(&wallet, bootstrap_limit)
                .await
            {
                Ok(sigs) => {
                    bootstrap_signatures = sigs.len();
                    let analytics =
                        TransferAnalytics::new(state.db_manager.clone(), state.redis_cache.clone());
                    let handler = state.transaction_handler.read().await;
                    for s in sigs {
                        match handler.process_transaction(&s.signature, None).await {
                            Ok(tx) => {
                                parsed_ok += 1;
                                match analytics.analyze_transaction(&tx).await {
                                    Ok(_) => {
                                        ingested += 1;
                                    }
                                    Err(e) => {
                                        persisted_failed += 1;
                                        if bootstrap_errors.len() < 3 {
                                            bootstrap_errors
                                                .push(format!("persist {}: {}", s.signature, e));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                parsed_failed += 1;
                                if bootstrap_errors.len() < 3 {
                                    bootstrap_errors.push(format!("parse {}: {}", s.signature, e));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Cluster bootstrap failed for {}: {}", wallet, e);
                    bootstrap_errors.push(format!("get_signatures: {}", e));
                }
            }
        }

        match compute_side_wallets(&state, &wallet, depth, threshold, limit, lookback_days).await {
            Ok(mut candidates) => {
                // Cluster includes the primary wallet plus the discovered candidates.
                let mut wallets = vec![json!({
                    "address": wallet,
                    "score": 1.0,
                    "depth": 0,
                    "reasons": ["Primary wallet"],
                })];

                let mut strength_sum = 0.0;
                for c in candidates.drain(..) {
                    strength_sum += c.score;
                    wallets.push(json!({
                        "address": c.address,
                        "score": c.score,
                        "depth": c.depth,
                        "tx_count": c.tx_count,
                        "total_sol": c.total_sol,
                        "total_token": c.total_token,
                        "first_seen_epoch": c.first_seen_epoch,
                        "last_seen_epoch": c.last_seen_epoch,
                        "direction": c.direction,
                        "shared_funders_count": c.shared_funders_count,
                        "shared_counterparties_count": c.shared_counterparties_count,
                        "shared_funders": c.shared_funders,
                        "shared_counterparties": c.shared_counterparties,
                        "behavioral_similarity": c.behavioral_similarity,
                        "reasons": c.reasons,
                    }));
                }
                let size = wallets.len();
                let avg_strength = if size > 1 {
                    strength_sum / ((size - 1) as f64)
                } else {
                    1.0
                };

                HttpResponse::Ok().json(json!({
                    "primary_wallet": wallet,
                    "cluster_size": size,
                    "wallets": wallets,
                    "connection_strength": avg_strength,
                    "analysis_depth": depth,
                    "confidence_threshold": threshold,
                    "lookback_days": lookback_days,
                    "bootstrap": bootstrap,
                    "bootstrap_ingested_transactions": ingested,
                    "bootstrap_signatures": bootstrap_signatures,
                    "bootstrap_parsed_ok": parsed_ok,
                    "bootstrap_parsed_failed": parsed_failed,
                    "bootstrap_persisted_failed": persisted_failed,
                    "bootstrap_errors": bootstrap_errors,
                    "message": if size <= 1 {
                        "No cluster expansion yet. Try increasing bootstrap_limit or lowering threshold."
                    } else {
                        "OK"
                    }
                }))
            }
            Err(e) => HttpResponse::InternalServerError().json(json!({
                "error": e
            })),
        }
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
            Ok(cluster) => HttpResponse::Ok().json(json!({
                "network": "solana",
                "cluster": "mainnet-beta",
                "active_validators": cluster.total_nodes,
                "network_health": "good",
                "tps": 400,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
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
            Ok(account) => HttpResponse::Ok().json(json!({
                "wallet": wallet,
                "balance_lamports": account.balance,
                "balance_sol": account.balance as f64 / 1_000_000_000.0,
                "owner": account.owner
            })),
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
            Ok(account) => HttpResponse::Ok().json(json!({
                "address": wallet,
                "balance_lamports": account.balance,
                "balance_sol": account.balance as f64 / 1_000_000_000.0,
                "owner": account.owner,
                "executable": account.executable,
                "rent_epoch": account.rent_epoch
            })),
            Err(e) => HttpResponse::NotFound().json(json!({
                "error": e.to_string()
            })),
        }
    }

    /// Get cluster info
    pub async fn get_cluster_info(state: web::Data<ApiState>) -> HttpResponse {
        match state.rpc_client.get_cluster_info().await {
            Ok(cluster) => HttpResponse::Ok().json(json!({
                "cluster": "mainnet-beta",
                "total_validators": cluster.total_nodes,
                "network_version": "1.18",
                "health": "operational"
            })),
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

mod modules;
mod core;
mod api;
mod database;
mod analysis;
mod graph;
mod cache;
mod middleware;
mod auth;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use core::config::Config;
use core::rpc_client::SolanaRpcClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("🚀 OnChain Beast - Solana Blockchain Analysis Engine");
    info!("Version: 0.1.0");
    
    // Load configuration from environment
    let config = Config::from_env();
    info!("📡 RPC Endpoint: {}", config.rpc_endpoint);
    
    // Initialize database
    let db = database::init_database().await?;
    let db = Arc::new(RwLock::new(db));
    
    info!("✅ Database initialized");
    
    // Initialize Solana RPC client
    let rpc_client = SolanaRpcClient::new(config.rpc_endpoint.clone());
    let rpc_client = Arc::new(rpc_client);
    
    // Health check
    match rpc_client.health_check().await {
        Ok(healthy) => {
            if healthy {
                info!("✅ Solana RPC connection healthy");
            } else {
                tracing::warn!("⚠️  Solana RPC connection unhealthy");
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to connect to Solana RPC: {}", e);
        }
    }
    
    // Get cluster info
    match rpc_client.get_cluster_info().await {
        Ok(cluster) => {
            info!("📊 Cluster Info: {} validator nodes active", cluster.total_nodes);
        }
        Err(e) => {
            tracing::warn!("Failed to get cluster info: {}", e);
        }
    }
    
    // Initialize analysis engine
    let analysis_engine = analysis::AnalysisEngine::new();
    let analysis_engine = Arc::new(RwLock::new(analysis_engine));
    
    info!("✅ Analysis engine initialized");
    
    // Initialize cache manager
    let cache_manager = cache::CacheManager::new();
    let cache_manager = Arc::new(cache_manager);
    
    info!("✅ Cache manager initialized");
    
    // Initialize authentication with API keys
    if config.enable_auth && !config.api_keys.is_empty() {
        auth::init_api_keys(config.api_keys.clone());
        info!("🔒 API authentication enabled ({} keys)", config.api_keys.len());
    } else {
        auth::init_api_keys(vec![]); // Empty = auth disabled
        tracing::warn!("⚠️  API authentication disabled - not recommended for production!");
    }
    info!("🚦 Rate limiting: {} requests/minute (unauthenticated)", config.rate_limit_per_minute);
    
    // Start REST API server
    let api_host = std::env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let api_port = std::env::var("API_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;
    
    info!("🌐 Starting REST API server on {}:{}", api_host, api_port);
    
    api::start_server(
        config,
        rpc_client,
        db,
        analysis_engine,
        cache_manager,
        &api_host,
        api_port,
    )
    .await?;
    
    Ok(())
}

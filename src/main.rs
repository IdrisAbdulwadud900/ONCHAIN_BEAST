use onchain_beast::api;
use onchain_beast::auth;
use onchain_beast::core::rpc_client::SolanaRpcClient;
use onchain_beast::storage::DatabaseManager;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let rpc_endpoint = std::env::var("SOLANA_RPC_ENDPOINT")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

    let database_url = std::env::var("DATABASE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        // Default to in-memory to keep setup friction low. For persistence, set DATABASE_URL to Postgres.
        .unwrap_or_else(|| "memory".to_string());

    // Optional API keys (comma-separated). If unset/empty, auth is disabled.
    let api_keys: Vec<String> = std::env::var("API_KEYS")
        .ok()
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    auth::init_api_keys(api_keys);

    let db_manager = Arc::new(DatabaseManager::new(&database_url).await?);
    db_manager.init_schema().await?;

    let rpc_client = Arc::new(SolanaRpcClient::new(rpc_endpoint));

    // Render (and some other PaaS) provide a required `PORT` env var. Prefer it if set.
    // When running locally, default to 127.0.0.1:8080 unless overridden via API_HOST/API_PORT.
    let render_port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());

    let api_host = std::env::var("API_HOST").unwrap_or_else(|_| {
        if render_port.is_some() {
            "0.0.0.0".to_string()
        } else {
            "127.0.0.1".to_string()
        }
    });

    let api_port = match render_port {
        Some(p) => p,
        None => std::env::var("API_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?,
    };

    api::start_server(rpc_client, db_manager, &api_host, api_port).await?;
    Ok(())
}

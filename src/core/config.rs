use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_endpoint: String,
    pub database_url: String,
    pub max_concurrent_requests: usize,
    pub cache_ttl_seconds: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            rpc_endpoint: env::var("SOLANA_RPC_ENDPOINT")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:onchain_beast.db".to_string()),
            max_concurrent_requests: env::var("MAX_CONCURRENT_REQUESTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            cache_ttl_seconds: env::var("CACHE_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
        }
    }
}

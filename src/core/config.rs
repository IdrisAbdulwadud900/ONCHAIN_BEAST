use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_endpoint: String,
    pub database_url: String,
    pub max_concurrent_requests: usize,
    pub cache_ttl_seconds: u64,
    pub api_keys: Vec<String>,
    pub rate_limit_per_minute: u32,
    pub enable_auth: bool,
}

impl Config {
    pub fn from_env() -> Self {
        // Parse API keys from comma-separated environment variable
        let api_keys = env::var("API_KEYS")
            .unwrap_or_else(|_| "demo-key-123,test-key-456".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

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
            api_keys,
            rate_limit_per_minute: env::var("RATE_LIMIT_PER_MINUTE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            enable_auth: env::var("ENABLE_AUTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
        }
    }
}

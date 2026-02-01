/// Price Oracle Types
use serde::{Deserialize, Serialize};

/// Token price data from Jupiter API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub id: String,
    #[serde(rename = "mintSymbol")]
    pub mint_symbol: Option<String>,
    #[serde(rename = "vsToken")]
    pub vs_token: String,
    #[serde(rename = "vsTokenSymbol")]
    pub vs_token_symbol: String,
    pub price: f64,
}

/// Jupiter price API response
#[derive(Debug, Deserialize)]
pub struct JupiterPriceResponse {
    pub data: std::collections::HashMap<String, PriceData>,
    #[serde(rename = "timeTaken")]
    pub time_taken: f64,
}

/// Price quote for a token
#[derive(Debug, Clone, Serialize)]
pub struct PriceQuote {
    pub token_mint: String,
    pub price_usd: f64,
    pub timestamp: i64,
    pub source: String,
}

/// Cached token price
#[derive(Debug, Clone)]
pub struct TokenPrice {
    pub mint: String,
    pub price_usd: f64,
    pub last_updated: i64,
}

impl TokenPrice {
    pub fn is_stale(&self, max_age_secs: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        now - self.last_updated > max_age_secs
    }
}

/// Standard quote tokens
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

/// Known stablecoin mints (assumed $1.00)
pub const STABLECOINS: &[&str] = &[
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", // USDT
    "7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs", // USDT (Wormhole)
];

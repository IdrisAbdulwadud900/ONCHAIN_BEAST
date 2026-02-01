/// Jupiter Price Oracle
/// Fetches token prices from Jupiter Price API v2
use crate::core::errors::{BeastError, BeastResult};
use crate::price::types::{JupiterPriceResponse, PriceQuote, TokenPrice, STABLECOINS};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const JUPITER_PRICE_API: &str = "https://api.jup.ag/price/v2";

pub struct JupiterPriceOracle {
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, TokenPrice>>>,
    cache_ttl_secs: i64,
}

impl JupiterPriceOracle {
    pub fn new(cache_ttl_secs: i64) -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl_secs,
        }
    }

    /// Get price for a single token
    pub async fn get_price(&self, token_mint: &str) -> BeastResult<PriceQuote> {
        // Check if it's a stablecoin
        if STABLECOINS.contains(&token_mint) {
            return Ok(PriceQuote {
                token_mint: token_mint.to_string(),
                price_usd: 1.0,
                timestamp: chrono::Utc::now().timestamp(),
                source: "stablecoin".to_string(),
            });
        }

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(token_mint) {
                if !cached.is_stale(self.cache_ttl_secs) {
                    return Ok(PriceQuote {
                        token_mint: token_mint.to_string(),
                        price_usd: cached.price_usd,
                        timestamp: cached.last_updated,
                        source: "cache".to_string(),
                    });
                }
            }
        }

        // Fetch from Jupiter
        self.fetch_and_cache_price(token_mint).await
    }

    /// Get prices for multiple tokens in one request
    pub async fn get_prices(&self, token_mints: &[String]) -> BeastResult<Vec<PriceQuote>> {
        if token_mints.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut to_fetch = Vec::new();

        // Check cache and identify stablecoins
        for mint in token_mints {
            if STABLECOINS.contains(&mint.as_str()) {
                results.push(PriceQuote {
                    token_mint: mint.clone(),
                    price_usd: 1.0,
                    timestamp: chrono::Utc::now().timestamp(),
                    source: "stablecoin".to_string(),
                });
            } else {
                let cache = self.cache.read().await;
                if let Some(cached) = cache.get(mint) {
                    if !cached.is_stale(self.cache_ttl_secs) {
                        results.push(PriceQuote {
                            token_mint: mint.clone(),
                            price_usd: cached.price_usd,
                            timestamp: cached.last_updated,
                            source: "cache".to_string(),
                        });
                        continue;
                    }
                }
                to_fetch.push(mint.clone());
            }
        }

        // Fetch remaining from Jupiter
        if !to_fetch.is_empty() {
            let fetched = self.fetch_and_cache_prices(&to_fetch).await?;
            results.extend(fetched);
        }

        Ok(results)
    }

    /// Fetch price from Jupiter and cache it
    async fn fetch_and_cache_price(&self, token_mint: &str) -> BeastResult<PriceQuote> {
        let url = format!("{}?ids={}", JUPITER_PRICE_API, token_mint);

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| BeastError::NetworkError(format!("Jupiter API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(BeastError::NetworkError(format!(
                "Jupiter API returned status: {}",
                response.status()
            )));
        }

        let jupiter_response: JupiterPriceResponse = response
            .json::<JupiterPriceResponse>()
            .await
            .map_err(|e| BeastError::ParseError(format!("Failed to parse Jupiter response: {}", e)))?;

        let price_data = jupiter_response
            .data
            .get(token_mint)
            .ok_or_else(|| BeastError::NotFound(format!("Price not found for token: {}", token_mint)))?;

        let timestamp = chrono::Utc::now().timestamp();

        // Cache the price
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                token_mint.to_string(),
                TokenPrice {
                    mint: token_mint.to_string(),
                    price_usd: price_data.price,
                    last_updated: timestamp,
                },
            );
        }

        Ok(PriceQuote {
            token_mint: token_mint.to_string(),
            price_usd: price_data.price,
            timestamp,
            source: "jupiter".to_string(),
        })
    }

    /// Fetch multiple prices from Jupiter
    async fn fetch_and_cache_prices(&self, token_mints: &[String]) -> BeastResult<Vec<PriceQuote>> {
        let ids = token_mints.join(",");
        let url = format!("{}?ids={}", JUPITER_PRICE_API, ids);

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| BeastError::NetworkError(format!("Jupiter API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(BeastError::NetworkError(format!(
                "Jupiter API returned status: {}",
                response.status()
            )));
        }

        let jupiter_response: JupiterPriceResponse = response
            .json::<JupiterPriceResponse>()
            .await
            .map_err(|e| BeastError::ParseError(format!("Failed to parse Jupiter response: {}", e)))?;

        let timestamp = chrono::Utc::now().timestamp();
        let mut results = Vec::new();

        // Cache all prices
        {
            let mut cache = self.cache.write().await;
            for (mint, price_data) in jupiter_response.data {
                cache.insert(
                    mint.clone(),
                    TokenPrice {
                        mint: mint.clone(),
                        price_usd: price_data.price,
                        last_updated: timestamp,
                    },
                );

                results.push(PriceQuote {
                    token_mint: mint,
                    price_usd: price_data.price,
                    timestamp,
                    source: "jupiter".to_string(),
                });
            }
        }

        Ok(results)
    }

    /// Get historical price (if available in cache, otherwise fetch current)
    pub async fn get_price_at(&self, token_mint: &str, _timestamp: i64) -> BeastResult<PriceQuote> {
        // For now, return current price
        // TODO: Implement historical price database lookup
        self.get_price(token_mint).await
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total = cache.len();
        let now = chrono::Utc::now().timestamp();
        let stale = cache
            .values()
            .filter(|p| now - p.last_updated > self.cache_ttl_secs)
            .count();
        (total, stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stablecoin_price() {
        let oracle = JupiterPriceOracle::new(300);
        let price = oracle.get_price("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").await.unwrap();
        assert_eq!(price.price_usd, 1.0);
        assert_eq!(price.source, "stablecoin");
    }

    #[tokio::test]
    async fn test_sol_price() {
        let oracle = JupiterPriceOracle::new(300);
        let price = oracle.get_price("So11111111111111111111111111111111111111112").await;
        
        // May fail if API is down, so just check it doesn't panic
        if let Ok(p) = price {
            assert!(p.price_usd > 0.0);
            println!("SOL price: ${}", p.price_usd);
        }
    }
}

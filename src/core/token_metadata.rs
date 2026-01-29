// Token Metadata Service
// Fetches and caches SPL token information from Solana blockchain

use crate::core::errors::{BeastError, BeastResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Token metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    /// Token mint address
    pub mint: String,

    /// Token symbol (e.g., "USDC", "SOL", "BONK")
    pub symbol: String,

    /// Token name (e.g., "USD Coin", "Solana", "Bonk")
    pub name: String,

    /// Number of decimal places
    pub decimals: u8,

    /// Token logo URI (optional)
    pub logo_uri: Option<String>,

    /// Token description (optional)
    pub description: Option<String>,

    /// Whether this is a verified token
    pub verified: bool,

    /// Total supply (optional)
    pub supply: Option<u64>,

    /// Timestamp when metadata was fetched
    pub fetched_at: u64,
}

/// Token account information
#[derive(Debug, Deserialize)]
struct TokenAccountData {
    #[serde(rename = "mint")]
    pub mint: String,

    #[serde(rename = "owner")]
    pub owner: String,

    #[serde(rename = "tokenAmount")]
    pub token_amount: TokenAmount,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    pub amount: String,
    pub decimals: u8,
    #[serde(rename = "uiAmount")]
    pub ui_amount: Option<f64>,
}

/// Mint account information
#[derive(Debug, Deserialize)]
struct MintAccountData {
    pub decimals: u8,

    #[serde(rename = "supply")]
    pub supply: String,

    #[serde(rename = "isInitialized")]
    pub is_initialized: bool,

    #[serde(rename = "freezeAuthority")]
    pub freeze_authority: Option<String>,

    #[serde(rename = "mintAuthority")]
    pub mint_authority: Option<String>,
}

/// Token Metadata Service
/// Fetches token information from Solana blockchain and caches it
pub struct TokenMetadataService {
    /// RPC endpoint URL
    rpc_url: String,

    /// HTTP client for RPC requests
    http_client: reqwest::Client,

    /// In-memory cache of token metadata
    cache: Arc<RwLock<HashMap<String, TokenMetadata>>>,

    /// Cache TTL in seconds (default: 1 hour)
    cache_ttl: u64,
}

impl TokenMetadataService {
    /// Create a new TokenMetadataService
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            http_client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: 3600, // 1 hour
        }
    }

    /// Create with custom cache TTL
    pub fn with_cache_ttl(rpc_url: String, cache_ttl: u64) -> Self {
        Self {
            rpc_url,
            http_client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }

    /// Get token metadata for a mint address
    /// Checks cache first, fetches from blockchain if not cached or expired
    pub async fn get_token_metadata(&self, mint: &str) -> BeastResult<TokenMetadata> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(metadata) = cache.get(mint) {
                // Check if cache entry is still valid
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                if now - metadata.fetched_at < self.cache_ttl {
                    return Ok(metadata.clone());
                }
            }
        }

        // Fetch from blockchain
        let metadata = self.fetch_token_metadata(mint).await?;

        // Cache it
        {
            let mut cache = self.cache.write().await;
            cache.insert(mint.to_string(), metadata.clone());
        }

        Ok(metadata)
    }

    /// Get multiple token metadata in batch
    pub async fn get_token_metadata_batch(
        &self,
        mints: &[String],
    ) -> BeastResult<HashMap<String, TokenMetadata>> {
        let mut result = HashMap::new();

        for mint in mints {
            match self.get_token_metadata(mint).await {
                Ok(metadata) => {
                    result.insert(mint.clone(), metadata);
                }
                Err(e) => {
                    eprintln!("Failed to fetch metadata for {}: {:?}", mint, e);
                    // Continue with other mints
                }
            }
        }

        Ok(result)
    }

    /// Fetch token metadata from Solana blockchain
    async fn fetch_token_metadata(&self, mint: &str) -> BeastResult<TokenMetadata> {
        // Fetch mint account data
        let account_info = self.fetch_account_info(mint).await?;

        // Parse mint data
        let mint_data = self.parse_mint_account(&account_info)?;

        // Try to fetch metadata from Metaplex (optional)
        let (symbol, name, logo_uri) =
            self.fetch_metaplex_metadata(mint)
                .await
                .unwrap_or_else(|_| {
                    (
                        self.generate_symbol_from_mint(mint),
                        format!("Unknown Token ({}...)", &mint[..8]),
                        None,
                    )
                });

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(TokenMetadata {
            mint: mint.to_string(),
            symbol,
            name,
            decimals: mint_data.decimals,
            logo_uri,
            description: None,
            verified: false, // Can be enhanced with token list verification
            supply: mint_data.supply.parse().ok(),
            fetched_at: now,
        })
    }

    /// Fetch account info from blockchain
    async fn fetch_account_info(&self, address: &str) -> BeastResult<serde_json::Value> {
        let params = serde_json::json!([
            address,
            {
                "encoding": "jsonParsed"
            }
        ]);

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": params
        });

        let response: serde_json::Value = self
            .http_client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BeastError::RpcError(format!("Failed to fetch account info: {}", e)))?
            .json()
            .await
            .map_err(|e| BeastError::ParseError(format!("Failed to parse response: {}", e)))?;

        response
            .get("result")
            .and_then(|r| r.get("value"))
            .ok_or_else(|| BeastError::ParseError("Failed to get account info".to_string()))
            .map(|v| v.clone())
    }

    /// Parse mint account data
    fn parse_mint_account(&self, account_info: &serde_json::Value) -> BeastResult<MintAccountData> {
        let data = account_info
            .get("data")
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))
            .ok_or_else(|| BeastError::ParseError("Invalid mint account data".to_string()))?;

        serde_json::from_value(data.clone())
            .map_err(|e| BeastError::ParseError(format!("Failed to parse mint data: {}", e)))
    }

    /// Fetch Metaplex metadata (token name, symbol, URI)
    async fn fetch_metaplex_metadata(
        &self,
        mint: &str,
    ) -> BeastResult<(String, String, Option<String>)> {
        // Find Metaplex metadata PDA
        let metadata_address = self.find_metadata_address(mint)?;

        // Fetch metadata account
        let account_info = match self.fetch_account_info(&metadata_address).await {
            Ok(info) => info,
            Err(_) => {
                return Err(BeastError::NotFound(
                    "Metaplex metadata not found".to_string(),
                ))
            }
        };

        // Parse metadata
        let data = account_info
            .get("data")
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))
            .ok_or_else(|| BeastError::ParseError("Invalid metadata structure".to_string()))?;

        let symbol = data
            .get("symbol")
            .and_then(|s| s.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        let name = data
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown Token")
            .to_string();

        let uri = data
            .get("uri")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string());

        Ok((symbol, name, uri))
    }

    /// Find Metaplex metadata PDA address for a mint
    fn find_metadata_address(&self, _mint: &str) -> BeastResult<String> {
        // Metaplex metadata program ID
        const METADATA_PROGRAM_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

        // This is a simplified version - in production, you'd use proper PDA derivation
        // For now, we'll use the standard Metaplex PDA formula
        // PDA = findProgramAddress([b"metadata", metadata_program_id, mint_pubkey])

        // Simplified: construct the expected metadata address
        // In a real implementation, use solana_sdk::pubkey::Pubkey::find_program_address

        // For now, return an error to fall back to basic metadata
        Err(BeastError::NotFound(
            "PDA derivation not implemented".to_string(),
        ))
    }

    /// Generate a simple symbol from mint address (fallback)
    fn generate_symbol_from_mint(&self, mint: &str) -> String {
        // Take first 4 chars of mint for symbol
        let short = &mint[..4.min(mint.len())];
        format!("{}...", short.to_uppercase())
    }

    /// Get token account owner
    pub async fn get_token_account_owner(&self, token_account: &str) -> BeastResult<String> {
        let account_info = self.fetch_account_info(token_account).await?;

        let owner = account_info
            .get("data")
            .and_then(|d| d.get("parsed"))
            .and_then(|p| p.get("info"))
            .and_then(|i| i.get("owner"))
            .and_then(|o| o.as_str())
            .ok_or_else(|| {
                BeastError::ParseError("Failed to get token account owner".to_string())
            })?;

        Ok(owner.to_string())
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache size
    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Preload well-known tokens into cache
    pub async fn preload_common_tokens(&self) {
        let common_tokens = vec![
            // USDC
            (
                "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
                "USDC",
                "USD Coin",
                6,
            ),
            // USDT
            (
                "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
                "USDT",
                "Tether USD",
                6,
            ),
            // SOL (wrapped)
            (
                "So11111111111111111111111111111111111111112",
                "SOL",
                "Wrapped SOL",
                9,
            ),
            // BONK
            (
                "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
                "BONK",
                "Bonk",
                5,
            ),
            // RAY
            (
                "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
                "RAY",
                "Raydium",
                6,
            ),
            // ORCA
            (
                "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE",
                "ORCA",
                "Orca",
                6,
            ),
        ];

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cache = self.cache.write().await;
        for (mint, symbol, name, decimals) in common_tokens {
            cache.insert(
                mint.to_string(),
                TokenMetadata {
                    mint: mint.to_string(),
                    symbol: symbol.to_string(),
                    name: name.to_string(),
                    decimals,
                    logo_uri: None,
                    description: None,
                    verified: true,
                    supply: None,
                    fetched_at: now,
                },
            );
        }
    }
}

impl Clone for TokenMetadataService {
    fn clone(&self) -> Self {
        Self {
            rpc_url: self.rpc_url.clone(),
            http_client: self.http_client.clone(),
            cache: Arc::clone(&self.cache),
            cache_ttl: self.cache_ttl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_symbol_from_mint() {
        let service = TokenMetadataService::new("https://api.mainnet-beta.solana.com".to_string());
        let symbol =
            service.generate_symbol_from_mint("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert_eq!(symbol, "EPJF...");
    }

    #[tokio::test]
    async fn test_cache_preload() {
        let service = TokenMetadataService::new("https://api.mainnet-beta.solana.com".to_string());

        // Cache should be empty
        assert_eq!(service.cache_size().await, 0);

        // Preload common tokens
        service.preload_common_tokens().await;

        // Cache should have common tokens
        assert!(service.cache_size().await > 0);

        // Should be able to get USDC
        let usdc = service
            .get_token_metadata("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
            .await;
        assert!(usdc.is_ok());
        assert_eq!(usdc.unwrap().symbol, "USDC");
    }
}

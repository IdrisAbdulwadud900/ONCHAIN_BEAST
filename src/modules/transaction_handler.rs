/// Transaction Handler Module
/// Integrates RPC client with enhanced transaction parser for real data processing

use crate::core::{SolanaRpcClient, EnhancedTransactionParser, EnhancedTransaction};
use crate::core::errors::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct TransactionHandler {
    rpc_client: Arc<SolanaRpcClient>,
    parser: EnhancedTransactionParser,
    /// Cache for parsed transactions
    cache: Arc<RwLock<HashMap<String, EnhancedTransaction>>>,
}

impl TransactionHandler {
    pub fn new(rpc_client: Arc<SolanaRpcClient>) -> Self {
        TransactionHandler {
            rpc_client,
            parser: EnhancedTransactionParser::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetch and parse a single transaction with full transfer extraction
    pub async fn process_transaction(
        &self,
        signature: &str,
        _commitment: Option<&str>,
    ) -> Result<EnhancedTransaction> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(signature) {
                return Ok(cached.clone());
            }
        }

        // Fetch from RPC
        tracing::info!("Fetching transaction: {}", signature);
        
        let response = self.rpc_client.get_transaction(signature).await?;

        // Parse the transaction using enhanced parser
        let parsed = self.parser.parse(&response.raw_data, signature.to_string())?;

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(signature.to_string(), parsed.clone());
        }

        Ok(parsed)
    }

    /// Fetch and parse wallet's transaction history
    pub async fn process_wallet_transactions(
        &self,
        wallet: &str,
        limit: usize,
    ) -> Result<Vec<EnhancedTransaction>> {
        tracing::info!("Processing {} transactions for wallet: {}", limit, wallet);

        // Get transaction signatures
        let signatures = self.rpc_client.get_signatures(wallet, limit as u64).await?;

        // Parse each transaction
        let mut results = Vec::new();
        for sig_obj in signatures {
            // sig_obj is already a TransactionSignature struct
            let sig = &sig_obj.signature;
            match self.process_transaction(sig, None).await {
                Ok(parsed) => results.push(parsed),
                Err(e) => {
                    tracing::warn!("Failed to parse transaction {}: {}", sig, e);
                    // Continue processing other transactions
                }
            }
        }

        Ok(results)
    }

    /// Batch process multiple transactions (more efficient)
    pub async fn process_transactions_batch(
        &self,
        signatures: Vec<String>,
    ) -> Result<Vec<EnhancedTransaction>> {
        tracing::info!("Processing batch of {} transactions", signatures.len());

        let mut results = Vec::new();
        
        // Process in parallel chunks (8 at a time to avoid rate limiting)
        for chunk in signatures.chunks(8) {
            let mut futures = Vec::new();
            
            for sig in chunk {
                let sig = sig.clone();
                let this = self.clone();
                futures.push(async move {
                    this.process_transaction(&sig, None).await
                });
            }

            // Wait for chunk to complete
            for future in futures {
                if let Ok(parsed) = future.await {
                    results.push(parsed);
                }
            }
        }

        Ok(results)
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
}

impl Clone for TransactionHandler {
    fn clone(&self) -> Self {
        TransactionHandler {
            rpc_client: Arc::clone(&self.rpc_client),
            parser: EnhancedTransactionParser::new(),
            cache: Arc::clone(&self.cache),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_creation() {
        let rpc = Arc::new(SolanaRpcClient::new("https://api.mainnet-beta.solana.com"));
        let handler = TransactionHandler::new(rpc);
        assert_eq!(handler.cache_size().await, 0);
    }
}

use crate::core::errors::Result;
/// Transaction Handler Module
/// Integrates RPC client with enhanced transaction parser for real data processing
use crate::core::{
    EnhancedTransaction, EnhancedTransactionParser, SolanaRpcClient, TokenMetadataService,
};
use crate::metrics::{Timer, PARSE_DURATION, SOL_TRANSFERS, TOKEN_TRANSFERS, TRANSACTIONS_PARSED};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct TransactionHandler {
    rpc_client: Arc<SolanaRpcClient>,
    parser: EnhancedTransactionParser,
    token_metadata: TokenMetadataService,
    /// Cache for parsed transactions
    cache: Arc<RwLock<HashMap<String, EnhancedTransaction>>>,
}

impl TransactionHandler {
    pub fn new(rpc_client: Arc<SolanaRpcClient>, rpc_url: String) -> Self {
        let token_metadata = TokenMetadataService::new(rpc_url);
        TransactionHandler {
            rpc_client,
            parser: EnhancedTransactionParser::new(),
            token_metadata,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetch and parse a single transaction with full transfer extraction
    pub async fn process_transaction(
        &self,
        signature: &str,
        _commitment: Option<&str>,
    ) -> Result<EnhancedTransaction> {
        let timer = Timer::new();

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
        let parsed = self
            .parser
            .parse(&response.raw_data, signature.to_string())?;

        // Track metrics
        TRANSACTIONS_PARSED.inc();
        for _ in 0..parsed.sol_transfers.len() {
            SOL_TRANSFERS.inc();
        }
        for _ in 0..parsed.token_transfers.len() {
            TOKEN_TRANSFERS.inc();
        }
        PARSE_DURATION.observe(timer.elapsed_secs());

        // Log transfer summary
        if !parsed.sol_transfers.is_empty() || !parsed.token_transfers.is_empty() {
            tracing::debug!(
                "Transaction {}: {} SOL transfers, {} token transfers",
                signature,
                parsed.sol_transfers.len(),
                parsed.token_transfers.len()
            );
        }

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
                futures.push(async move { this.process_transaction(&sig, None).await });
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

    /// Enrich transaction with token metadata
    pub async fn enrich_with_token_metadata(
        &self,
        mut transaction: EnhancedTransaction,
    ) -> Result<EnhancedTransaction> {
        // Collect all unique mints from token transfers
        let mints: Vec<String> = transaction
            .token_transfers
            .iter()
            .map(|t| t.mint.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if mints.is_empty() {
            return Ok(transaction);
        }

        // Fetch metadata for all mints
        let metadata_map: std::collections::HashMap<String, crate::core::TokenMetadata> =
            self.token_metadata.get_token_metadata_batch(&mints).await?;

        // Enrich each token transfer
        for transfer in &mut transaction.token_transfers {
            if let Some(metadata) = metadata_map.get(&transfer.mint) {
                transfer.token_symbol = Some(metadata.symbol.clone());
                transfer.token_name = Some(metadata.name.clone());
                transfer.verified = Some(metadata.verified);

                // Update decimals if we got them from metadata
                if transfer.decimals == 0 && metadata.decimals > 0 {
                    transfer.decimals = metadata.decimals;
                    // Recalculate UI amount with correct decimals
                    transfer.amount_ui =
                        transfer.amount as f64 / 10_u64.pow(metadata.decimals as u32) as f64;
                }
            }
        }

        Ok(transaction)
    }

    /// Preload common token metadata into cache
    pub async fn preload_token_metadata(&self) {
        self.token_metadata.preload_common_tokens().await;
    }
}

impl Clone for TransactionHandler {
    fn clone(&self) -> Self {
        TransactionHandler {
            rpc_client: Arc::clone(&self.rpc_client),
            parser: EnhancedTransactionParser::new(),
            token_metadata: self.token_metadata.clone(),
            cache: Arc::clone(&self.cache),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_creation() {
        let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
        let rpc = Arc::new(SolanaRpcClient::new(rpc_url.clone()));
        let handler = TransactionHandler::new(rpc, rpc_url);
        assert_eq!(handler.cache_size().await, 0);
    }
}

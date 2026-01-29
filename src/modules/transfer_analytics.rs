use crate::core::errors::BeastResult;
/// Transfer Analytics Service
/// Provides analytics and persistence for SOL and token transfers
use crate::core::{EnhancedTransaction, SolTransfer, TokenTransfer};
use crate::storage::{keys, DatabaseManager, RedisCache};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Transfer analytics aggregator
pub struct TransferAnalytics {
    db_manager: Arc<DatabaseManager>,
    redis_cache: Arc<RedisCache>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSummary {
    pub total_sol_transferred: f64,
    pub total_token_transfers: usize,
    pub unique_senders: usize,
    pub unique_receivers: usize,
    pub largest_sol_transfer: f64,
    pub most_active_wallet: String,
    pub transfer_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransferStats {
    pub wallet: String,
    pub total_sent_sol: f64,
    pub total_received_sol: f64,
    pub total_sent_tokens: usize,
    pub total_received_tokens: usize,
    pub unique_counterparties: usize,
    pub transaction_count: usize,
}

impl TransferAnalytics {
    pub fn new(db_manager: Arc<DatabaseManager>, redis_cache: Arc<RedisCache>) -> Self {
        Self {
            db_manager,
            redis_cache,
        }
    }

    /// Analyze transfers from a transaction and store them
    pub async fn analyze_transaction(&self, tx: &EnhancedTransaction) -> BeastResult<()> {
        // Store the transaction in database
        self.db_manager.store_transaction(tx).await?;

        // Process SOL transfers
        for transfer in &tx.sol_transfers {
            self.store_sol_transfer(tx, transfer).await?;
        }

        // Process token transfers
        for transfer in &tx.token_transfers {
            self.store_token_transfer(tx, transfer).await?;
        }

        Ok(())
    }

    /// Store SOL transfer relationship
    async fn store_sol_transfer(
        &self,
        _tx: &EnhancedTransaction,
        transfer: &SolTransfer,
    ) -> BeastResult<()> {
        // Update wallet relationship in database
        self.db_manager
            .store_wallet_relationship(
                &transfer.from,
                &transfer.to,
                transfer.amount_sol,
                0, // No token amount for SOL transfers
            )
            .await?;

        // Invalidate wallet stats cache
        self.redis_cache
            .delete(&keys::wallet_analysis(&transfer.from))
            .await
            .ok();
        self.redis_cache
            .delete(&keys::wallet_analysis(&transfer.to))
            .await
            .ok();

        Ok(())
    }

    /// Store token transfer relationship
    async fn store_token_transfer(
        &self,
        _tx: &EnhancedTransaction,
        transfer: &TokenTransfer,
    ) -> BeastResult<()> {
        // Update wallet relationship in database
        if let (Some(from), Some(to)) = (&transfer.from_owner, &transfer.to_owner) {
            self.db_manager
                .store_wallet_relationship(
                    from,
                    to,
                    0.0, // No SOL amount for token transfers
                    transfer.amount,
                )
                .await?;
        }

        Ok(())
    }

    /// Get transfer summary for a transaction
    pub fn summarize_transaction(&self, tx: &EnhancedTransaction) -> TransferSummary {
        let mut senders = std::collections::HashSet::new();
        let mut receivers = std::collections::HashSet::new();
        let mut wallet_activity: HashMap<String, usize> = HashMap::new();

        let total_sol: f64 = tx.sol_transfers.iter().map(|t| t.amount_sol).sum();
        let largest_sol = tx
            .sol_transfers
            .iter()
            .map(|t| t.amount_sol)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        // Track unique wallets from SOL transfers
        for transfer in &tx.sol_transfers {
            senders.insert(&transfer.from);
            receivers.insert(&transfer.to);
            *wallet_activity.entry(transfer.from.clone()).or_insert(0) += 1;
            *wallet_activity.entry(transfer.to.clone()).or_insert(0) += 1;
        }

        // Track unique wallets from token transfers
        for transfer in &tx.token_transfers {
            if let Some(from) = &transfer.from_owner {
                senders.insert(from);
                *wallet_activity.entry(from.clone()).or_insert(0) += 1;
            }
            if let Some(to) = &transfer.to_owner {
                receivers.insert(to);
                *wallet_activity.entry(to.clone()).or_insert(0) += 1;
            }
        }

        // Find most active wallet
        let most_active = wallet_activity
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(wallet, _)| wallet.clone())
            .unwrap_or_else(|| "N/A".to_string());

        TransferSummary {
            total_sol_transferred: total_sol,
            total_token_transfers: tx.token_transfers.len(),
            unique_senders: senders.len(),
            unique_receivers: receivers.len(),
            largest_sol_transfer: largest_sol,
            most_active_wallet: most_active,
            transfer_count: tx.sol_transfers.len() + tx.token_transfers.len(),
        }
    }

    /// Get wallet transfer statistics
    pub async fn get_wallet_stats(&self, wallet: &str) -> BeastResult<WalletTransferStats> {
        // Check cache first
        let cache_key = format!("wallet_stats:{}", wallet);
        if let Ok(Some(cached)) = self
            .redis_cache
            .get::<WalletTransferStats>(&cache_key)
            .await
        {
            return Ok(cached);
        }

        // Get connections from database
        let connections = self.db_manager.get_wallet_connections(wallet).await?;

        let mut total_sent_sol = 0.0;
        let mut total_received_sol = 0.0;
        let mut total_sent_tokens = 0;
        let mut total_received_tokens = 0;
        let mut counterparties = std::collections::HashSet::new();

        for conn in &connections {
            if conn.from_wallet == wallet {
                total_sent_sol += conn.total_sol_transferred;
                total_sent_tokens += conn.total_token_transferred as usize;
                counterparties.insert(&conn.to_wallet);
            } else {
                total_received_sol += conn.total_sol_transferred;
                total_received_tokens += conn.total_token_transferred as usize;
                counterparties.insert(&conn.from_wallet);
            }
        }

        let stats = WalletTransferStats {
            wallet: wallet.to_string(),
            total_sent_sol,
            total_received_sol,
            total_sent_tokens,
            total_received_tokens,
            unique_counterparties: counterparties.len(),
            transaction_count: connections.len(),
        };

        // Cache for 5 minutes
        self.redis_cache
            .set_with_ttl(&cache_key, &stats, 300)
            .await
            .ok();

        Ok(stats)
    }

    /// Get transfer summary from cache or compute
    pub async fn get_or_compute_summary(
        &self,
        tx: &EnhancedTransaction,
    ) -> BeastResult<TransferSummary> {
        let cache_key = format!("tx_summary:{}", tx.signature);

        // Check cache
        if let Ok(Some(cached)) = self.redis_cache.get::<TransferSummary>(&cache_key).await {
            return Ok(cached);
        }

        // Compute summary
        let summary = self.summarize_transaction(tx);

        // Cache for 1 hour
        self.redis_cache
            .set_with_ttl(&cache_key, &summary, 3600)
            .await
            .ok();

        Ok(summary)
    }

    /// Batch analyze multiple transactions
    pub async fn batch_analyze(
        &self,
        transactions: &[EnhancedTransaction],
    ) -> BeastResult<Vec<TransferSummary>> {
        let mut summaries = Vec::new();

        for tx in transactions {
            // Store in database
            self.analyze_transaction(tx).await.ok(); // Don't fail entire batch on one error

            // Compute summary
            let summary = self.summarize_transaction(tx);
            summaries.push(summary);
        }

        Ok(summaries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_summary() {
        // Test will be implemented when we have test fixtures
    }
}

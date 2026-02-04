use crate::core::errors::BeastResult;
use crate::core::{EnhancedTransaction, SolTransfer, TokenTransfer};
use crate::storage::DatabaseManager;
use std::sync::Arc;

/// Transfer ingestion: persists event-level transfers and relationship edges.
pub struct TransferAnalytics {
    db_manager: Arc<DatabaseManager>,
}

impl TransferAnalytics {
    pub fn new(db_manager: Arc<DatabaseManager>) -> Self {
        Self { db_manager }
    }

    /// Analyze transfers from a transaction and store them.
    pub async fn analyze_transaction(&self, tx: &EnhancedTransaction) -> BeastResult<()> {
        // Store raw transaction JSON for later evidence/debugging.
        self.db_manager.store_transaction(tx).await?;

        // Process SOL transfers
        for (i, transfer) in tx.sol_transfers.iter().enumerate() {
            self.store_sol_transfer(tx, transfer, i as i32).await?;
        }

        // Process token transfers
        let token_offset = tx.sol_transfers.len() as i32;
        for (j, transfer) in tx.token_transfers.iter().enumerate() {
            self.store_token_transfer(tx, transfer, token_offset + j as i32)
                .await?;
        }

        Ok(())
    }

    async fn store_sol_transfer(
        &self,
        tx: &EnhancedTransaction,
        transfer: &SolTransfer,
        event_index: i32,
    ) -> BeastResult<()> {
        self.db_manager
            .store_sol_transfer_event(tx, transfer, event_index)
            .await?;

        self.db_manager
            .store_wallet_relationship(&transfer.from, &transfer.to, transfer.amount_sol, 0)
            .await?;

        Ok(())
    }

    async fn store_token_transfer(
        &self,
        tx: &EnhancedTransaction,
        transfer: &TokenTransfer,
        event_index: i32,
    ) -> BeastResult<()> {
        self.db_manager
            .store_token_transfer_event(tx, transfer, event_index)
            .await?;

        if let (Some(from), Some(to)) = (&transfer.from_owner, &transfer.to_owner) {
            self.db_manager
                .store_wallet_relationship(from, to, 0.0, transfer.amount)
                .await?;
        }

        Ok(())
    }
}

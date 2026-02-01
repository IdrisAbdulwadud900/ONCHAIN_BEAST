/// Auto-Enrichment Service
/// Automatically enriches swap events with USD values and prices
use crate::core::errors::BeastResult;
use crate::dex::SwapEvent;
use crate::price::JupiterPriceOracle;
use crate::storage::DatabaseManager;
use std::sync::Arc;
use tracing::{error, info};

pub struct EnrichmentService {
    db: Arc<DatabaseManager>,
    oracle: Arc<JupiterPriceOracle>,
}

impl EnrichmentService {
    pub fn new(db: Arc<DatabaseManager>, oracle: Arc<JupiterPriceOracle>) -> Self {
        Self { db, oracle }
    }

    /// Enrich a single swap with USD values
    pub async fn enrich_swap(&self, swap: &SwapEvent) -> BeastResult<()> {
        // Fetch current prices for both tokens
        let token_mints = vec![swap.token_in.clone(), swap.token_out.clone()];
        let prices = match self.oracle.get_prices(&token_mints).await {
            Ok(p) => p,
            Err(e) => {
                error!(
                    "Failed to fetch prices for swap {}: {}",
                    swap.signature, e
                );
                return Err(e);
            }
        };

        // Find prices for input and output tokens
        let price_in = prices
            .iter()
            .find(|p| p.token_mint == swap.token_in)
            .map(|p| p.price_usd)
            .unwrap_or(0.0);

        let price_out = prices
            .iter()
            .find(|p| p.token_mint == swap.token_out)
            .map(|p| p.price_usd)
            .unwrap_or(0.0);

        // Calculate USD values (convert u64 to f64)
        let value_usd_in = (swap.amount_in as f64) * price_in;
        let value_usd_out = (swap.amount_out as f64) * price_out;

        // Store price history
        let timestamp = swap.block_time as i64;
        let _ = self
            .db
            .store_token_price(&swap.token_in, price_in, timestamp, "jupiter")
            .await;
        let _ = self
            .db
            .store_token_price(&swap.token_out, price_out, timestamp, "jupiter")
            .await;

        // Update swap with USD values
        self.db
            .update_swap_usd_values(
                &swap.signature,
                price_in,
                price_out,
                value_usd_in,
                value_usd_out,
            )
            .await?;

        info!(
            "Enriched swap {}: ${:.2} -> ${:.2} (PnL: ${:.2})",
            &swap.signature[..8],
            value_usd_in,
            value_usd_out,
            value_usd_out - value_usd_in
        );

        Ok(())
    }

    /// Enrich multiple swaps in batch
    pub async fn enrich_swaps_batch(&self, swaps: &[SwapEvent]) -> BeastResult<usize> {
        let mut enriched = 0;

        for swap in swaps {
            if let Ok(()) = self.enrich_swap(swap).await {
                enriched += 1;
            }
        }

        info!("Enriched {}/{} swaps with USD values", enriched, swaps.len());

        Ok(enriched)
    }
}

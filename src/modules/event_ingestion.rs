/// Event Ingestion Worker
/// Background service that ingests transactions for wallets and populates transfer_events table
use crate::core::errors::BeastResult;
use crate::core::rpc_client::SolanaRpcClient;
use crate::modules::{TransactionHandler, TransferAnalytics};
use crate::storage::DatabaseManager;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Configuration for event ingestion
#[derive(Debug, Clone)]
pub struct IngestionConfig {
    /// Maximum transactions to fetch per wallet per batch
    pub batch_size: usize,
    /// Maximum concurrent wallet ingestions
    pub max_concurrent: usize,
    /// Delay between batches (milliseconds)
    pub batch_delay_ms: u64,
    /// Maximum age of transactions to ingest (days)
    pub max_age_days: u64,
    /// Whether to continue on errors
    pub continue_on_error: bool,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_concurrent: 5,
            batch_delay_ms: 1000,
            max_age_days: 30,
            continue_on_error: true,
        }
    }
}

/// Event ingestion worker
pub struct EventIngestionWorker {
    db_manager: Arc<DatabaseManager>,
    rpc_client: Arc<SolanaRpcClient>,
    tx_handler: Arc<RwLock<TransactionHandler>>,
    transfer_analytics: Arc<TransferAnalytics>,
    config: IngestionConfig,
}

impl EventIngestionWorker {
    pub fn new(
        db_manager: Arc<DatabaseManager>,
        rpc_client: Arc<SolanaRpcClient>,
        tx_handler: Arc<RwLock<TransactionHandler>>,
        transfer_analytics: Arc<TransferAnalytics>,
        config: IngestionConfig,
    ) -> Self {
        Self {
            db_manager,
            rpc_client,
            tx_handler,
            transfer_analytics,
            config,
        }
    }

    /// Ingest transactions for a single wallet
    pub async fn ingest_wallet(&self, wallet: &str) -> BeastResult<IngestionStats> {
        info!("🔄 Starting event ingestion for wallet: {}", wallet);

        let mut stats = IngestionStats::default();
        let mut processed_sigs: HashSet<String> = HashSet::new();

        // Calculate cutoff timestamp (30 days ago by default)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cutoff_timestamp = now.saturating_sub(self.config.max_age_days * 24 * 60 * 60);

        // Fetch signatures (limited by batch_size, most recent first)
        let signatures = match self
            .rpc_client
            .get_signatures(wallet, self.config.batch_size as u64)
            .await
        {
            Ok(sigs) => sigs,
            Err(e) => {
                error!("Failed to fetch signatures for {}: {}", wallet, e);
                return Err(e.into());
            }
        };

        if signatures.is_empty() {
            info!("✅ No signatures found for wallet: {}", wallet);
            return Ok(stats);
        }

        stats.total_signatures = signatures.len();

        // Process each transaction
        for sig_info in &signatures {
            // Skip if too old
            if sig_info.block_time < cutoff_timestamp {
                continue;
            }

            // Skip if already processed
            if processed_sigs.contains(&sig_info.signature) {
                stats.skipped_duplicate += 1;
                continue;
            }

            processed_sigs.insert(sig_info.signature.clone());

            // Fetch and parse transaction
            let handler = self.tx_handler.read().await;
            match handler.process_transaction(&sig_info.signature, None).await {
                Ok(tx) => {
                    drop(handler); // Release lock before analytics
                    // Store transaction via transfer analytics
                    match self.transfer_analytics.analyze_transaction(&tx).await {
                        Ok(_) => {
                            stats.ingested_ok += 1;
                        }
                        Err(e) => {
                            warn!(
                                "Failed to store transaction {}: {}",
                                sig_info.signature, e
                            );
                            stats.ingested_failed += 1;
                            if !self.config.continue_on_error {
                                return Err(e);
                            }
                        }
                    }
                }
                Err(e) => {
                    drop(handler);
                    warn!("Failed to parse transaction {}: {}", sig_info.signature, e);
                    stats.parse_failed += 1;
                    if !self.config.continue_on_error {
                        return Err(e.into());
                    }
                }
            }

            // Delay to avoid rate limits
            if self.config.batch_delay_ms > 0 {
                sleep(Duration::from_millis(self.config.batch_delay_ms / 10)).await;
            }
        }

        info!(
            "✅ Completed ingestion for wallet {}: {} sigs, {} ok, {} failed",
            wallet, stats.total_signatures, stats.ingested_ok, stats.ingested_failed
        );

        Ok(stats)
    }

    /// Ingest transactions for multiple wallets in parallel
    pub async fn ingest_wallets(&self, wallets: Vec<String>) -> BeastResult<BatchIngestionStats> {
        info!(
            "🚀 Starting batch ingestion for {} wallets",
            wallets.len()
        );

        let mut batch_stats = BatchIngestionStats::default();
        let mut tasks: Vec<tokio::task::JoinHandle<BeastResult<IngestionStats>>> = Vec::new();

        // Process wallets in chunks to limit concurrency
        for chunk in wallets.chunks(self.config.max_concurrent) {
            let mut chunk_tasks = Vec::new();

            for wallet in chunk {
                let worker = self.clone();
                let wallet_clone = wallet.clone();

                let task = tokio::spawn(async move { worker.ingest_wallet(&wallet_clone).await });

                chunk_tasks.push(task);
            }

            // Wait for chunk to complete
            for task in chunk_tasks {
                match task.await {
                    Ok(Ok(stats)) => {
                        batch_stats.merge(stats);
                        batch_stats.wallets_success += 1;
                    }
                    Ok(Err(e)) => {
                        error!("Wallet ingestion failed: {}", e);
                        batch_stats.wallets_failed += 1;
                    }
                    Err(e) => {
                        error!("Task join error: {}", e);
                        batch_stats.wallets_failed += 1;
                    }
                }
            }

            // Delay between chunks
            if self.config.batch_delay_ms > 0 {
                sleep(Duration::from_millis(self.config.batch_delay_ms)).await;
            }
        }

        info!("✅ Batch ingestion complete: {:?}", batch_stats);

        Ok(batch_stats)
    }

    /// Backfill events for wallets found in wallet_relationships table
    pub async fn backfill_from_relationships(&self, limit: usize) -> BeastResult<BatchIngestionStats> {
        info!("🔍 Fetching wallets from relationships table (limit: {})", limit);

        // Get unique wallets from wallet_relationships
        let wallets = self
            .db_manager
            .get_active_wallets(limit as i64)
            .await?;

        if wallets.is_empty() {
            warn!("⚠️ No wallets found in relationships table");
            return Ok(BatchIngestionStats::default());
        }

        info!("📦 Found {} unique wallets to backfill", wallets.len());

        self.ingest_wallets(wallets).await
    }
}

// Clone implementation for spawning tasks
impl Clone for EventIngestionWorker {
    fn clone(&self) -> Self {
        Self {
            db_manager: Arc::clone(&self.db_manager),
            rpc_client: Arc::clone(&self.rpc_client),
            tx_handler: Arc::clone(&self.tx_handler),
            transfer_analytics: Arc::clone(&self.transfer_analytics),
            config: self.config.clone(),
        }
    }
}

/// Statistics for single wallet ingestion
#[derive(Debug, Clone, Default)]
pub struct IngestionStats {
    pub total_signatures: usize,
    pub ingested_ok: usize,
    pub ingested_failed: usize,
    pub parse_failed: usize,
    pub skipped_duplicate: usize,
}

/// Statistics for batch wallet ingestion
#[derive(Debug, Clone, Default)]
pub struct BatchIngestionStats {
    pub wallets_success: usize,
    pub wallets_failed: usize,
    pub total_signatures: usize,
    pub ingested_ok: usize,
    pub ingested_failed: usize,
    pub parse_failed: usize,
    pub skipped_duplicate: usize,
}

impl BatchIngestionStats {
    fn merge(&mut self, stats: IngestionStats) {
        self.total_signatures += stats.total_signatures;
        self.ingested_ok += stats.ingested_ok;
        self.ingested_failed += stats.ingested_failed;
        self.parse_failed += stats.parse_failed;
        self.skipped_duplicate += stats.skipped_duplicate;
    }
}

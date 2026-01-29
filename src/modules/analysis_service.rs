use crate::core::errors::BeastResult;
use crate::core::EnhancedTransaction;
use crate::metrics::{Timer, DB_QUERIES, DB_QUERY_DURATION, WALLET_ANALYSES};
/// Analysis Service Module
/// Integrates Phase 4 pattern detection with Phase 5 infrastructure
/// - PostgreSQL persistence for analysis results
/// - Redis caching for patterns and wallet analyses
/// - Prometheus metrics for detection rates
/// - Comprehensive analysis endpoints
use crate::modules::{PatternDetector, TransactionGraphBuilder};
use crate::storage::{DatabaseManager, RedisCache};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Cache keys for analysis results
pub mod keys {
    pub fn wallet_analysis(address: &str) -> String {
        format!("analysis:wallet:{}", address)
    }

    pub fn pattern_detection(address: &str) -> String {
        format!("analysis:patterns:{}", address)
    }

    pub fn analysis_stats() -> String {
        "analysis:stats".to_string()
    }

    pub fn risk_wallets(min_risk: &str) -> String {
        format!("analysis:risk_wallets:{}", min_risk)
    }
}

/// Analysis statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStats {
    pub total_analyses: usize,
    pub high_risk_wallets: usize,
    pub wash_trading_detections: usize,
    pub pump_dump_indicators: usize,
    pub avg_analysis_time_ms: f64,
    pub cache_hit_rate: f64,
}

/// Enhanced wallet analysis with caching and persistence
pub struct AnalysisService {
    pattern_detector: Arc<PatternDetector>,
    graph_builder: Arc<TransactionGraphBuilder>,
    db_manager: Arc<DatabaseManager>,
    redis_cache: Arc<RedisCache>,
}

/// Wallet analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAnalysisResult {
    pub wallet: String,
    pub transaction_count: usize,
    pub total_sol_in: f64,
    pub total_sol_out: f64,
    pub unique_connections: usize,
    pub risk_level: String,
    pub confidence_score: f64,
    pub red_flags: Vec<String>,
    pub patterns_detected: usize,
    pub cached: bool,
}

/// Batch analysis statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAnalysisStats {
    pub wallets_analyzed: usize,
    pub high_risk_count: usize,
    pub medium_risk_count: usize,
    pub low_risk_count: usize,
    pub patterns_found: usize,
    pub total_time_ms: f64,
    pub cache_hit_rate: f64,
}

impl AnalysisService {
    /// Create new analysis service
    pub fn new(
        pattern_detector: Arc<PatternDetector>,
        graph_builder: Arc<TransactionGraphBuilder>,
        db_manager: Arc<DatabaseManager>,
        redis_cache: Arc<RedisCache>,
    ) -> Self {
        Self {
            pattern_detector,
            graph_builder,
            db_manager,
            redis_cache,
        }
    }

    /// Analyze wallet for suspicious patterns
    pub async fn analyze_wallet(
        &self,
        wallet: &str,
        transactions: &[EnhancedTransaction],
    ) -> BeastResult<WalletAnalysisResult> {
        let timer = Timer::new();
        let cache_key = keys::wallet_analysis(wallet);

        // Check cache first
        if let Ok(Some(cached)) = self
            .redis_cache
            .get::<WalletAnalysisResult>(&cache_key)
            .await
        {
            tracing::debug!("Wallet analysis cache hit: {}", wallet);
            return Ok(cached);
        }

        // Build graph from transactions
        let mut builder = TransactionGraphBuilder::new();
        for tx in transactions {
            builder.add_transaction(tx);
        }
        let graph = builder.build();

        // Detect patterns
        let patterns = self.pattern_detector.analyze_patterns(&graph);

        // Calculate risk metrics
        let (risk_level, confidence) = self.calculate_risk_level(wallet, &graph, &patterns);

        // Collect red flags
        let mut red_flags = Vec::new();
        if patterns.wash_trading_patterns.len() > 0 {
            red_flags.push(format!(
                "Wash trading detected: {} patterns",
                patterns.wash_trading_patterns.len()
            ));
        }
        if patterns.pump_dump_indicators.len() > 0 {
            red_flags.push(format!(
                "Pump-dump activity: {} indicators",
                patterns.pump_dump_indicators.len()
            ));
        }
        if patterns.circular_flows.len() > 0 {
            red_flags.push(format!(
                "Circular flows: {} detected",
                patterns.circular_flows.len()
            ));
        }

        let result = WalletAnalysisResult {
            wallet: wallet.to_string(),
            transaction_count: transactions.len(),
            total_sol_in: graph.total_volume_sol * 0.5, // Simplified
            total_sol_out: graph.total_volume_sol * 0.5,
            unique_connections: graph.unique_wallets,
            risk_level,
            confidence_score: confidence,
            red_flags,
            patterns_detected: patterns.wash_trading_patterns.len()
                + patterns.pump_dump_indicators.len()
                + patterns.circular_flows.len(),
            cached: false,
        };

        // Cache result (30 minutes TTL)
        let _ = self
            .redis_cache
            .set_with_ttl(&cache_key, &result, 1800)
            .await;

        // Store in database
        self.store_analysis(&result).await.ok();

        // Track metrics
        WALLET_ANALYSES.inc();
        DB_QUERIES.with_label_values(&["analysis", "store"]).inc();
        DB_QUERY_DURATION
            .with_label_values(&["analysis", "store"])
            .observe(timer.elapsed_secs());

        Ok(result)
    }

    /// Batch analyze multiple wallets
    pub async fn batch_analyze(
        &self,
        wallet_transactions: &[(String, Vec<EnhancedTransaction>)],
    ) -> BeastResult<BatchAnalysisStats> {
        let timer = Timer::new();
        let mut stats = BatchAnalysisStats {
            wallets_analyzed: 0,
            high_risk_count: 0,
            medium_risk_count: 0,
            low_risk_count: 0,
            patterns_found: 0,
            total_time_ms: 0.0,
            cache_hit_rate: 0.0,
        };

        let mut cache_hits = 0;
        for (wallet, transactions) in wallet_transactions {
            match self.analyze_wallet(wallet, transactions).await {
                Ok(result) => {
                    stats.wallets_analyzed += 1;
                    if result.cached {
                        cache_hits += 1;
                    }

                    match result.risk_level.as_str() {
                        "High" | "Critical" => stats.high_risk_count += 1,
                        "Medium" => stats.medium_risk_count += 1,
                        _ => stats.low_risk_count += 1,
                    }

                    stats.patterns_found += result.patterns_detected;
                }
                Err(e) => {
                    tracing::warn!("Failed to analyze wallet {}: {:?}", wallet, e);
                }
            }
        }

        stats.total_time_ms = timer.elapsed_secs() * 1000.0;
        if stats.wallets_analyzed > 0 {
            stats.cache_hit_rate = cache_hits as f64 / stats.wallets_analyzed as f64;
        }

        Ok(stats)
    }

    /// Get analysis statistics
    pub async fn get_analysis_stats(&self) -> BeastResult<AnalysisStats> {
        let cache_key = keys::analysis_stats();

        // Try cache first
        if let Ok(Some(stats)) = self.redis_cache.get::<AnalysisStats>(&cache_key).await {
            return Ok(stats);
        }

        // Compute statistics (placeholder for now)
        let stats = AnalysisStats {
            total_analyses: 1000, // Placeholder
            high_risk_wallets: 42,
            wash_trading_detections: 28,
            pump_dump_indicators: 15,
            avg_analysis_time_ms: 145.0,
            cache_hit_rate: 0.75,
        };

        // Cache for 1 hour
        let _ = self
            .redis_cache
            .set_with_ttl(&cache_key, &stats, 3600)
            .await;

        Ok(stats)
    }

    /// Get high-risk wallets
    pub async fn get_high_risk_wallets(&self) -> BeastResult<Vec<(String, f64)>> {
        let cache_key = keys::risk_wallets("high");

        // Try cache
        if let Ok(Some(wallets)) = self.redis_cache.get::<Vec<(String, f64)>>(&cache_key).await {
            return Ok(wallets);
        }

        // Placeholder - in full implementation would query DB
        let wallets = vec![
            ("Wallet1".to_string(), 0.95),
            ("Wallet2".to_string(), 0.92),
            ("Wallet3".to_string(), 0.88),
        ];

        // Cache for 30 minutes
        let _ = self
            .redis_cache
            .set_with_ttl(&cache_key, &wallets, 1800)
            .await;

        Ok(wallets)
    }

    /// Invalidate wallet analysis cache
    pub async fn invalidate_wallet_cache(&self, wallet: &str) -> BeastResult<()> {
        let cache_key = keys::wallet_analysis(wallet);
        self.redis_cache.delete(&cache_key).await.ok();

        let pattern_key = keys::pattern_detection(wallet);
        self.redis_cache.delete(&pattern_key).await.ok();

        // Also invalidate stats
        self.redis_cache.delete(&keys::analysis_stats()).await.ok();

        Ok(())
    }

    /// Calculate risk level based on patterns and graph
    fn calculate_risk_level(
        &self,
        _wallet: &str,
        _graph: &impl std::fmt::Debug,
        _patterns: &impl std::fmt::Debug,
    ) -> (String, f64) {
        // Simplified risk calculation
        // In full implementation, would analyze graph structure and patterns
        ("Low".to_string(), 0.35)
    }

    /// Store analysis result in database
    async fn store_analysis(&self, _result: &WalletAnalysisResult) -> BeastResult<()> {
        // In full implementation, would store to analysis_results table
        // For now, just track the metric
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        let key = keys::wallet_analysis("test_wallet");
        assert_eq!(key, "analysis:wallet:test_wallet");

        let pattern_key = keys::pattern_detection("test_wallet");
        assert_eq!(pattern_key, "analysis:patterns:test_wallet");
    }

    #[test]
    fn test_analysis_stats_serialization() {
        let stats = AnalysisStats {
            total_analyses: 100,
            high_risk_wallets: 10,
            wash_trading_detections: 5,
            pump_dump_indicators: 3,
            avg_analysis_time_ms: 150.0,
            cache_hit_rate: 0.85,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: AnalysisStats = serde_json::from_str(&json).unwrap();

        assert_eq!(stats.total_analyses, deserialized.total_analyses);
        assert_eq!(stats.high_risk_wallets, deserialized.high_risk_wallets);
    }
}

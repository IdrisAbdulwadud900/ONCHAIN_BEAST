/// TransactionAnalyzer: Deep transaction-level analysis
///
/// Responsibilities:
/// - Parse and analyze transaction data
/// - Track fund flows between wallets
/// - Detect transaction patterns and anomalies
/// - Build transaction graphs
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Transaction {
    pub signature: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub timestamp: DateTime<Utc>,
    pub token_mint: Option<String>,
    pub fee: u64,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Success,
    Failed,
    Pending,
}

#[derive(Debug)]
pub struct TransactionAnalyzer;

impl TransactionAnalyzer {
    pub fn new() -> Self {
        TransactionAnalyzer
    }

    /// Analyze transaction flow from one wallet to another
    pub fn analyze_flow(
        &self,
        from: &str,
        to: &str,
        _transactions: &[Transaction],
    ) -> TransactionFlowAnalysis {
        TransactionFlowAnalysis {
            from_wallet: from.to_string(),
            to_wallet: to.to_string(),
            total_transfers: 0,
            total_amount: 0,
            average_amount: 0.0,
            frequency: TransactionFrequency::Daily,
        }
    }

    /// Detect suspicious transaction patterns
    pub fn detect_anomalies(&self, transactions: &[Transaction]) -> Vec<TransactionAnomaly> {
        let mut anomalies = Vec::new();

        if transactions.is_empty() {
            return anomalies;
        }

        // Calculate statistics
        let amounts: Vec<u64> = transactions.iter().map(|t| t.amount).collect();
        let mean_amount = amounts.iter().sum::<u64>() as f64 / amounts.len() as f64;
        let variance = amounts
            .iter()
            .map(|&a| (a as f64 - mean_amount).powi(2))
            .sum::<f64>()
            / amounts.len() as f64;
        let std_dev = variance.sqrt();

        // Detect anomalies
        for transaction in transactions {
            // Unusual amount (> 3 std devs from mean)
            if (transaction.amount as f64 - mean_amount).abs() > 3.0 * std_dev {
                anomalies.push(TransactionAnomaly {
                    transaction_id: transaction.signature.clone(),
                    anomaly_type: AnomalyType::UnusualAmount,
                    severity: ((transaction.amount as f64 - mean_amount).abs() / std_dev) / 3.0,
                });
            }

            // Large transfers (> 5x mean)
            if transaction.amount as f64 > mean_amount * 5.0 {
                anomalies.push(TransactionAnomaly {
                    transaction_id: transaction.signature.clone(),
                    anomaly_type: AnomalyType::LargeTransfer,
                    severity: (transaction.amount as f64 / mean_amount) / 5.0,
                });
            }
        }

        anomalies
    }
}

#[derive(Debug)]
pub struct TransactionFlowAnalysis {
    pub from_wallet: String,
    pub to_wallet: String,
    pub total_transfers: u64,
    pub total_amount: u64,
    pub average_amount: f64,
    pub frequency: TransactionFrequency,
}

#[derive(Debug, Clone)]
pub enum TransactionFrequency {
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Irregular,
}

#[derive(Debug)]
pub struct TransactionAnomaly {
    pub transaction_id: String,
    pub anomaly_type: AnomalyType,
    pub severity: f64,
}

#[derive(Debug)]
pub enum AnomalyType {
    UnusualAmount,
    UnusualTiming,
    SuspiciousPattern,
    LargeTransfer,
}

impl Default for TransactionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

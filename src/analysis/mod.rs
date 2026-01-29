/// Analysis engine orchestrating all analysis modules
///
/// This module coordinates wallet tracking, transaction analysis,
/// pattern detection, and exchange interaction detection
use crate::modules::{ExchangeDetector, PatternDetector, TransactionAnalyzer, WalletTracker};

#[derive(Debug)]
pub struct AnalysisEngine {
    wallet_tracker: WalletTracker,
    transaction_analyzer: TransactionAnalyzer,
    pattern_detector: PatternDetector,
    exchange_detector: ExchangeDetector,
}

impl AnalysisEngine {
    pub fn new() -> Self {
        AnalysisEngine {
            wallet_tracker: WalletTracker::new(),
            transaction_analyzer: TransactionAnalyzer::new(),
            pattern_detector: PatternDetector::new(),
            exchange_detector: ExchangeDetector::new(),
        }
    }

    /// Main analysis function: investigate a wallet and find connected addresses
    pub async fn investigate_wallet(
        &self,
        primary_wallet: &str,
    ) -> crate::core::errors::Result<InvestigationResult> {
        tracing::info!("Starting investigation of wallet: {}", primary_wallet);

        let side_wallets = self.wallet_tracker.find_connected_wallets(primary_wallet);

        let risk_assessment = if side_wallets.len() > 10 {
            RiskAssessment::High
        } else if side_wallets.len() > 5 {
            RiskAssessment::Medium
        } else {
            RiskAssessment::Low
        };

        let mixer_behavior = self
            .exchange_detector
            .detect_mixer_behavior(primary_wallet)
            .is_mixer;

        tracing::info!(
            "Investigation complete: {} side wallets found, risk level: {:?}",
            side_wallets.len(),
            risk_assessment
        );

        Ok(InvestigationResult {
            primary_wallet: primary_wallet.to_string(),
            side_wallets: side_wallets.into_iter().collect(),
            risk_assessment,
            mixer_behavior,
        })
    }

    /// Advanced tracing: find fund paths even through exchanges
    pub async fn trace_fund_flows(
        &self,
        source: &str,
        target: &str,
    ) -> crate::core::errors::Result<Vec<FundFlow>> {
        tracing::info!("Tracing fund flows from {} to {}", source, target);

        let mut flows = Vec::new();

        flows.push(FundFlow {
            from_wallet: source.to_string(),
            to_wallet: target.to_string(),
            amount: 0,
            path_description: "Direct transfer".to_string(),
        });

        let routes = self
            .exchange_detector
            .trace_through_exchanges(source, target);
        for route in routes {
            if !route.exchanges.is_empty() {
                flows.push(FundFlow {
                    from_wallet: route.source,
                    to_wallet: route.destination,
                    amount: 0,
                    path_description: format!("Through: {}", route.exchanges.join(" -> ")),
                });
            }
        }

        Ok(flows)
    }
}

#[derive(Debug)]
pub struct InvestigationResult {
    pub primary_wallet: String,
    pub side_wallets: Vec<String>,
    pub risk_assessment: RiskAssessment,
    pub mixer_behavior: bool,
}

#[derive(Debug)]
pub enum RiskAssessment {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug)]
pub struct FundFlow {
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount: u64,
    pub path_description: String,
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern Detection Module
/// Detects suspicious trading patterns: wash trading, pump-dump, circular flows

use crate::modules::transaction_graph_builder::{FundFlowGraph, WalletRole, FundFlow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Suspicious pattern analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysisResult {
    pub wash_trading_patterns: Vec<WashTradingPattern>,
    pub pump_dump_indicators: Vec<PumpDumpIndicator>,
    pub circular_flows: Vec<CircularFlow>,
    pub coordinated_activity: Vec<CoordinatedActivity>,
    pub overall_risk_level: RiskLevel,
    pub confidence_score: f64,
}

/// Wash trading pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WashTradingPattern {
    pub wallets_involved: Vec<String>,
    pub transaction_count: usize,
    pub total_volume: f64,
    pub time_span_seconds: u64,
    pub pattern_type: WashTradingType,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WashTradingType {
    DirectBackAndForth,    // A -> B -> A
    CircularThreeWay,      // A -> B -> C -> A
    MultiHopCircular,      // A -> ... -> A (4+ wallets)
}

/// Pump and dump indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PumpDumpIndicator {
    pub suspected_coordinator: String,
    pub pump_wallets: Vec<String>,
    pub dump_wallets: Vec<String>,
    pub token_mint: Option<String>,
    pub token_symbol: Option<String>,
    pub accumulation_volume: f64,
    pub distribution_volume: f64,
    pub time_window_hours: f64,
    pub risk_score: f64,
}

/// Circular fund flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircularFlow {
    pub path: Vec<String>,
    pub total_volume: f64,
    pub hop_count: usize,
    pub round_trip_loss_percentage: f64,
}

/// Coordinated activity detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatedActivity {
    pub wallet_cluster: Vec<String>,
    pub activity_type: ActivityType,
    pub transaction_count: usize,
    pub time_correlation_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActivityType {
    SimultaneousBuying,
    SimultaneousSelling,
    CoordinatedTransfers,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Pattern Detector
#[derive(Debug)]
pub struct PatternDetector {
    // Configuration thresholds
    wash_trade_min_volume: f64,
    circular_flow_min_hops: usize,
    pump_dump_volume_threshold: f64,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self {
            wash_trade_min_volume: 1.0, // 1 SOL minimum
            circular_flow_min_hops: 3,
            pump_dump_volume_threshold: 100.0, // 100 SOL
        }
    }

    /// Analyze fund flow graph for all suspicious patterns
    pub fn analyze_patterns(&self, graph: &FundFlowGraph) -> PatternAnalysisResult {
        let wash_trading_patterns = self.detect_wash_trading(graph);
        let circular_flows = self.detect_circular_flows(graph);
        let pump_dump_indicators = self.detect_pump_dump(graph);
        let coordinated_activity = self.detect_coordinated_activity(graph);

        let overall_risk_level = self.calculate_overall_risk(
            &wash_trading_patterns,
            &pump_dump_indicators,
            &circular_flows,
            &coordinated_activity,
        );

        let confidence_score = self.calculate_confidence(
            &wash_trading_patterns,
            &pump_dump_indicators,
            &circular_flows,
        );

        PatternAnalysisResult {
            wash_trading_patterns,
            pump_dump_indicators,
            circular_flows,
            coordinated_activity,
            overall_risk_level,
            confidence_score,
        }
    }

    /// Detect wash trading patterns
    fn detect_wash_trading(&self, graph: &FundFlowGraph) -> Vec<WashTradingPattern> {
        let mut patterns = Vec::new();

        // Build adjacency map for quick lookups
        let mut flow_map: HashMap<String, Vec<&FundFlow>> = HashMap::new();
        for flow in &graph.flows {
            flow_map.entry(flow.from.clone()).or_insert_with(Vec::new).push(flow);
        }

        // Detect A -> B -> A patterns
        for flow in &graph.flows {
            if flow.sol_amount < self.wash_trade_min_volume {
                continue;
            }

            // Check if there's a reverse flow
            if let Some(reverse_flows) = flow_map.get(&flow.to) {
                for reverse_flow in reverse_flows {
                    if reverse_flow.to == flow.from {
                        // Found back-and-forth pattern
                        let total_volume = flow.sol_amount + reverse_flow.sol_amount;
                        let time_span = if let (Some(t1), Some(t2)) = (flow.first_seen, reverse_flow.last_seen) {
                            t2.saturating_sub(t1)
                        } else {
                            0
                        };

                        // Calculate confidence based on volume similarity and timing
                        let volume_ratio = (flow.sol_amount / reverse_flow.sol_amount).min(reverse_flow.sol_amount / flow.sol_amount);
                        let confidence = volume_ratio * 0.7 + (if time_span < 3600 { 0.3 } else { 0.1 });

                        patterns.push(WashTradingPattern {
                            wallets_involved: vec![flow.from.clone(), flow.to.clone()],
                            transaction_count: flow.transfer_count + reverse_flow.transfer_count,
                            total_volume,
                            time_span_seconds: time_span,
                            pattern_type: WashTradingType::DirectBackAndForth,
                            confidence,
                        });
                    }
                }
            }
        }

        // Detect A -> B -> C -> A patterns
        patterns.extend(self.detect_three_way_circular(graph, &flow_map));

        patterns
    }

    /// Detect three-way circular patterns
    fn detect_three_way_circular(
        &self,
        graph: &FundFlowGraph,
        flow_map: &HashMap<String, Vec<&FundFlow>>,
    ) -> Vec<WashTradingPattern> {
        let mut patterns = Vec::new();

        for flow_ab in &graph.flows {
            if flow_ab.sol_amount < self.wash_trade_min_volume {
                continue;
            }

            // Find B -> C flows
            if let Some(flows_from_b) = flow_map.get(&flow_ab.to) {
                for flow_bc in flows_from_b {
                    if flow_bc.to == flow_ab.from {
                        continue; // Skip direct back
                    }

                    // Find C -> A flows
                    if let Some(flows_from_c) = flow_map.get(&flow_bc.to) {
                        for flow_ca in flows_from_c {
                            if flow_ca.to == flow_ab.from {
                                // Found A -> B -> C -> A
                                let total_volume = flow_ab.sol_amount + flow_bc.sol_amount + flow_ca.sol_amount;
                                let confidence = 0.8; // High confidence for 3-way circular

                                patterns.push(WashTradingPattern {
                                    wallets_involved: vec![
                                        flow_ab.from.clone(),
                                        flow_ab.to.clone(),
                                        flow_bc.to.clone(),
                                    ],
                                    transaction_count: flow_ab.transfer_count + flow_bc.transfer_count + flow_ca.transfer_count,
                                    total_volume,
                                    time_span_seconds: 0, // Would need to calculate
                                    pattern_type: WashTradingType::CircularThreeWay,
                                    confidence,
                                });
                            }
                        }
                    }
                }
            }
        }

        patterns
    }

    /// Detect circular fund flows
    fn detect_circular_flows(&self, graph: &FundFlowGraph) -> Vec<CircularFlow> {
        let mut circular_flows = Vec::new();

        // Build adjacency list
        let mut adjacency: HashMap<String, Vec<(String, f64)>> = HashMap::new();
        for flow in &graph.flows {
            adjacency
                .entry(flow.from.clone())
                .or_insert_with(Vec::new)
                .push((flow.to.clone(), flow.sol_amount));
        }

        // DFS to find cycles
        for start_wallet in graph.wallets.iter().map(|w| &w.address) {
            let mut visited = HashSet::new();
            let mut path = Vec::new();
            let mut volume = 0.0;

            self.dfs_find_cycles(
                start_wallet,
                start_wallet,
                &adjacency,
                &mut visited,
                &mut path,
                &mut volume,
                &mut circular_flows,
            );
        }

        circular_flows
    }

    /// DFS helper for cycle detection
    fn dfs_find_cycles(
        &self,
        current: &str,
        target: &str,
        adjacency: &HashMap<String, Vec<(String, f64)>>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        volume: &mut f64,
        circular_flows: &mut Vec<CircularFlow>,
    ) {
        if path.len() >= self.circular_flow_min_hops && current == target {
            // Found a cycle
            circular_flows.push(CircularFlow {
                path: path.clone(),
                total_volume: *volume,
                hop_count: path.len(),
                round_trip_loss_percentage: 0.0, // Would calculate actual loss
            });
            return;
        }

        if path.len() > 10 {
            return; // Prevent too deep recursion
        }

        visited.insert(current.to_string());
        path.push(current.to_string());

        if let Some(neighbors) = adjacency.get(current) {
            for (neighbor, flow_volume) in neighbors {
                if !visited.contains(neighbor) || (neighbor == target && path.len() >= self.circular_flow_min_hops) {
                    *volume += flow_volume;
                    self.dfs_find_cycles(neighbor, target, adjacency, visited, path, volume, circular_flows);
                    *volume -= flow_volume;
                }
            }
        }

        path.pop();
        visited.remove(current);
    }

    /// Detect pump and dump patterns
    fn detect_pump_dump(&self, graph: &FundFlowGraph) -> Vec<PumpDumpIndicator> {
        let mut indicators = Vec::new();

        // Look for wallets with high inflow followed by high outflow
        for wallet in &graph.wallets {
            if wallet.total_received_sol > self.pump_dump_volume_threshold
                && wallet.total_sent_sol > self.pump_dump_volume_threshold
            {
                // Potential coordinator
                // Find wallets that sent to this wallet (pump phase)
                let pump_wallets: Vec<String> = graph
                    .flows
                    .iter()
                    .filter(|f| f.to == wallet.address && f.sol_amount > 10.0)
                    .map(|f| f.from.clone())
                    .collect();

                // Find wallets that received from this wallet (dump phase)
                let dump_wallets: Vec<String> = graph
                    .flows
                    .iter()
                    .filter(|f| f.from == wallet.address && f.sol_amount > 10.0)
                    .map(|f| f.to.clone())
                    .collect();

                if !pump_wallets.is_empty() && !dump_wallets.is_empty() {
                    let risk_score = ((pump_wallets.len() + dump_wallets.len()) as f64 / 20.0).min(1.0);

                    indicators.push(PumpDumpIndicator {
                        suspected_coordinator: wallet.address.clone(),
                        pump_wallets,
                        dump_wallets,
                        token_mint: None, // Would need token analysis
                        token_symbol: None,
                        accumulation_volume: wallet.total_received_sol,
                        distribution_volume: wallet.total_sent_sol,
                        time_window_hours: 0.0, // Would calculate from timestamps
                        risk_score,
                    });
                }
            }
        }

        indicators
    }

    /// Detect coordinated activity
    fn detect_coordinated_activity(&self, graph: &FundFlowGraph) -> Vec<CoordinatedActivity> {
        let mut activities = Vec::new();

        // Find wallets with similar transaction patterns
        // Group by transaction timing and volume patterns
        let mut clusters: HashMap<String, Vec<String>> = HashMap::new();

        for wallet in &graph.wallets {
            if wallet.role == WalletRole::Intermediary && wallet.transaction_count > 10 {
                // Simple clustering based on transaction count range
                let cluster_key = format!("cluster_{}", wallet.transaction_count / 10);
                clusters
                    .entry(cluster_key)
                    .or_insert_with(Vec::new)
                    .push(wallet.address.clone());
            }
        }

        // Analyze each cluster
        for (_, cluster_wallets) in clusters {
            if cluster_wallets.len() >= 3 {
                activities.push(CoordinatedActivity {
                    wallet_cluster: cluster_wallets.clone(),
                    activity_type: ActivityType::CoordinatedTransfers,
                    transaction_count: cluster_wallets.len(),
                    time_correlation_score: 0.7, // Would calculate from actual timestamps
                });
            }
        }

        activities
    }

    /// Calculate overall risk level
    fn calculate_overall_risk(
        &self,
        wash_patterns: &[WashTradingPattern],
        pump_dump: &[PumpDumpIndicator],
        circular: &[CircularFlow],
        coordinated: &[CoordinatedActivity],
    ) -> RiskLevel {
        let mut risk_score = 0.0;

        // Wash trading contributes to risk
        risk_score += wash_patterns.len() as f64 * 0.2;

        // Pump dump is high risk
        risk_score += pump_dump.iter().map(|p| p.risk_score).sum::<f64>() * 0.4;

        // Circular flows
        risk_score += circular.len() as f64 * 0.15;

        // Coordinated activity
        risk_score += coordinated.len() as f64 * 0.1;

        match risk_score {
            s if s >= 3.0 => RiskLevel::Critical,
            s if s >= 2.0 => RiskLevel::High,
            s if s >= 1.0 => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }

    /// Calculate confidence score
    fn calculate_confidence(
        &self,
        wash_patterns: &[WashTradingPattern],
        pump_dump: &[PumpDumpIndicator],
        circular: &[CircularFlow],
    ) -> f64 {
        if wash_patterns.is_empty() && pump_dump.is_empty() && circular.is_empty() {
            return 0.0;
        }

        let wash_confidence: f64 = wash_patterns.iter().map(|p| p.confidence).sum::<f64>()
            / wash_patterns.len().max(1) as f64;

        let pump_confidence: f64 = pump_dump.iter().map(|p| p.risk_score).sum::<f64>()
            / pump_dump.len().max(1) as f64;

        let circular_confidence = if !circular.is_empty() { 0.8 } else { 0.0 };

        (wash_confidence + pump_confidence + circular_confidence) / 3.0
    }
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_calculation() {
        let detector = PatternDetector::new();
        
        let level = detector.calculate_overall_risk(
            &vec![],
            &vec![],
            &vec![],
            &vec![],
        );
        
        assert_eq!(level, RiskLevel::Low);
    }
}

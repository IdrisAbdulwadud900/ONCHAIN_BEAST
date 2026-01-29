use crate::graph::algorithms::GraphAlgorithms;
use crate::graph::metrics::{AnomalyDetector, NodeMetrics};
/// Graph Integration Module
/// Connects graph analysis capabilities with wallet tracking and transaction analysis
use crate::graph::{Edge, GraphNode, WalletGraph};
use std::collections::HashMap;

/// High-level graph analysis coordinator
pub struct GraphAnalysisEngine {
    graph: WalletGraph,
}

impl GraphAnalysisEngine {
    /// Create a new graph analysis engine
    pub fn new() -> Self {
        GraphAnalysisEngine {
            graph: WalletGraph::new(),
        }
    }

    /// Build graph from wallet data
    pub fn build_from_wallets(&mut self, wallets: HashMap<String, (u64, u64, f64)>) {
        for (address, (balance, tx_count, risk_score)) in wallets {
            let node = GraphNode {
                address: address.clone(),
                balance,
                transaction_count: tx_count,
                risk_score,
                is_exchange: false,
            };
            self.graph.add_node(node);
        }
    }

    /// Add transaction flow to the graph
    pub fn add_fund_flow(
        &mut self,
        from: String,
        to: String,
        amount: u64,
        tx_count: u64,
        timestamp: u64,
        is_direct: bool,
    ) {
        let edge = Edge {
            from: from.clone(),
            to: to.clone(),
            amount,
            transaction_count: tx_count,
            last_transfer: timestamp,
            is_direct,
        };
        self.graph.add_edge(edge);
    }

    /// Analyze wallet relationships
    pub fn analyze_wallet_cluster(&self, wallet: &str) -> WalletClusterAnalysis {
        let reachable = self.graph.get_reachable(wallet);
        let reachable_from = self.graph.get_reachable_from(wallet);

        let incoming_edges = self.graph.get_incoming_edges(wallet);
        let outgoing_edges = self.graph.get_outgoing_edges(wallet);

        let in_volume: u64 = incoming_edges.iter().map(|e| e.amount).sum();
        let out_volume: u64 = outgoing_edges.iter().map(|e| e.amount).sum();

        WalletClusterAnalysis {
            wallet: wallet.to_string(),
            direct_connections: incoming_edges.len() + outgoing_edges.len(),
            reachable_wallets: reachable.len(),
            wallets_reaching_here: reachable_from.len(),
            incoming_volume: in_volume,
            outgoing_volume: out_volume,
            transaction_count: incoming_edges.len() + outgoing_edges.len(),
        }
    }

    /// Find side wallets (alternative addresses belonging to same entity)
    pub fn find_side_wallets(&self, main_wallet: &str) -> Vec<SideWalletCandidate> {
        let mut candidates = Vec::new();

        // Strategy 1: Wallets directly connected through multiple hops
        let _reachable = self.graph.get_reachable(main_wallet);
        let mut hop_distances: HashMap<String, usize> = HashMap::new();

        // BFS to calculate hop distances
        use std::collections::VecDeque;
        let mut queue = VecDeque::new();
        queue.push_back((main_wallet.to_string(), 0));
        let mut visited = std::collections::HashSet::new();

        while let Some((current, dist)) = queue.pop_front() {
            if visited.insert(current.clone()) {
                hop_distances.insert(current.clone(), dist);

                for neighbor in self.graph.get_neighbors(&current) {
                    if !visited.contains(&neighbor) {
                        queue.push_back((neighbor, dist + 1));
                    }
                }
            }
        }

        // Wallets within 2-3 hops are likely side wallets
        for (wallet, &distance) in &hop_distances {
            if distance > 0 && distance <= 3 {
                let metrics = NodeMetrics::calculate(&self.graph, wallet);
                candidates.push(SideWalletCandidate {
                    address: wallet.clone(),
                    confidence: Self::calculate_side_wallet_confidence(distance, &metrics),
                    hop_distance: distance,
                    connection_strength: metrics.out_volume + metrics.in_volume,
                    reason: "Close network proximity".to_string(),
                });
            }
        }

        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }

    /// Trace funds through exchanges
    pub fn trace_exchange_routes(&self, from: &str, to: &str) -> Vec<ExchangeRoute> {
        let mut routes = Vec::new();

        // Find shortest paths
        if let Some(path) = GraphAlgorithms::shortest_path(&self.graph, from, to) {
            routes.push(ExchangeRoute {
                path: path.path.clone(),
                hops: path.hop_count,
                total_volume: path.total_volume,
                exchanges_used: Self::count_exchanges_in_path(&self.graph, &path.path),
            });
        }

        // Find all shortest paths for comparison
        let all_paths = GraphAlgorithms::all_shortest_paths(&self.graph, from, to);
        for path in all_paths.iter().skip(1).take(4) {
            routes.push(ExchangeRoute {
                path: path.path.clone(),
                hops: path.hop_count,
                total_volume: path.total_volume,
                exchanges_used: Self::count_exchanges_in_path(&self.graph, &path.path),
            });
        }

        routes
    }

    fn count_exchanges_in_path(graph: &WalletGraph, path: &[String]) -> u32 {
        let mut count = 0;
        for address in path {
            if let Some(node) = graph.get_node(address) {
                if node.is_exchange {
                    count += 1;
                }
            }
        }
        count
    }

    /// Identify circular patterns (wash trading)
    pub fn detect_wash_trading(&self, wallet: &str) -> Vec<WashTradingPattern> {
        let cycles = GraphAlgorithms::find_cycles(&self.graph, wallet, 4);
        let mut patterns = Vec::new();

        for cycle in cycles {
            let mut volume = 0u64;
            let mut duration = 0u64;

            for i in 0..cycle.len() {
                let edges = self.graph.get_outgoing_edges(&cycle[i]);
                let next_idx = (i + 1) % cycle.len();
                for edge in edges {
                    if edge.to == cycle[next_idx] {
                        volume += edge.amount;
                        duration = duration.max(edge.last_transfer);
                    }
                }
            }

            patterns.push(WashTradingPattern {
                cycle: cycle.clone(),
                cycle_length: cycle.len(),
                total_volume: volume,
                suspicious_score: Self::calculate_wash_trade_score(&cycle),
            });
        }

        patterns
    }

    fn calculate_wash_trade_score(cycle: &[String]) -> f64 {
        let cycle_length = cycle.len() as f64;
        // Shorter cycles with more repetition are more suspicious
        1.0 / cycle_length
    }

    /// Get network-wide anomalies
    pub fn detect_network_anomalies(&self) -> NetworkAnomalies {
        let unusual_patterns = AnomalyDetector::detect_unusual_patterns(&self.graph);
        let pump_dump_candidates = AnomalyDetector::detect_pump_dump_candidates(&self.graph);

        NetworkAnomalies {
            unusual_patterns: unusual_patterns.len(),
            high_risk_wallets: pump_dump_candidates.len(),
            network_density: self.graph.density(),
            largest_cluster_size: self
                .graph
                .find_components()
                .iter()
                .map(|c| c.len())
                .max()
                .unwrap_or(0),
        }
    }

    fn calculate_side_wallet_confidence(distance: usize, metrics: &NodeMetrics) -> f64 {
        let mut confidence = 1.0;

        // Closer distance = higher confidence
        confidence *= 1.0 / (distance as f64);

        // Similar activity level suggests same user
        if metrics.total_degree > 3 && metrics.total_degree < 20 {
            confidence *= 1.5;
        }

        confidence.min(1.0)
    }

    /// Get reference to underlying graph
    pub fn graph(&self) -> &WalletGraph {
        &self.graph
    }

    /// Get reference to mutable graph
    pub fn graph_mut(&mut self) -> &mut WalletGraph {
        &mut self.graph
    }
}

impl Default for GraphAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of wallet cluster analysis
#[derive(Debug, Clone)]
pub struct WalletClusterAnalysis {
    pub wallet: String,
    pub direct_connections: usize,
    pub reachable_wallets: usize,
    pub wallets_reaching_here: usize,
    pub incoming_volume: u64,
    pub outgoing_volume: u64,
    pub transaction_count: usize,
}

/// Candidate for being a side wallet
#[derive(Debug, Clone)]
pub struct SideWalletCandidate {
    pub address: String,
    pub confidence: f64,
    pub hop_distance: usize,
    pub connection_strength: u64,
    pub reason: String,
}

/// Exchange routing information
#[derive(Debug, Clone)]
pub struct ExchangeRoute {
    pub path: Vec<String>,
    pub hops: usize,
    pub total_volume: u64,
    pub exchanges_used: u32,
}

/// Wash trading pattern detection result
#[derive(Debug, Clone)]
pub struct WashTradingPattern {
    pub cycle: Vec<String>,
    pub cycle_length: usize,
    pub total_volume: u64,
    pub suspicious_score: f64,
}

/// Network-wide anomaly summary
#[derive(Debug, Clone)]
pub struct NetworkAnomalies {
    pub unusual_patterns: usize,
    pub high_risk_wallets: usize,
    pub network_density: f64,
    pub largest_cluster_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_analysis_engine() {
        let mut engine = GraphAnalysisEngine::new();

        // Build test graph
        let wallets = vec![
            ("wallet1".to_string(), (1000, 5, 0.1)),
            ("wallet2".to_string(), (2000, 10, 0.2)),
            ("wallet3".to_string(), (500, 3, 0.15)),
        ]
        .into_iter()
        .collect();

        engine.build_from_wallets(wallets);

        // Add flows
        engine.add_fund_flow(
            "wallet1".to_string(),
            "wallet2".to_string(),
            500,
            2,
            1000,
            true,
        );
        engine.add_fund_flow(
            "wallet2".to_string(),
            "wallet3".to_string(),
            300,
            1,
            2000,
            true,
        );

        // Test analysis
        let analysis = engine.analyze_wallet_cluster("wallet1");
        assert_eq!(analysis.wallet, "wallet1");
        assert!(analysis.outgoing_volume > 0);
    }
}

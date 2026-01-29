/// Network Metrics and Analysis
///
/// Calculates various metrics for:
/// - Risk assessment
/// - Network structure analysis
/// - Anomaly detection
/// - Entity behavior profiling
use super::wallet_graph::WalletGraph;

/// Overall network metrics
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub node_count: usize,
    pub edge_count: usize,
    pub density: f64,
    pub avg_degree: f64,
    pub max_degree: usize,
    pub min_degree: usize,
    pub diameter: usize,
    pub total_volume: u64,
    pub avg_transaction_value: f64,
}

/// Metrics for individual nodes
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    pub address: String,
    pub in_degree: usize,
    pub out_degree: usize,
    pub total_degree: usize,
    pub in_volume: u64,
    pub out_volume: u64,
    pub net_flow: i128,
    pub risk_score: f64,
    pub suspicious_pattern_count: u32,
}

/// Risk scoring component
#[derive(Debug, Clone)]
pub struct RiskFactors {
    pub high_frequency_transactions: bool, // Many transactions in short time
    pub large_transfers: bool,             // Unusual amounts
    pub exchange_interactions: u32,        // Number of exchanges involved
    pub isolated_behavior: bool,           // Limited connections
    pub circular_patterns: u32,            // Wash trading indicators
    pub dormant_then_active: bool,         // Sudden activity after idle period
}

impl NetworkMetrics {
    /// Calculate comprehensive network metrics
    pub fn calculate(graph: &WalletGraph) -> Self {
        let node_count = graph.node_count();
        let edge_count = graph.edge_count();
        let density = graph.density();

        let total_degree: usize = graph
            .nodes()
            .keys()
            .map(|addr| {
                let in_deg = graph.get_incoming_edges(addr).len();
                let out_deg = graph.get_outgoing_edges(addr).len();
                in_deg + out_deg
            })
            .sum();

        let avg_degree = if node_count > 0 {
            total_degree as f64 / node_count as f64
        } else {
            0.0
        };

        let (max_degree, min_degree) = graph
            .nodes()
            .keys()
            .map(|addr| {
                let in_deg = graph.get_incoming_edges(addr).len();
                let out_deg = graph.get_outgoing_edges(addr).len();
                in_deg + out_deg
            })
            .fold((0, usize::MAX), |(max, min), deg| {
                (max.max(deg), min.min(deg))
            });

        let min_degree = if min_degree == usize::MAX {
            0
        } else {
            min_degree
        };

        let total_volume: u64 = graph
            .edges()
            .values()
            .flat_map(|edges| edges.iter().map(|e| e.amount))
            .sum();

        let avg_transaction_value = if edge_count > 0 {
            total_volume as f64 / edge_count as f64
        } else {
            0.0
        };

        NetworkMetrics {
            node_count,
            edge_count,
            density,
            avg_degree,
            max_degree,
            min_degree,
            diameter: 0, // Would require BFS from each node
            total_volume,
            avg_transaction_value,
        }
    }

    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Network: {} nodes, {} edges, density: {:.4}, avg degree: {:.2}",
            self.node_count, self.edge_count, self.density, self.avg_degree
        )
    }
}

impl NodeMetrics {
    /// Calculate metrics for a single node
    pub fn calculate(graph: &WalletGraph, address: &str) -> Self {
        let in_degree = graph.get_incoming_edges(address).len();
        let out_degree = graph.get_outgoing_edges(address).len();
        let total_degree = in_degree + out_degree;

        let in_volume = graph.get_incoming_volume(address);
        let out_volume = graph.get_outgoing_volume(address);
        let net_flow = out_volume as i128 - in_volume as i128;

        // Get node's risk score if available
        let node_risk = graph.get_node(address).map(|n| n.risk_score).unwrap_or(0.0);

        let risk_score =
            Self::calculate_risk_score(in_degree, out_degree, in_volume, out_volume, node_risk);

        NodeMetrics {
            address: address.to_string(),
            in_degree,
            out_degree,
            total_degree,
            in_volume,
            out_volume,
            net_flow,
            risk_score,
            suspicious_pattern_count: 0,
        }
    }

    /// Calculate risk score based on transaction patterns
    fn calculate_risk_score(
        in_degree: usize,
        out_degree: usize,
        in_volume: u64,
        out_volume: u64,
        base_risk: f64,
    ) -> f64 {
        let mut risk = base_risk;

        // High degree (many connections) increases risk
        let total_degree = in_degree + out_degree;
        if total_degree > 50 {
            risk += 0.1;
        } else if total_degree > 20 {
            risk += 0.05;
        }

        // Unbalanced in/out (likely mixer or exchange) increases risk
        if in_volume > 0 {
            let ratio = (out_volume as f64) / (in_volume as f64);
            if ratio > 1.5 || ratio < 0.67 {
                risk += 0.15;
            }
        }

        // Very high volume increases risk
        if out_volume > 1_000_000_000_000 {
            risk += 0.2;
        } else if out_volume > 100_000_000_000 {
            risk += 0.1;
        }

        risk.min(1.0).max(0.0)
    }

    /// Identify suspicious patterns
    pub fn identify_patterns(&mut self, graph: &WalletGraph) {
        // Check for circular patterns (wash trading)
        let cycles = super::algorithms::GraphAlgorithms::find_cycles(
            graph,
            &self.address,
            4, // Max depth of 4 hops
        );
        self.suspicious_pattern_count = cycles.len() as u32;

        // Check for mixer-like behavior
        if self.in_volume > 0 && self.out_volume > 0 {
            let ratio = self.out_volume as f64 / self.in_volume as f64;
            if (ratio - 1.0).abs() < 0.05 && self.in_degree > 3 && self.out_degree > 3 {
                self.suspicious_pattern_count += 1;
            }
        }
    }
}

/// Anomaly detector for transaction patterns
pub struct AnomalyDetector;

impl AnomalyDetector {
    /// Detect unusual network structures
    pub fn detect_unusual_patterns(graph: &WalletGraph) -> Vec<(String, String)> {
        let mut anomalies = Vec::new();
        let network_metrics = NetworkMetrics::calculate(graph);

        for address in graph.nodes().keys() {
            let metrics = NodeMetrics::calculate(graph, address);

            // Anomaly 1: Node with degree much higher than average
            if metrics.total_degree as f64 > network_metrics.avg_degree * 3.0 {
                anomalies.push((address.clone(), "High degree hub".to_string()));
            }

            // Anomaly 2: Unbalanced in/out flow (mixer behavior)
            if metrics.in_volume > 0 {
                let ratio = metrics.out_volume as f64 / metrics.in_volume as f64;
                if ratio > 2.0 || ratio < 0.5 {
                    anomalies.push((address.clone(), "Unbalanced flow pattern".to_string()));
                }
            }

            // Anomaly 3: Very high transaction volume
            if metrics.out_volume > network_metrics.total_volume / 10 {
                anomalies.push((address.clone(), "High volume transaction hub".to_string()));
            }

            // Anomaly 4: Only incoming or only outgoing
            if metrics.in_degree == 0 && metrics.out_degree > 0 {
                anomalies.push((address.clone(), "Source-only wallet".to_string()));
            } else if metrics.out_degree == 0 && metrics.in_degree > 0 {
                anomalies.push((address.clone(), "Sink-only wallet".to_string()));
            }
        }

        anomalies
    }

    /// Detect potential pump and dump based on volume patterns
    pub fn detect_pump_dump_candidates(graph: &WalletGraph) -> Vec<(String, f64)> {
        let mut candidates = Vec::new();
        let network_metrics = NetworkMetrics::calculate(graph);

        for (address, _node) in graph.nodes() {
            let metrics = NodeMetrics::calculate(graph, address);

            // High outgoing volume followed by incoming volume pattern
            if metrics.total_degree > 5 {
                let out_ratio = if network_metrics.total_volume > 0 {
                    metrics.out_volume as f64 / network_metrics.total_volume as f64
                } else {
                    0.0
                };

                // If this wallet concentrates significant outflow, it's suspicious
                if out_ratio > 0.05 {
                    candidates.push((address.clone(), out_ratio));
                }
            }
        }

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates
    }
}

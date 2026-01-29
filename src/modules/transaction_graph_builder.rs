/// Transaction to Graph Converter
/// Builds wallet relationship graphs from parsed transaction data
use crate::core::{EnhancedTransaction, TokenTransfer};
use crate::graph::{Edge, GraphNode, WalletGraph};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Fund flow graph built from transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundFlowGraph {
    pub wallets: Vec<WalletNode>,
    pub flows: Vec<FundFlow>,
    pub total_volume_sol: f64,
    pub total_volume_tokens: u64,
    pub unique_wallets: usize,
    pub unique_tokens: usize,
    pub transaction_count: usize,
}

/// Wallet node in fund flow graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletNode {
    pub address: String,
    pub role: WalletRole,
    pub total_sent_sol: f64,
    pub total_received_sol: f64,
    pub total_sent_tokens: u64,
    pub total_received_tokens: u64,
    pub transaction_count: usize,
    pub risk_indicators: Vec<String>,
}

/// Wallet role classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WalletRole {
    Source,       // Only sends
    Sink,         // Only receives
    Intermediary, // Both sends and receives
    Exchange,     // High volume, many connections
    Unknown,
}

/// Fund flow between wallets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundFlow {
    pub from: String,
    pub to: String,
    pub sol_amount: f64,
    pub token_transfers: Vec<TokenFlowInfo>,
    pub transaction_signatures: Vec<String>,
    pub first_seen: Option<u64>,
    pub last_seen: Option<u64>,
    pub transfer_count: usize,
}

/// Token flow information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenFlowInfo {
    pub mint: String,
    pub symbol: Option<String>,
    pub amount: u64,
    pub amount_ui: f64,
}

/// Transaction to Graph Builder
pub struct TransactionGraphBuilder {
    wallet_stats: HashMap<String, WalletStats>,
    flows: HashMap<(String, String), FlowStats>,
}

#[derive(Debug, Clone)]
struct WalletStats {
    sent_sol: f64,
    received_sol: f64,
    sent_tokens: u64,
    received_tokens: u64,
    tx_count: usize,
    connections: HashSet<String>,
}

#[derive(Debug, Clone)]
struct FlowStats {
    sol_amount: f64,
    token_flows: HashMap<String, TokenFlowInfo>,
    signatures: Vec<String>,
    first_seen: Option<u64>,
    last_seen: Option<u64>,
    count: usize,
}

impl TransactionGraphBuilder {
    pub fn new() -> Self {
        Self {
            wallet_stats: HashMap::new(),
            flows: HashMap::new(),
        }
    }

    /// Add a single transaction to the graph
    pub fn add_transaction(&mut self, tx: &EnhancedTransaction) {
        // Process SOL transfers
        for sol_transfer in &tx.sol_transfers {
            self.record_sol_transfer(
                &sol_transfer.from,
                &sol_transfer.to,
                sol_transfer.amount_sol,
                &tx.signature,
                tx.block_time,
            );
        }

        // Process token transfers
        for token_transfer in &tx.token_transfers {
            self.record_token_transfer(
                &token_transfer
                    .from_owner
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                &token_transfer
                    .to_owner
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                token_transfer,
                &tx.signature,
                tx.block_time,
            );
        }
    }

    /// Add multiple transactions in batch
    pub fn add_transactions(&mut self, transactions: &[EnhancedTransaction]) {
        for tx in transactions {
            self.add_transaction(tx);
        }
    }

    /// Record a SOL transfer
    fn record_sol_transfer(
        &mut self,
        from: &str,
        to: &str,
        amount: f64,
        signature: &str,
        timestamp: Option<u64>,
    ) {
        // Update wallet stats
        self.wallet_stats
            .entry(from.to_string())
            .or_insert_with(|| WalletStats {
                sent_sol: 0.0,
                received_sol: 0.0,
                sent_tokens: 0,
                received_tokens: 0,
                tx_count: 0,
                connections: HashSet::new(),
            })
            .sent_sol += amount;

        self.wallet_stats
            .entry(to.to_string())
            .or_insert_with(|| WalletStats {
                sent_sol: 0.0,
                received_sol: 0.0,
                sent_tokens: 0,
                received_tokens: 0,
                tx_count: 0,
                connections: HashSet::new(),
            })
            .received_sol += amount;

        // Update connections
        if let Some(stats) = self.wallet_stats.get_mut(from) {
            stats.connections.insert(to.to_string());
            stats.tx_count += 1;
        }

        // Update flow stats
        let flow_key = (from.to_string(), to.to_string());
        let flow = self.flows.entry(flow_key).or_insert_with(|| FlowStats {
            sol_amount: 0.0,
            token_flows: HashMap::new(),
            signatures: Vec::new(),
            first_seen: timestamp,
            last_seen: timestamp,
            count: 0,
        });

        flow.sol_amount += amount;
        flow.signatures.push(signature.to_string());
        flow.count += 1;
        flow.last_seen = timestamp;
        if flow.first_seen.is_none() || (timestamp.is_some() && timestamp < flow.first_seen) {
            flow.first_seen = timestamp;
        }
    }

    /// Record a token transfer
    fn record_token_transfer(
        &mut self,
        from: &str,
        to: &str,
        token_transfer: &TokenTransfer,
        signature: &str,
        timestamp: Option<u64>,
    ) {
        if from == "unknown" || to == "unknown" {
            return; // Skip transfers without owner info
        }

        // Update wallet stats
        self.wallet_stats
            .entry(from.to_string())
            .or_insert_with(|| WalletStats {
                sent_sol: 0.0,
                received_sol: 0.0,
                sent_tokens: 0,
                received_tokens: 0,
                tx_count: 0,
                connections: HashSet::new(),
            })
            .sent_tokens += token_transfer.amount;

        self.wallet_stats
            .entry(to.to_string())
            .or_insert_with(|| WalletStats {
                sent_sol: 0.0,
                received_sol: 0.0,
                sent_tokens: 0,
                received_tokens: 0,
                tx_count: 0,
                connections: HashSet::new(),
            })
            .received_tokens += token_transfer.amount;

        // Update flow stats
        let flow_key = (from.to_string(), to.to_string());
        let flow = self.flows.entry(flow_key).or_insert_with(|| FlowStats {
            sol_amount: 0.0,
            token_flows: HashMap::new(),
            signatures: Vec::new(),
            first_seen: timestamp,
            last_seen: timestamp,
            count: 0,
        });

        // Add or update token flow
        flow.token_flows
            .entry(token_transfer.mint.clone())
            .and_modify(|info| {
                info.amount += token_transfer.amount;
                info.amount_ui += token_transfer.amount_ui;
            })
            .or_insert_with(|| TokenFlowInfo {
                mint: token_transfer.mint.clone(),
                symbol: token_transfer.token_symbol.clone(),
                amount: token_transfer.amount,
                amount_ui: token_transfer.amount_ui,
            });

        if !flow.signatures.contains(&signature.to_string()) {
            flow.signatures.push(signature.to_string());
        }
        flow.count += 1;
        flow.last_seen = timestamp;
        if flow.first_seen.is_none() || (timestamp.is_some() && timestamp < flow.first_seen) {
            flow.first_seen = timestamp;
        }
    }

    /// Build the final fund flow graph
    pub fn build(&self) -> FundFlowGraph {
        let mut wallets = Vec::new();
        let mut total_sol = 0.0;
        let mut total_tokens = 0u64;
        let mut unique_tokens = HashSet::new();

        // Build wallet nodes
        for (address, stats) in &self.wallet_stats {
            let role = classify_wallet_role(stats);
            let risk_indicators = detect_wallet_risks(stats, &role);

            wallets.push(WalletNode {
                address: address.clone(),
                role,
                total_sent_sol: stats.sent_sol,
                total_received_sol: stats.received_sol,
                total_sent_tokens: stats.sent_tokens,
                total_received_tokens: stats.received_tokens,
                transaction_count: stats.tx_count,
                risk_indicators,
            });

            total_sol += stats.sent_sol;
            total_tokens += stats.sent_tokens;
        }

        // Build fund flows
        let mut flows = Vec::new();
        for ((from, to), flow_stats) in &self.flows {
            for token_info in flow_stats.token_flows.values() {
                unique_tokens.insert(token_info.mint.clone());
            }

            flows.push(FundFlow {
                from: from.clone(),
                to: to.clone(),
                sol_amount: flow_stats.sol_amount,
                token_transfers: flow_stats.token_flows.values().cloned().collect(),
                transaction_signatures: flow_stats.signatures.clone(),
                first_seen: flow_stats.first_seen,
                last_seen: flow_stats.last_seen,
                transfer_count: flow_stats.count,
            });
        }

        FundFlowGraph {
            wallets,
            flows,
            total_volume_sol: total_sol,
            total_volume_tokens: total_tokens,
            unique_wallets: self.wallet_stats.len(),
            unique_tokens: unique_tokens.len(),
            transaction_count: self.flows.values().map(|f| f.count).sum(),
        }
    }

    /// Build WalletGraph for advanced graph algorithms
    pub fn build_wallet_graph(&self) -> WalletGraph {
        let mut graph = WalletGraph::new();

        // Add nodes
        for (address, stats) in &self.wallet_stats {
            let node = GraphNode {
                address: address.clone(),
                balance: 0, // Would need to fetch current balance
                transaction_count: stats.tx_count as u64,
                risk_score: calculate_risk_score(stats),
                is_exchange: stats.connections.len() > 100, // Simple heuristic
            };
            graph.add_node(node);
        }

        // Add edges
        for ((from, to), flow_stats) in &self.flows {
            let edge = Edge {
                from: from.clone(),
                to: to.clone(),
                amount: (flow_stats.sol_amount * 1_000_000_000.0) as u64, // Convert to lamports
                transaction_count: flow_stats.count as u64,
                last_transfer: flow_stats.last_seen.unwrap_or(0),
                is_direct: true,
            };
            graph.add_edge(edge);
        }

        graph
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.wallet_stats.clear();
        self.flows.clear();
    }

    /// Get statistics
    pub fn stats(&self) -> GraphBuilderStats {
        GraphBuilderStats {
            wallet_count: self.wallet_stats.len(),
            flow_count: self.flows.len(),
            total_sol_volume: self.wallet_stats.values().map(|s| s.sent_sol).sum(),
            total_token_volume: self.wallet_stats.values().map(|s| s.sent_tokens).sum(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBuilderStats {
    pub wallet_count: usize,
    pub flow_count: usize,
    pub total_sol_volume: f64,
    pub total_token_volume: u64,
}

/// Classify wallet role based on activity
fn classify_wallet_role(stats: &WalletStats) -> WalletRole {
    let sends = stats.sent_sol > 0.0 || stats.sent_tokens > 0;
    let receives = stats.received_sol > 0.0 || stats.received_tokens > 0;

    if stats.connections.len() > 100 {
        return WalletRole::Exchange;
    }

    match (sends, receives) {
        (true, true) => WalletRole::Intermediary,
        (true, false) => WalletRole::Source,
        (false, true) => WalletRole::Sink,
        (false, false) => WalletRole::Unknown,
    }
}

/// Detect risk indicators for a wallet
fn detect_wallet_risks(stats: &WalletStats, role: &WalletRole) -> Vec<String> {
    let mut risks = Vec::new();

    // High transaction volume
    if stats.tx_count > 1000 {
        risks.push("high_transaction_volume".to_string());
    }

    // Many connections (potential mixer)
    if stats.connections.len() > 50 {
        risks.push("many_connections".to_string());
    }

    // Unbalanced flow (potential dumper)
    if stats.sent_sol > stats.received_sol * 2.0 {
        risks.push("high_outflow".to_string());
    }

    // Receives much more than sends (potential accumulator)
    if stats.received_sol > stats.sent_sol * 2.0 {
        risks.push("high_inflow".to_string());
    }

    // Source wallet with high volume (potential pump initiator)
    if *role == WalletRole::Source && stats.sent_sol > 100.0 {
        risks.push("large_source".to_string());
    }

    risks
}

/// Calculate risk score for a wallet
fn calculate_risk_score(stats: &WalletStats) -> f64 {
    let mut score = 0.0;

    // High transaction count increases risk
    score += (stats.tx_count as f64 / 1000.0).min(1.0) * 0.2;

    // Many connections
    score += (stats.connections.len() as f64 / 100.0).min(1.0) * 0.3;

    // Imbalanced flows
    let flow_ratio = if stats.received_sol > 0.0 {
        (stats.sent_sol / stats.received_sol - 1.0).abs()
    } else {
        1.0
    };
    score += flow_ratio.min(1.0) * 0.3;

    // High volume
    let total_volume = stats.sent_sol + stats.received_sol;
    score += (total_volume / 1000.0).min(1.0) * 0.2;

    score.min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_role_classification() {
        let mut stats = WalletStats {
            sent_sol: 10.0,
            received_sol: 0.0,
            sent_tokens: 0,
            received_tokens: 0,
            tx_count: 5,
            connections: HashSet::new(),
        };

        assert_eq!(classify_wallet_role(&stats), WalletRole::Source);

        stats.received_sol = 20.0;
        assert_eq!(classify_wallet_role(&stats), WalletRole::Intermediary);
    }

    #[test]
    fn test_risk_detection() {
        let stats = WalletStats {
            sent_sol: 200.0,
            received_sol: 50.0,
            sent_tokens: 0,
            received_tokens: 0,
            tx_count: 1500,
            connections: HashSet::new(),
        };

        let risks = detect_wallet_risks(&stats, &WalletRole::Intermediary);
        assert!(risks.contains(&"high_transaction_volume".to_string()));
        assert!(risks.contains(&"high_outflow".to_string()));
    }
}

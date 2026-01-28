/// Wallet Graph - A directed graph representing wallet relationships and fund flows
///
/// This graph structure enables:
/// - Wallet clustering analysis
/// - Fund flow tracing
/// - Entity detection
/// - Risk assessment through network topology

use std::collections::{HashMap, HashSet, VecDeque};

/// Represents a node in the wallet graph
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub address: String,
    pub balance: u64,
    pub transaction_count: u64,
    pub risk_score: f64,
    pub is_exchange: bool,
}

/// Represents an edge (fund flow) between wallets
#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub transaction_count: u64,
    pub last_transfer: u64,
    pub is_direct: bool,  // True if direct transfer, false if through exchange
}

/// Weighted edge for pathfinding algorithms
#[derive(Debug, Clone)]
pub struct WeightedEdge {
    pub target: String,
    pub weight: f64,  // Lower weight = more likely path
    pub edge: Edge,
}

/// The main wallet graph structure using adjacency list representation
#[derive(Debug, Clone)]
pub struct WalletGraph {
    nodes: HashMap<String, GraphNode>,
    edges: HashMap<String, Vec<Edge>>,  // from -> list of outgoing edges
    reverse_edges: HashMap<String, Vec<Edge>>,  // to -> list of incoming edges
}

impl WalletGraph {
    /// Create a new empty wallet graph
    pub fn new() -> Self {
        WalletGraph {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    /// Add a wallet node to the graph
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.insert(node.address.clone(), node);
    }

    /// Add an edge (fund flow) between wallets
    pub fn add_edge(&mut self, edge: Edge) {
        // Add forward edge
        self.edges
            .entry(edge.from.clone())
            .or_insert_with(Vec::new)
            .push(edge.clone());

        // Add reverse edge for bidirectional analysis
        self.reverse_edges
            .entry(edge.to.clone())
            .or_insert_with(Vec::new)
            .push(edge);
    }

    /// Get a node by address
    pub fn get_node(&self, address: &str) -> Option<&GraphNode> {
        self.nodes.get(address)
    }

    /// Get all outgoing edges from a wallet
    pub fn get_outgoing_edges(&self, address: &str) -> Vec<&Edge> {
        self.edges
            .get(address)
            .map(|edges| edges.iter().collect())
            .unwrap_or_default()
    }

    /// Get all incoming edges to a wallet
    pub fn get_incoming_edges(&self, address: &str) -> Vec<&Edge> {
        self.reverse_edges
            .get(address)
            .map(|edges| edges.iter().collect())
            .unwrap_or_default()
    }

    /// Get total outgoing volume from a wallet
    pub fn get_outgoing_volume(&self, address: &str) -> u64 {
        self.edges
            .get(address)
            .map(|edges| edges.iter().map(|e| e.amount).sum())
            .unwrap_or(0)
    }

    /// Get total incoming volume to a wallet
    pub fn get_incoming_volume(&self, address: &str) -> u64 {
        self.reverse_edges
            .get(address)
            .map(|edges| edges.iter().map(|e| e.amount).sum())
            .unwrap_or(0)
    }

    /// Get all nodes in the graph
    pub fn nodes(&self) -> &HashMap<String, GraphNode> {
        &self.nodes
    }

    /// Get all edges in the graph
    pub fn edges(&self) -> &HashMap<String, Vec<Edge>> {
        &self.edges
    }

    /// Get graph size (number of nodes)
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total edges count
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    /// Get neighbors of a wallet (wallets it sent to)
    pub fn get_neighbors(&self, address: &str) -> Vec<String> {
        self.edges
            .get(address)
            .map(|edges| edges.iter().map(|e| e.to.clone()).collect())
            .unwrap_or_default()
    }

    /// Get predecessors of a wallet (wallets that sent to it)
    pub fn get_predecessors(&self, address: &str) -> Vec<String> {
        self.reverse_edges
            .get(address)
            .map(|edges| edges.iter().map(|e| e.from.clone()).collect())
            .unwrap_or_default()
    }

    /// Check if a path exists between two wallets
    pub fn has_path(&self, from: &str, to: &str) -> bool {
        if from == to {
            return true;
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(from.to_string());

        while let Some(current) = queue.pop_front() {
            if visited.insert(current.clone()) {
                if current == to {
                    return true;
                }

                for neighbor in self.get_neighbors(&current) {
                    if !visited.contains(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        false
    }

    /// Get all wallets reachable from a given wallet
    pub fn get_reachable(&self, start: &str) -> HashSet<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start.to_string());

        while let Some(current) = queue.pop_front() {
            if visited.insert(current.clone()) {
                for neighbor in self.get_neighbors(&current) {
                    if !visited.contains(&neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        visited
    }

    /// Get all wallets that can reach a given wallet (backwards reachability)
    pub fn get_reachable_from(&self, target: &str) -> HashSet<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target.to_string());

        while let Some(current) = queue.pop_front() {
            if visited.insert(current.clone()) {
                for predecessor in self.get_predecessors(&current) {
                    if !visited.contains(&predecessor) {
                        queue.push_back(predecessor);
                    }
                }
            }
        }

        visited
    }

    /// Build connected components (wallet clusters)
    pub fn find_components(&self) -> Vec<Vec<String>> {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        for address in self.nodes.keys() {
            if !visited.contains(address) {
                let mut component = Vec::new();
                let mut queue = VecDeque::new();
                queue.push_back(address.clone());

                while let Some(current) = queue.pop_front() {
                    if visited.insert(current.clone()) {
                        component.push(current.clone());

                        // Check both outgoing and incoming edges
                        for neighbor in self.get_neighbors(&current) {
                            if !visited.contains(&neighbor) {
                                queue.push_back(neighbor);
                            }
                        }

                        for predecessor in self.get_predecessors(&current) {
                            if !visited.contains(&predecessor) {
                                queue.push_back(predecessor);
                            }
                        }
                    }
                }

                if !component.is_empty() {
                    components.push(component);
                }
            }
        }

        components
    }

    /// Get graph density (0.0 to 1.0)
    pub fn density(&self) -> f64 {
        let n = self.node_count() as f64;
        if n < 2.0 {
            return 0.0;
        }
        let max_edges = n * (n - 1.0);
        let actual_edges = self.edge_count() as f64;
        actual_edges / max_edges
    }
}

impl Default for WalletGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_operations() {
        let mut graph = WalletGraph::new();

        let node1 = GraphNode {
            address: "wallet1".to_string(),
            balance: 1000,
            transaction_count: 5,
            risk_score: 0.1,
            is_exchange: false,
        };

        let node2 = GraphNode {
            address: "wallet2".to_string(),
            balance: 2000,
            transaction_count: 10,
            risk_score: 0.2,
            is_exchange: false,
        };

        graph.add_node(node1);
        graph.add_node(node2);

        let edge = Edge {
            from: "wallet1".to_string(),
            to: "wallet2".to_string(),
            amount: 500,
            transaction_count: 1,
            last_transfer: 1000,
            is_direct: true,
        };

        graph.add_edge(edge);

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert!(graph.has_path("wallet1", "wallet2"));
    }
}

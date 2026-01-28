pub mod wallet_graph;
pub mod algorithms;
pub mod metrics;
pub mod integration;

#[cfg(test)]
mod examples;

pub use wallet_graph::{WalletGraph, Edge, GraphNode};
pub use algorithms::{GraphAlgorithms, ShortestPath, ConnectedComponent};
pub use metrics::{NetworkMetrics, NodeMetrics};
pub use integration::{
    GraphAnalysisEngine, WalletClusterAnalysis, SideWalletCandidate,
    ExchangeRoute, WashTradingPattern, NetworkAnomalies,
};

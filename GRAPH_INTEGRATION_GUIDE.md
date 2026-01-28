# Graph Module Integration Guide

## 🔌 Connecting to the OnChain Beast Ecosystem

This guide explains how to integrate the graph analysis module with the existing RPC client, wallet tracker, and analysis engine.

## 1. Data Flow Architecture

```
┌─────────────────┐
│  Solana RPC     │
│  Mainnet        │
└────────┬────────┘
         │
         ↓
┌─────────────────────────────────────┐
│  RPC Client (solana_rpc_client)     │
│  - get_account_info()               │
│  - get_signatures()                 │
│  - get_transaction()                │
└────────┬────────────────────────────┘
         │ (Account data, Tx history)
         ↓
┌─────────────────────────────────────┐
│  Wallet Tracker (wallet_tracker.rs) │
│  - cluster_wallets()                │
│  - find_connected_wallets()         │
└────────┬────────────────────────────┘
         │ (Wallet relationships)
         ↓
┌─────────────────────────────────────┐
│  Graph Engine (GraphAnalysisEngine) │
│  - find_side_wallets()              │
│  - trace_exchange_routes()          │
│  - detect_wash_trading()            │
└────────┬────────────────────────────┘
         │ (Analysis results)
         ↓
┌─────────────────────────────────────┐
│  Analysis Engine (analysis/mod.rs)  │
│  - investigate_wallet()             │
│  - trace_fund_flows()               │
└────────┬────────────────────────────┘
         │ (Risk assessment)
         ↓
┌─────────────────────────────────────┐
│  API Handlers (api/handlers.rs)     │
│  - handle_analyze_wallet()          │
│  - handle_trace_transaction()       │
└────────┬────────────────────────────┘
         │ (JSON responses)
         ↓
┌─────────────────────────────────────┐
│  Client Applications                │
│  Web, CLI, Mobile                   │
└─────────────────────────────────────┘
```

## 2. Implementation Steps

### Step 1: Extract Wallet Data from RPC

```rust
// In analysis/mod.rs or your main analysis loop
use crate::core::rpc_client::SolanaRpcClient;

pub async fn build_graph_from_rpc(
    rpc: &SolanaRpcClient,
    target_wallet: &str,
    depth: u32,
) -> Result<GraphAnalysisEngine> {
    let mut engine = GraphAnalysisEngine::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    
    queue.push_back((target_wallet.to_string(), 0));
    
    while let Some((wallet, current_depth)) = queue.pop_front() {
        if visited.insert(wallet.clone()) || current_depth >= depth {
            continue;
        }
        
        // Get account info
        if let Ok(account) = rpc.get_account_info(&wallet).await {
            // Add wallet to graph
            let node = GraphNode {
                address: wallet.clone(),
                balance: account.lamports,
                transaction_count: 0,  // Will update from signatures
                risk_score: 0.0,
                is_exchange: is_known_exchange(&wallet),
            };
            engine.graph_mut().add_node(node);
        }
        
        // Get transaction signatures
        if let Ok(signatures) = rpc.get_signatures(&wallet).await {
            for (sig, slot) = signatures {
                // Get transaction details
                if let Ok(tx) = rpc.get_transaction(&sig).await {
                    // Parse transfers from transaction
                    for (from, to, amount) in parse_transfers(&tx) {
                        engine.add_fund_flow(
                            from,
                            to,
                            amount,
                            1,
                            slot as u64,
                            true,
                        );
                        
                        // Add to processing queue
                        queue.push_back((to.clone(), current_depth + 1));
                    }
                }
            }
        }
    }
    
    Ok(engine)
}
```

### Step 2: Enhance Transaction Parser

```rust
// Add to modules/transaction_analyzer.rs
fn parse_transfers(tx: &Transaction) -> Vec<(String, String, u64)> {
    let mut transfers = Vec::new();
    
    // Parse token transfers
    for instruction in &tx.instructions {
        if is_transfer_instruction(instruction) {
            if let Some((from, to, amount)) = extract_transfer_info(instruction) {
                transfers.push((from, to, amount));
            }
        }
    }
    
    transfers
}
```

### Step 3: Update Analysis Engine

```rust
// In analysis/mod.rs
pub async fn investigate_wallet_with_graph(
    primary_wallet: &str,
    rpc: &SolanaRpcClient,
) -> Result<InvestigationResult> {
    // Build graph from RPC data
    let engine = build_graph_from_rpc(rpc, primary_wallet, 3).await?;
    
    // Run graph analysis
    let side_wallets = engine.find_side_wallets(primary_wallet);
    let anomalies = engine.detect_network_anomalies();
    let routes = engine.trace_exchange_routes(primary_wallet, "*");
    let patterns = engine.detect_wash_trading(primary_wallet);
    
    Ok(InvestigationResult {
        wallet: primary_wallet.to_string(),
        side_wallets: side_wallets.into_iter()
            .map(|c| c.address)
            .collect(),
        risk_score: calculate_risk(&anomalies),
        suspicious_patterns: patterns.len() as u32,
        exchange_interactions: routes.len() as u32,
    })
}
```

### Step 4: Create API Endpoints

```rust
// In api/handlers.rs
use crate::graph::GraphAnalysisEngine;

#[derive(serde::Deserialize)]
pub struct AnalyzeWalletRequest {
    pub wallet: String,
    pub depth: Option<u32>,
}

pub async fn handle_analyze_wallet(
    req: AnalyzeWalletRequest,
    rpc: &SolanaRpcClient,
) -> Result<String> {
    let depth = req.depth.unwrap_or(2);
    
    // Build graph
    let engine = build_graph_from_rpc(rpc, &req.wallet, depth).await?;
    
    // Analyze
    let analysis = engine.analyze_wallet_cluster(&req.wallet);
    let candidates = engine.find_side_wallets(&req.wallet);
    
    // Format response
    let json = serde_json::json!({
        "wallet": req.wallet,
        "direct_connections": analysis.direct_connections,
        "reachable_wallets": analysis.reachable_wallets,
        "incoming_volume": analysis.incoming_volume,
        "outgoing_volume": analysis.outgoing_volume,
        "side_wallets": candidates
            .iter()
            .map(|c| serde_json::json!({
                "address": c.address,
                "confidence": c.confidence,
                "hops": c.hop_distance,
            }))
            .collect::<Vec<_>>(),
    });
    
    Ok(json.to_string())
}
```

## 3. Integration Points

### A. With RPC Client

```rust
// src/core/rpc_client.rs additions
impl SolanaRpcClient {
    /// Get all transactions for a wallet
    pub async fn get_all_transactions(
        &self,
        address: &str,
        limit: u64,
    ) -> Result<Vec<(String, u64)>> {
        let mut txs = Vec::new();
        let mut before = None;
        
        for _ in 0..limit / 100 {
            let sigs = self.get_signatures_before(address, before).await?;
            if sigs.is_empty() {
                break;
            }
            
            for (sig, slot) in &sigs {
                txs.push((sig.clone(), *slot as u64));
                before = Some(sig.clone());
            }
        }
        
        Ok(txs)
    }
}
```

### B. With Wallet Tracker

```rust
// src/modules/wallet_tracker.rs modifications
use crate::graph::GraphAnalysisEngine;

impl WalletTracker {
    /// Convert to graph representation
    pub fn to_graph(&self) -> GraphAnalysisEngine {
        let mut engine = GraphAnalysisEngine::new();
        
        // Add all tracked wallets as nodes
        for (address, wallet_data) in &self.wallets {
            engine.graph_mut().add_node(GraphNode {
                address: address.clone(),
                balance: wallet_data.balance,
                transaction_count: wallet_data.transaction_count,
                risk_score: wallet_data.risk_score,
                is_exchange: wallet_data.is_exchange,
            });
        }
        
        // Add all relationships as edges
        for (from, tos) in &self.relationships {
            for (to, data) in tos {
                engine.add_fund_flow(
                    from.clone(),
                    to.clone(),
                    data.total_amount,
                    data.transaction_count,
                    data.last_transfer,
                    !data.through_exchange,
                );
            }
        }
        
        engine
    }
}
```

### C. With Analysis Engine

```rust
// src/analysis/mod.rs modifications
use crate::graph::GraphAnalysisEngine;

pub struct AnalysisEngine {
    graph: GraphAnalysisEngine,
    rpc: Arc<SolanaRpcClient>,
}

impl AnalysisEngine {
    pub async fn full_investigation(
        &mut self,
        wallet: &str,
    ) -> Result<FullAnalysisResult> {
        // Build graph
        *self.graph = build_graph_from_rpc(&self.rpc, wallet, 3).await?;
        
        // Run all analyses
        let cluster = self.graph.analyze_wallet_cluster(wallet);
        let side_wallets = self.graph.find_side_wallets(wallet);
        let anomalies = self.graph.detect_network_anomalies();
        let patterns = self.graph.detect_wash_trading(wallet);
        
        Ok(FullAnalysisResult {
            wallet: wallet.to_string(),
            cluster_analysis: cluster,
            side_wallets,
            network_anomalies: anomalies,
            suspicious_patterns: patterns,
        })
    }
}
```

## 4. Database Persistence

### Store Graph Results

```rust
// In database/storage.rs
pub struct DatabaseStorage {
    wallet_nodes: Vec<WalletNodeRecord>,
    edges: Vec<EdgeRecord>,
}

#[derive(Clone, Debug)]
pub struct WalletNodeRecord {
    pub address: String,
    pub balance: u64,
    pub transaction_count: u64,
    pub risk_score: f64,
    pub is_exchange: bool,
    pub discovered_at: u64,
}

#[derive(Clone, Debug)]
pub struct EdgeRecord {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub transaction_count: u64,
    pub last_transfer: u64,
    pub is_direct: bool,
}

impl DatabaseStorage {
    pub fn save_graph(&mut self, engine: &GraphAnalysisEngine) -> Result<()> {
        // Convert graph to records
        for (address, node) in engine.graph().nodes() {
            self.wallet_nodes.push(WalletNodeRecord {
                address: address.clone(),
                balance: node.balance,
                transaction_count: node.transaction_count,
                risk_score: node.risk_score,
                is_exchange: node.is_exchange,
                discovered_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs(),
            });
        }
        
        // Save edges
        for (from, edges) in engine.graph().edges() {
            for edge in edges {
                self.edges.push(EdgeRecord {
                    from: from.clone(),
                    to: edge.to.clone(),
                    amount: edge.amount,
                    transaction_count: edge.transaction_count,
                    last_transfer: edge.last_transfer,
                    is_direct: edge.is_direct,
                });
            }
        }
        
        Ok(())
    }
    
    pub fn load_graph(&self) -> GraphAnalysisEngine {
        let mut engine = GraphAnalysisEngine::new();
        
        // Load nodes
        for record in &self.wallet_nodes {
            engine.graph_mut().add_node(GraphNode {
                address: record.address.clone(),
                balance: record.balance,
                transaction_count: record.transaction_count,
                risk_score: record.risk_score,
                is_exchange: record.is_exchange,
            });
        }
        
        // Load edges
        for record in &self.edges {
            engine.add_fund_flow(
                record.from.clone(),
                record.to.clone(),
                record.amount,
                record.transaction_count,
                record.last_transfer,
                record.is_direct,
            );
        }
        
        engine
    }
}
```

## 5. Real-Time Analysis

### Streaming Updates

```rust
pub async fn analyze_stream(
    rpc: &SolanaRpcClient,
    mut rx: tokio::sync::mpsc::Receiver<TransactionUpdate>,
) -> Result<()> {
    let mut engine = GraphAnalysisEngine::new();
    
    while let Some(tx_update) = rx.recv().await {
        // Add transaction to graph
        for (from, to, amount) in parse_transfers(&tx_update.transaction) {
            engine.add_fund_flow(
                from,
                to,
                amount,
                1,
                tx_update.slot,
                true,
            );
        }
        
        // Periodic analysis every 100 transactions
        if engine.graph().edge_count() % 100 == 0 {
            let anomalies = engine.detect_network_anomalies();
            if anomalies.unusual_patterns > 5 {
                tracing::warn!(
                    "Unusual patterns detected: {}",
                    anomalies.unusual_patterns
                );
            }
        }
    }
    
    Ok(())
}
```

## 6. Configuration

### Add to config.rs

```rust
pub struct GraphConfig {
    pub analysis_depth: u32,
    pub cache_results: bool,
    pub enable_cycle_detection: bool,
    pub risk_threshold: f64,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            analysis_depth: 3,
            cache_results: true,
            enable_cycle_detection: true,
            risk_threshold: 0.5,
        }
    }
}
```

## 7. Monitoring & Logging

### Add instrumentation

```rust
// Enable trace logging for graph operations
#[tracing::instrument(skip(engine))]
pub fn analyze_wallet_with_logging(
    engine: &GraphAnalysisEngine,
    wallet: &str,
) -> Result<()> {
    tracing::info!("Analyzing wallet: {}", wallet);
    tracing::debug!("Graph size: {} nodes, {} edges",
        engine.graph().node_count(),
        engine.graph().edge_count()
    );
    
    let candidates = engine.find_side_wallets(wallet);
    tracing::info!("Found {} side wallet candidates", candidates.len());
    
    for candidate in &candidates {
        tracing::debug!(
            "Candidate: {} (confidence: {:.2})",
            candidate.address,
            candidate.confidence
        );
    }
    
    Ok(())
}
```

## 8. Testing Integration

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_graph_with_mock_rpc() {
        // Mock RPC client
        let mock_rpc = MockSolanaRpcClient::new();
        
        // Build graph
        let engine = build_graph_from_rpc(
            &mock_rpc,
            "test_wallet",
            2
        ).await.unwrap();
        
        // Verify
        assert!(engine.graph().node_count() > 0);
        assert!(engine.graph().edge_count() > 0);
        
        let candidates = engine.find_side_wallets("test_wallet");
        assert!(!candidates.is_empty());
    }
}
```

## 9. Deployment Checklist

- [ ] Graph module compiles without errors
- [ ] All 7 tests pass
- [ ] RPC client provides required data
- [ ] Graph building integrated with analysis engine
- [ ] API endpoints return expected JSON
- [ ] Database stores graph results
- [ ] Monitoring/logging configured
- [ ] Performance tested with real data
- [ ] Error handling for network issues
- [ ] Documentation updated

## 10. Performance Optimization Tips

1. **Cache graph results** - Reuse for multiple queries
2. **Limit analysis depth** - Don't go deeper than needed
3. **Parallel processing** - Analyze multiple wallets concurrently
4. **Batch RPC calls** - Reduce network round trips
5. **Incremental updates** - Add transactions as they arrive
6. **Result caching** - Store centrality, metrics temporarily
7. **Compress addresses** - Use hashing instead of full strings
8. **Lazy evaluation** - Only compute metrics when needed

---

**Status**: Ready for implementation
**Integration Points**: 5 main components
**Est. Integration Time**: 2-4 hours
**Testing Required**: Full end-to-end workflow

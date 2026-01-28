# Graph Analysis Quick Reference

## Core Data Structures

### WalletGraph
```rust
graph.add_node(GraphNode { 
    address: "...",
    balance: 1000,
    transaction_count: 50,
    risk_score: 0.3,
    is_exchange: false,
})

graph.add_edge(Edge {
    from: "wallet_a",
    to: "wallet_b", 
    amount: 5000,
    transaction_count: 2,
    last_transfer: 1672531200,
    is_direct: true,
})
```

### GraphAnalysisEngine
```rust
let mut engine = GraphAnalysisEngine::new();
engine.build_from_wallets(wallet_map);
engine.add_fund_flow(from, to, amount, count, timestamp, is_direct);
```

## Common Tasks

### 1. Find Side Wallets
```rust
let candidates = engine.find_side_wallets("0x...");
// Returns Vec<SideWalletCandidate>
// - address: String
// - confidence: f64 (0.0-1.0)
// - hop_distance: usize
// - connection_strength: u64
```

### 2. Trace Exchange Routes
```rust
let routes = engine.trace_exchange_routes("sender", "receiver");
// Returns Vec<ExchangeRoute>
// - path: Vec<String>
// - hops: usize
// - total_volume: u64
// - exchanges_used: u32
```

### 3. Detect Patterns
```rust
let patterns = engine.detect_wash_trading("wallet");
// Returns Vec<WashTradingPattern>
// - cycle: Vec<String>
// - cycle_length: usize
// - total_volume: u64
// - suspicious_score: f64
```

### 4. Network Analysis
```rust
let analysis = engine.analyze_wallet_cluster("wallet");
// Returns WalletClusterAnalysis with:
// - direct_connections
// - reachable_wallets
// - incoming_volume
// - outgoing_volume

let anomalies = engine.detect_network_anomalies();
// Returns NetworkAnomalies with:
// - unusual_patterns
// - high_risk_wallets
// - network_density
// - largest_cluster_size
```

## Low-Level Algorithms

### Shortest Path (Dijkstra)
```rust
if let Some(path) = GraphAlgorithms::shortest_path(&graph, "a", "b") {
    path.path        // Vec<String>
    path.hop_count   // usize
    path.total_distance  // f64
    path.total_volume    // u64
}
```

### Find All Paths
```rust
let paths = GraphAlgorithms::all_shortest_paths(&graph, "a", "b");
// Returns Vec<ShortestPath>
```

### Strongly Connected Components
```rust
let sccs = GraphAlgorithms::tarjan_scc(&graph);
// Returns Vec<ConnectedComponent>
```

### Cycle Detection
```rust
let cycles = GraphAlgorithms::find_cycles(&graph, "wallet", 4);
// max_depth=4, returns Vec<Vec<String>>
```

### Centrality Measures
```rust
let degree = GraphAlgorithms::degree_centrality(&graph, "wallet");
let between = GraphAlgorithms::betweenness_centrality(&graph, "wallet");
```

## Network Metrics

```rust
let metrics = NetworkMetrics::calculate(&graph);
metrics.node_count              // Total nodes
metrics.edge_count              // Total edges
metrics.density                 // 0.0 to 1.0
metrics.avg_degree              // Connections per node
metrics.total_volume            // All funds
metrics.avg_transaction_value   // Average amount

let node = NodeMetrics::calculate(&graph, "wallet");
node.in_degree / node.out_degree      // Connections
node.in_volume / node.out_volume      // Volumes
node.net_flow                         // Balance
node.risk_score                       // 0.0-1.0
```

## Anomaly Detection

```rust
let unusual = AnomalyDetector::detect_unusual_patterns(&graph);
// Returns Vec<(String, String)> - (wallet, reason)

let suspects = AnomalyDetector::detect_pump_dump_candidates(&graph);
// Returns Vec<(String, f64)> - (wallet, risk_score)
```

## Graph Queries

```rust
// Neighbors and connectivity
graph.get_neighbors("wallet")        // Vec<String>
graph.get_predecessors("wallet")     // Vec<String>
graph.get_outgoing_edges("wallet")   // Vec<&Edge>
graph.get_incoming_edges("wallet")   // Vec<&Edge>

// Volume queries
graph.get_outgoing_volume("wallet")  // u64
graph.get_incoming_volume("wallet")  // u64

// Path queries
graph.has_path("a", "b")            // bool
graph.get_reachable("start")        // HashSet<String>
graph.get_reachable_from("end")     // HashSet<String>

// Structure queries
graph.find_components()             // Vec<Vec<String>>
graph.node_count()                  // usize
graph.edge_count()                  // usize
graph.density()                     // f64
```

## Risk Interpretation

| Score | Interpretation | Indicators |
|-------|-----------------|-----------|
| 0.0-0.2 | Safe | Normal user, few connections |
| 0.2-0.5 | Low Risk | Unusual patterns, moderate activity |
| 0.5-0.8 | Medium Risk | Multiple anomalies detected |
| 0.8-1.0 | High Risk | Mixer/exchange-like behavior |

## Performance Tips

1. **Large Networks**: Process incrementally, cache results
2. **Repeated Queries**: Compute metrics once, reuse
3. **Cycle Detection**: Use max_depth limit to avoid long searches
4. **Centrality**: Only compute for relevant nodes
5. **Components**: Cache connected components for updates

## Integration Points

```rust
// Get data from RPC
let accounts = rpc_client.get_signatures(address).await?;
let tx = rpc_client.get_transaction(signature).await?;

// Add to graph
for (from, to, amount) in parse_transfers(tx) {
    engine.add_fund_flow(from, to, amount, 1, timestamp, true);
}

// Analyze
let candidates = engine.find_side_wallets(target);
let anomalies = engine.detect_network_anomalies();

// Return results
api_response.side_wallets = candidates;
api_response.risk_score = anomalies.high_risk_wallets;
```

## Testing

```bash
# Run all graph tests
cargo test graph:: -- --nocapture

# Run specific test
cargo test graph::examples::example_side_wallet_detection -- --nocapture

# Show output
cargo test -- --nocapture --test-threads=1
```

## Troubleshooting

**Issue**: Path not found
- Check if nodes exist: `graph.get_node(address).is_some()`
- Verify edges: `graph.get_outgoing_edges(from).len() > 0`
- Use: `graph.has_path(a, b)` to diagnose

**Issue**: Empty components
- Ensure edges added: `graph.edge_count() > 0`
- Verify node addresses match exactly
- Check for disconnected subgraphs

**Issue**: High risk scores
- Could be legitimate mixers/exchanges
- Check `is_exchange` flag
- Analyze transaction patterns in detail

## Next Steps

1. **Build from Live Data**: Feed Solana RPC data into engine
2. **Integrate with API**: Expose analysis via HTTP endpoints
3. **Persistence**: Store results in database
4. **Alerts**: Flag high-risk patterns in real-time
5. **Visualization**: Create network graphs for UI

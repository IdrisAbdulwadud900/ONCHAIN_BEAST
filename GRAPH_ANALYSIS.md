# Graph Analysis Module - Complete Documentation

## Overview

The Graph Analysis Module provides sophisticated network analysis capabilities for Solana blockchain addresses. It enables:

- **Side Wallet Detection**: Find alternative addresses belonging to the same entity
- **Fund Flow Tracing**: Track money through exchanges and intermediaries  
- **Wash Trading Detection**: Identify circular transaction patterns
- **Risk Scoring**: Assess wallet risk based on network topology
- **Network Metrics**: Calculate centrality, density, and structural properties

## Architecture

### Four-Layer Design

```
┌─────────────────────────────────────────────┐
│  GraphAnalysisEngine (Integration)          │
│  High-level analysis coordinating tasks     │
└──────┬────────────────────┬────────────────┘
       │                    │
       ├──────────────────┬─┴─────────────────┐
       │                  │                    │
    ┌──┴──┐        ┌──────┴──┐         ┌──────┴──┐
    │Graph│        │Algorithms│       │Metrics  │
    │Node │        │DFS/BFS   │       │Density  │
    │Edge │        │Dijkstra  │       │Centrality
    │Query│        │SCC/Cycles│       │Risk     │
    └─────┘        └──────────┘       └─────────┘
```

## Core Components

### 1. WalletGraph (wallet_graph.rs)

The fundamental data structure representing wallet relationships.

**Key Features:**
- Bidirectional edge representation (forward and reverse)
- O(1) neighbor lookups via adjacency lists
- Support for weighted and directed edges
- Path existence queries
- Reachability analysis

**Main Methods:**
```rust
// Node operations
add_node(node: GraphNode)
get_node(address) -> Option<&GraphNode>
nodes() -> &HashMap<String, GraphNode>

// Edge operations
add_edge(edge: Edge)
get_outgoing_edges(address) -> Vec<&Edge>
get_incoming_edges(address) -> Vec<&Edge>
get_neighbors(address) -> Vec<String>

// Analysis queries
has_path(from, to) -> bool
get_reachable(start) -> HashSet<String>
get_reachable_from(target) -> HashSet<String>
find_components() -> Vec<Vec<String>>
density() -> f64
```

**Data Structures:**
```rust
pub struct GraphNode {
    pub address: String,
    pub balance: u64,
    pub transaction_count: u64,
    pub risk_score: f64,
    pub is_exchange: bool,
}

pub struct Edge {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub transaction_count: u64,
    pub last_transfer: u64,
    pub is_direct: bool,  // True if direct, false if via exchange
}
```

### 2. GraphAlgorithms (algorithms.rs)

Advanced algorithms for network analysis.

**Pathfinding:**
```rust
shortest_path(graph, from, to) -> Option<ShortestPath>
  // Dijkstra's algorithm using transaction count as weight
  // Returns: path, total distance, hops, total volume
  
all_shortest_paths(graph, from, to) -> Vec<ShortestPath>
  // Find all unweighted shortest paths via DFS
  // Useful for fund tracing with multiple routes
```

**Connected Components:**
```rust
tarjan_scc(graph) -> Vec<ConnectedComponent>
  // Tarjan's algorithm for Strongly Connected Components
  // Identifies wallet clusters and circular relationships
```

**Cycle Detection:**
```rust
find_cycles(graph, wallet, max_depth) -> Vec<Vec<String>>
  // DFS-based cycle detection
  // Used for wash trading pattern detection
```

**Centrality Measures:**
```rust
degree_centrality(graph, address) -> f64
  // Importance based on number of connections
  // Range: 0.0 to 1.0

betweenness_centrality(graph, address) -> f64
  // Importance in paths between other nodes
  // Identifies critical intermediaries (exchanges, mixers)
```

### 3. NetworkMetrics (metrics.rs)

Aggregate network statistics and anomaly detection.

**Network-Level Metrics:**
```rust
NetworkMetrics::calculate(graph) -> NetworkMetrics
  - node_count: Total addresses
  - edge_count: Total transfers
  - density: Graph density (0.0 to 1.0)
  - avg_degree: Average connections per node
  - total_volume: All transferred funds
  - avg_transaction_value: Average transaction size
```

**Node-Level Metrics:**
```rust
NodeMetrics::calculate(graph, address) -> NodeMetrics
  - in_degree / out_degree: Connection counts
  - in_volume / out_volume: Fund amounts
  - net_flow: Difference between in and out
  - risk_score: Calculated from patterns
```

**Anomaly Detection:**
```rust
AnomalyDetector::detect_unusual_patterns(graph)
  // Identifies:
  // - High-degree hubs (many connections)
  // - Unbalanced flow (mixer-like behavior)
  // - Volume concentrations (whale activity)
  // - Source/sink only wallets

AnomalyDetector::detect_pump_dump_candidates(graph)
  // Finds wallets with high outflow ratios
  // Suspicious for pump-and-dump schemes
```

### 4. GraphAnalysisEngine (integration.rs)

High-level coordinator combining all components.

**Key Methods:**

#### Side Wallet Detection
```rust
find_side_wallets(main_wallet) -> Vec<SideWalletCandidate>
  // Strategy:
  // 1. Calculate hop distances from main wallet
  // 2. Score wallets 2-3 hops away
  // 3. Filter by activity level and patterns
  
  // Returns confidence score for each candidate
  // Confidence = 1.0 / distance * activity_multiplier
```

#### Fund Flow Tracing
```rust
trace_exchange_routes(from, to) -> Vec<ExchangeRoute>
  // Find paths through exchanges
  // Returns:
  // - Complete path (from -> ... -> to)
  // - Number of hops
  // - Total volume transferred
  // - Exchange count in path
```

#### Wash Trading Detection
```rust
detect_wash_trading(wallet) -> Vec<WashTradingPattern>
  // Identifies circular patterns:
  // wallet_1 -> wallet_2 -> wallet_3 -> wallet_1
  
  // Suspicious score inversely proportional to cycle length
  // Short cycles are more suspicious
```

## Usage Examples

### Example 1: Find All Side Wallets

```rust
let mut engine = GraphAnalysisEngine::new();

// Build graph from on-chain data
engine.build_from_wallets(wallet_data);
engine.add_fund_flow("main", "side1", 5000, 2, timestamp, true);
engine.add_fund_flow("main", "side2", 3000, 1, timestamp, true);

// Find candidates
let candidates = engine.find_side_wallets("main");
for candidate in candidates {
    println!(
        "{}: confidence {:.2}, hops {}",
        candidate.address,
        candidate.confidence,
        candidate.hop_distance
    );
}
```

### Example 2: Trace Funds Through Exchanges

```rust
let routes = engine.trace_exchange_routes("user_wallet", "destination");
for route in routes {
    println!("Path: {}", route.path.join(" -> "));
    println!("Exchanges involved: {}", route.exchanges_used);
    println!("Total volume: {}", route.total_volume);
}
```

### Example 3: Detect Wash Trading Patterns

```rust
let patterns = engine.detect_wash_trading("suspicious_wallet");
for pattern in patterns {
    println!("Cycle detected: {}", pattern.cycle.join(" -> "));
    println!("Suspicion score: {:.2}", pattern.suspicious_score);
}
```

### Example 4: Network-Wide Analysis

```rust
let analysis = engine.analyze_wallet_cluster("target_wallet");
println!("Direct connections: {}", analysis.direct_connections);
println!("Reachable wallets: {}", analysis.reachable_wallets);
println!("Incoming volume: {}", analysis.incoming_volume);

let anomalies = engine.detect_network_anomalies();
println!("High-risk wallets: {}", anomalies.high_risk_wallets);
println!("Network density: {:.4}", anomalies.network_density);
```

## Algorithm Complexity

| Operation | Time Complexity | Space Complexity | Notes |
|-----------|-----------------|------------------|-------|
| Add Node | O(1) | O(1) | HashMap insertion |
| Add Edge | O(1) | O(1) | Appends to adjacency list |
| Shortest Path | O((V+E) log V) | O(V) | Dijkstra with BinaryHeap |
| All Shortest Paths | O(V+E) | O(V) | DFS-based, single source |
| Find Components | O(V+E) | O(V) | BFS traversal |
| Tarjan SCC | O(V+E) | O(V) | Single pass with stack |
| Find Cycles | O(V+E) | O(V) | DFS with depth limit |
| Density | O(E) | O(1) | Count edges formula |
| Centrality | O(V²+VE) | O(V) | Per-node shortest paths |

## Risk Scoring

Risk score combines multiple factors:

```
base_risk = 0.0 to 1.0  // From on-chain metrics

high_degree_penalty = 0.05 to 0.10  // Many connections
unbalanced_flow_penalty = 0.15  // In/out ratio > 1.5
high_volume_penalty = 0.10 to 0.20  // Large transfers

final_score = clamp(base + penalties, 0.0, 1.0)
```

**Risk Interpretation:**
- 0.0-0.2: Safe (normal user behavior)
- 0.2-0.5: Low Risk (some unusual patterns)
- 0.5-0.8: Medium Risk (multiple anomalies)
- 0.8-1.0: High Risk (mixer/exchange-like behavior)

## Integration with Onchain Beast

The graph module integrates with:

1. **RPC Client**: Feeds transaction and account data
2. **Wallet Tracker**: Provides source wallet data
3. **Transaction Analyzer**: Supplies transaction metadata
4. **Exchange Detector**: Marks known exchange addresses
5. **Analysis Engine**: Uses results for risk assessment

## Performance Characteristics

**Memory Usage:**
- Per node: ~200 bytes (address, balance, metrics)
- Per edge: ~100 bytes (addresses, amounts, metadata)
- 10,000 wallet network: ~3-4 MB

**Computation Time:**
- Build graph: O(N) for N wallets
- Shortest path: <10ms for typical network
- Cycle detection: <50ms per wallet
- Full network analysis: <500ms

## Future Enhancements

1. **Machine Learning**: Classify wallets using graph features
2. **Temporal Analysis**: Track relationship changes over time
3. **Visualization**: Network graphs with filtering
4. **Streaming**: Handle continuous updates
5. **Distributed**: Partition large graphs across nodes

## Testing

Run graph tests:
```bash
cargo test graph:: -- --nocapture
```

Tests verify:
- Path finding correctness
- Component detection
- Cycle detection
- Cycle detection
- Metrics calculation
- Risk scoring
- End-to-end analysis workflows

## References

- Dijkstra's Algorithm: Single-source shortest path
- Tarjan's Algorithm: Strongly connected components
- BFS/DFS: Graph traversal and reachability
- Centrality Metrics: Network importance measures

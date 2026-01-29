use super::wallet_graph::WalletGraph;
use std::cmp::Ordering;
/// Graph Algorithms for blockchain analysis
///
/// Implements advanced algorithms for:
/// - Shortest path finding (fund tracing)
/// - Connected components (wallet clustering)
/// - Centrality analysis (importance scoring)
/// - Cycle detection (wash trading patterns)
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Result of shortest path finding
#[derive(Debug, Clone)]
pub struct ShortestPath {
    pub path: Vec<String>,
    pub total_distance: f64,
    pub hop_count: usize,
    pub total_volume: u64,
}

/// A connected component in the graph
#[derive(Debug, Clone)]
pub struct ConnectedComponent {
    pub wallets: Vec<String>,
    pub size: usize,
    pub density: f64,
    pub total_volume: u64,
}

/// Centrality metrics for a node
#[derive(Debug, Clone)]
pub struct NodeCentrality {
    pub address: String,
    pub degree: usize,
    pub in_degree: usize,
    pub out_degree: usize,
    pub betweenness: f64,
    pub closeness: f64,
}

/// Graph algorithms implementation
pub struct GraphAlgorithms;

impl GraphAlgorithms {
    /// Find shortest path between two wallets using Dijkstra's algorithm
    /// Weight = 1/transaction_count (more transactions = shorter path)
    pub fn shortest_path(graph: &WalletGraph, from: &str, to: &str) -> Option<ShortestPath> {
        if from == to {
            return Some(ShortestPath {
                path: vec![from.to_string()],
                total_distance: 0.0,
                hop_count: 0,
                total_volume: 0,
            });
        }

        let mut distances: HashMap<String, f64> = HashMap::new();
        let mut previous: HashMap<String, String> = HashMap::new();
        let mut volumes: HashMap<String, u64> = HashMap::new();
        let mut heap = BinaryHeap::new();

        distances.insert(from.to_string(), 0.0);
        volumes.insert(from.to_string(), 0);
        heap.push(std::cmp::Reverse((OrderedFloat(0.0), from.to_string())));

        while let Some(std::cmp::Reverse((OrderedFloat(current_dist), current))) = heap.pop() {
            if current == to {
                // Reconstruct path
                let mut path = vec![to.to_string()];
                let mut node = to.to_string();

                while let Some(prev) = previous.get(&node) {
                    path.push(prev.clone());
                    node = prev.clone();
                }

                path.reverse();
                return Some(ShortestPath {
                    path: path.clone(),
                    total_distance: current_dist,
                    hop_count: path.len() - 1,
                    total_volume: *volumes.get(to).unwrap_or(&0),
                });
            }

            if let Some(&dist) = distances.get(&current) {
                if current_dist > dist {
                    continue;
                }
            }

            // Check outgoing edges
            for edge in graph.get_outgoing_edges(&current) {
                // Weight = 1 / (1 + transaction_count) - more transactions = lower weight
                let weight = 1.0 / (1.0 + edge.transaction_count as f64);
                let next_dist = current_dist + weight;

                let next_vol = volumes.get(&current).unwrap_or(&0) + edge.amount;

                if !distances.contains_key(&edge.to) || next_dist < distances[&edge.to] {
                    distances.insert(edge.to.clone(), next_dist);
                    volumes.insert(edge.to.clone(), next_vol);
                    previous.insert(edge.to.clone(), current.clone());
                    heap.push(std::cmp::Reverse((
                        OrderedFloat(next_dist),
                        edge.to.clone(),
                    )));
                }
            }
        }

        None
    }

    /// Find all shortest paths (BFS for unweighted shortest paths)
    pub fn all_shortest_paths(graph: &WalletGraph, from: &str, to: &str) -> Vec<ShortestPath> {
        if from == to {
            return vec![ShortestPath {
                path: vec![from.to_string()],
                total_distance: 0.0,
                hop_count: 0,
                total_volume: 0,
            }];
        }

        let mut paths = Vec::new();
        let mut visited = HashSet::new();
        Self::dfs_paths(
            graph,
            from,
            to,
            &mut visited,
            &mut vec![from.to_string()],
            &mut paths,
        );
        paths
    }

    /// Depth-first search for finding all paths
    fn dfs_paths(
        graph: &WalletGraph,
        current: &str,
        target: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        all_paths: &mut Vec<ShortestPath>,
    ) {
        if current == target {
            all_paths.push(ShortestPath {
                path: path.clone(),
                total_distance: path.len() as f64 - 1.0,
                hop_count: path.len() - 1,
                total_volume: Self::calculate_path_volume(graph, path),
            });
            return;
        }

        visited.insert(current.to_string());

        for neighbor in graph.get_neighbors(current) {
            if !visited.contains(&neighbor) {
                path.push(neighbor.clone());
                Self::dfs_paths(graph, &neighbor, target, visited, path, all_paths);
                path.pop();
            }
        }

        visited.remove(current);
    }

    /// Calculate total volume transferred along a path
    fn calculate_path_volume(graph: &WalletGraph, path: &[String]) -> u64 {
        let mut total = 0u64;
        for i in 0..path.len() - 1 {
            for edge in graph.get_outgoing_edges(&path[i]) {
                if edge.to == path[i + 1] {
                    total = total.saturating_add(edge.amount);
                }
            }
        }
        total
    }

    /// Find strongly connected components (SCC)
    pub fn tarjan_scc(graph: &WalletGraph) -> Vec<ConnectedComponent> {
        let mut index_counter = 0;
        let mut stack = Vec::new();
        let mut lowlinks: HashMap<String, usize> = HashMap::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut on_stack: HashSet<String> = HashSet::new();
        let mut components = Vec::new();

        for address in graph.nodes().keys() {
            if !index.contains_key(address) {
                Self::strongconnect(
                    address,
                    graph,
                    &mut index_counter,
                    &mut index,
                    &mut lowlinks,
                    &mut stack,
                    &mut on_stack,
                    &mut components,
                );
            }
        }

        components
    }

    fn strongconnect(
        v: &str,
        graph: &WalletGraph,
        index_counter: &mut usize,
        index: &mut HashMap<String, usize>,
        lowlinks: &mut HashMap<String, usize>,
        stack: &mut Vec<String>,
        on_stack: &mut HashSet<String>,
        components: &mut Vec<ConnectedComponent>,
    ) {
        index.insert(v.to_string(), *index_counter);
        lowlinks.insert(v.to_string(), *index_counter);
        *index_counter += 1;
        stack.push(v.to_string());
        on_stack.insert(v.to_string());

        for neighbor in graph.get_neighbors(v) {
            if !index.contains_key(&neighbor) {
                Self::strongconnect(
                    &neighbor,
                    graph,
                    index_counter,
                    index,
                    lowlinks,
                    stack,
                    on_stack,
                    components,
                );
                let neighbor_lowlink = *lowlinks.get(&neighbor).unwrap_or(&0);
                lowlinks.insert(
                    v.to_string(),
                    (*lowlinks.get(v).unwrap_or(&0)).min(neighbor_lowlink),
                );
            } else if on_stack.contains(&neighbor) {
                let neighbor_index = *index.get(&neighbor).unwrap_or(&0);
                lowlinks.insert(
                    v.to_string(),
                    (*lowlinks.get(v).unwrap_or(&0)).min(neighbor_index),
                );
            }
        }

        if lowlinks.get(v) == index.get(v) {
            let mut component = Vec::new();
            loop {
                let w = stack.pop().unwrap_or_default();
                on_stack.remove(&w);
                component.push(w.clone());
                if w == v {
                    break;
                }
            }

            let volume = Self::calculate_component_volume(graph, &component);
            components.push(ConnectedComponent {
                size: component.len(),
                wallets: component,
                density: 0.0, // Would be calculated
                total_volume: volume,
            });
        }
    }

    fn calculate_component_volume(graph: &WalletGraph, component: &[String]) -> u64 {
        let mut total = 0u64;
        for wallet in component {
            total = total.saturating_add(graph.get_outgoing_volume(wallet));
        }
        total
    }

    /// Detect cycles (potential wash trading patterns)
    pub fn find_cycles(graph: &WalletGraph, wallet: &str, max_depth: usize) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        Self::find_cycles_dfs(
            graph,
            wallet,
            wallet,
            &mut visited,
            &mut vec![wallet.to_string()],
            &mut cycles,
            max_depth,
        );
        cycles
    }

    fn find_cycles_dfs(
        graph: &WalletGraph,
        start: &str,
        current: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
        depth: usize,
    ) {
        if depth == 0 {
            return;
        }

        for neighbor in graph.get_neighbors(current) {
            if neighbor == start && path.len() > 2 {
                cycles.push(path.clone());
            } else if !visited.contains(&neighbor) && path.len() < depth {
                visited.insert(neighbor.clone());
                path.push(neighbor.clone());
                Self::find_cycles_dfs(graph, start, &neighbor, visited, path, cycles, depth - 1);
                path.pop();
                visited.remove(&neighbor);
            }
        }
    }

    /// Calculate degree centrality for a node
    pub fn degree_centrality(graph: &WalletGraph, address: &str) -> f64 {
        let max_possible = (graph.node_count() - 1) as f64;
        if max_possible == 0.0 {
            return 0.0;
        }
        let degree =
            (graph.get_neighbors(address).len() + graph.get_predecessors(address).len()) as f64;
        degree / max_possible
    }

    /// Calculate betweenness centrality (importance in paths between nodes)
    pub fn betweenness_centrality(graph: &WalletGraph, address: &str) -> f64 {
        let mut count = 0;
        let mut total_paths = 0;

        for node1 in graph.nodes().keys() {
            for node2 in graph.nodes().keys() {
                if node1 != node2 && node1 != address && node2 != address {
                    if let Some(path) = Self::shortest_path(graph, node1, node2) {
                        total_paths += 1;
                        if path.path.contains(&address.to_string()) {
                            count += 1;
                        }
                    }
                }
            }
        }

        if total_paths == 0 {
            return 0.0;
        }
        count as f64 / total_paths as f64
    }
}

/// Helper struct for priority queue ordering
#[derive(Copy, Clone, PartialEq)]
struct OrderedFloat(f64);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

/// Graph Analysis Example
/// Demonstrates the graph-based wallet analysis capabilities

#[allow(dead_code)]
#[cfg(test)]
mod graph_analysis_examples {
    use crate::graph::{GraphAlgorithms, GraphAnalysisEngine, GraphNode, WalletGraph};

    #[test]
    fn example_side_wallet_detection() {
        let mut engine = GraphAnalysisEngine::new();

        // Simulate a user with a main wallet and several side wallets
        let wallets = vec![
            ("main_wallet".to_string(), (50000, 100, 0.1)),
            ("side_wallet_1".to_string(), (5000, 20, 0.2)),
            ("side_wallet_2".to_string(), (3000, 15, 0.25)),
            ("side_wallet_3".to_string(), (8000, 25, 0.15)),
            ("trading_bot".to_string(), (100000, 500, 0.3)),
        ]
        .into_iter()
        .collect();

        engine.build_from_wallets(wallets);

        // Add fund flows from main to side wallets
        engine.add_fund_flow(
            "main_wallet".to_string(),
            "side_wallet_1".to_string(),
            2000,
            5,
            1000,
            true,
        );
        engine.add_fund_flow(
            "main_wallet".to_string(),
            "side_wallet_2".to_string(),
            1500,
            3,
            2000,
            true,
        );
        engine.add_fund_flow(
            "main_wallet".to_string(),
            "side_wallet_3".to_string(),
            3000,
            8,
            3000,
            true,
        );

        // Side wallets send to trading bot
        engine.add_fund_flow(
            "side_wallet_1".to_string(),
            "trading_bot".to_string(),
            1800,
            4,
            4000,
            true,
        );
        engine.add_fund_flow(
            "side_wallet_2".to_string(),
            "trading_bot".to_string(),
            1300,
            2,
            5000,
            true,
        );
        engine.add_fund_flow(
            "side_wallet_3".to_string(),
            "trading_bot".to_string(),
            2700,
            6,
            6000,
            true,
        );

        // Detect side wallets
        let candidates = engine.find_side_wallets("main_wallet");

        println!("Side wallet candidates for main_wallet:");
        for candidate in &candidates {
            println!(
                "  {} (confidence: {:.2}, hops: {})",
                candidate.address, candidate.confidence, candidate.hop_distance
            );
        }

        assert!(!candidates.is_empty());
    }

    #[test]
    fn example_exchange_route_tracing() {
        let mut engine = GraphAnalysisEngine::new();

        // Create a scenario with exchange intermediaries
        let wallets = vec![
            ("user_wallet".to_string(), (10000, 5, 0.1)),
            ("exchange_a".to_string(), (1000000, 10000, 0.5)),
            ("exchange_b".to_string(), (2000000, 15000, 0.5)),
            ("final_wallet".to_string(), (5000, 2, 0.3)),
        ]
        .into_iter()
        .collect();

        engine.build_from_wallets(wallets);

        // Route: user -> exchange_a -> exchange_b -> final
        engine.add_fund_flow(
            "user_wallet".to_string(),
            "exchange_a".to_string(),
            10000,
            1,
            1000,
            true,
        );
        engine.add_fund_flow(
            "exchange_a".to_string(),
            "exchange_b".to_string(),
            9500,
            1,
            2000,
            true,
        );
        engine.add_fund_flow(
            "exchange_b".to_string(),
            "final_wallet".to_string(),
            9000,
            1,
            3000,
            true,
        );

        // Trace routes
        let routes = engine.trace_exchange_routes("user_wallet", "final_wallet");

        println!("Exchange routes from user_wallet to final_wallet:");
        for route in &routes {
            println!(
                "  Path: {} ({} hops, {} volume)",
                route.path.join(" -> "),
                route.hops,
                route.total_volume
            );
        }

        assert!(!routes.is_empty());
    }

    #[test]
    fn example_wash_trading_detection() {
        let mut engine = GraphAnalysisEngine::new();

        // Create circular trading pattern
        let wallets = vec![
            ("bot_1".to_string(), (5000, 50, 0.3)),
            ("bot_2".to_string(), (5000, 50, 0.3)),
            ("bot_3".to_string(), (5000, 50, 0.3)),
        ]
        .into_iter()
        .collect();

        engine.build_from_wallets(wallets);

        // Create circular flows (bot_1 -> bot_2 -> bot_3 -> bot_1)
        engine.add_fund_flow(
            "bot_1".to_string(),
            "bot_2".to_string(),
            5000,
            20,
            1000,
            true,
        );
        engine.add_fund_flow(
            "bot_2".to_string(),
            "bot_3".to_string(),
            4900,
            19,
            2000,
            true,
        );
        engine.add_fund_flow(
            "bot_3".to_string(),
            "bot_1".to_string(),
            4800,
            18,
            3000,
            true,
        );

        // Detect wash trading
        let patterns = engine.detect_wash_trading("bot_1");

        println!("Wash trading patterns detected:");
        for pattern in &patterns {
            println!(
                "  Cycle: {} ({} hops, score: {:.2})",
                pattern.cycle.join(" -> "),
                pattern.cycle_length,
                pattern.suspicious_score
            );
        }

        assert!(!patterns.is_empty());
    }

    #[test]
    fn example_network_analysis() {
        let mut engine = GraphAnalysisEngine::new();

        // Create a more complex network
        let wallets = vec![
            ("hub_1".to_string(), (100000, 200, 0.4)),
            ("hub_2".to_string(), (80000, 150, 0.35)),
            ("leaf_1".to_string(), (1000, 5, 0.1)),
            ("leaf_2".to_string(), (1500, 7, 0.15)),
            ("leaf_3".to_string(), (2000, 8, 0.2)),
        ]
        .into_iter()
        .collect();

        engine.build_from_wallets(wallets);

        // Create hub-spoke topology
        engine.add_fund_flow(
            "hub_1".to_string(),
            "leaf_1".to_string(),
            5000,
            10,
            1000,
            true,
        );
        engine.add_fund_flow(
            "hub_1".to_string(),
            "leaf_2".to_string(),
            4000,
            8,
            2000,
            true,
        );
        engine.add_fund_flow(
            "hub_1".to_string(),
            "leaf_3".to_string(),
            6000,
            12,
            3000,
            true,
        );
        engine.add_fund_flow(
            "hub_2".to_string(),
            "leaf_1".to_string(),
            3000,
            5,
            4000,
            true,
        );
        engine.add_fund_flow(
            "hub_2".to_string(),
            "hub_1".to_string(),
            10000,
            3,
            5000,
            true,
        );

        // Detect network anomalies
        let anomalies = engine.detect_network_anomalies();

        println!("Network analysis results:");
        println!("  Unusual patterns: {}", anomalies.unusual_patterns);
        println!("  High-risk wallets: {}", anomalies.high_risk_wallets);
        println!("  Network density: {:.4}", anomalies.network_density);
        println!("  Largest cluster: {}", anomalies.largest_cluster_size);
    }

    #[test]
    fn example_shortest_path_analysis() {
        let mut engine = GraphAnalysisEngine::new();

        let wallets = vec![
            ("wallet_a".to_string(), (1000, 5, 0.1)),
            ("wallet_b".to_string(), (2000, 10, 0.2)),
            ("wallet_c".to_string(), (1500, 7, 0.15)),
            ("wallet_d".to_string(), (3000, 15, 0.25)),
        ]
        .into_iter()
        .collect();

        engine.build_from_wallets(wallets);

        // Create multiple paths
        engine.add_fund_flow(
            "wallet_a".to_string(),
            "wallet_b".to_string(),
            500,
            2,
            1000,
            true,
        );
        engine.add_fund_flow(
            "wallet_a".to_string(),
            "wallet_c".to_string(),
            300,
            1,
            2000,
            true,
        );
        engine.add_fund_flow(
            "wallet_b".to_string(),
            "wallet_d".to_string(),
            400,
            1,
            3000,
            true,
        );
        engine.add_fund_flow(
            "wallet_c".to_string(),
            "wallet_d".to_string(),
            250,
            2,
            4000,
            true,
        );

        // Find shortest path
        if let Some(path) = GraphAlgorithms::shortest_path(engine.graph(), "wallet_a", "wallet_d") {
            println!("Shortest path from wallet_a to wallet_d:");
            println!("  Route: {}", path.path.join(" -> "));
            println!("  Hops: {}", path.hop_count);
            println!("  Distance: {:.2}", path.total_distance);
            println!("  Volume: {}", path.total_volume);
        }
    }
}

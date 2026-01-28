// Example usage of OnChain Beast Solana RPC integration
// This file demonstrates how to use the RPC client for wallet analysis

use std::env;

mod core {
    pub mod errors {
        pub use std::fmt;

        #[derive(Debug)]
        pub enum BeastError {
            RpcError(String),
            InvalidAddress(String),
        }

        impl fmt::Display for BeastError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::RpcError(msg) => write!(f, "RPC Error: {}", msg),
                    Self::InvalidAddress(msg) => write!(f, "Invalid Address: {}", msg),
                }
            }
        }

        impl std::error::Error for BeastError {}

        pub type Result<T> = std::result::Result<T, BeastError>;
    }

    pub mod rpc_client {
        use serde::Deserialize;
        use super::errors::{BeastError, Result};

        #[derive(Clone)]
        pub struct SolanaRpcClient {
            endpoint: String,
            http_client: reqwest::Client,
        }

        impl SolanaRpcClient {
            pub fn new(endpoint: String) -> Self {
                SolanaRpcClient {
                    endpoint,
                    http_client: reqwest::Client::new(),
                }
            }

            pub async fn health_check(&self) -> Result<bool> {
                let body = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "getHealth"
                });

                match self.http_client.post(&self.endpoint)
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(response) => match response.json::<serde_json::Value>().await {
                        Ok(value) => {
                            let result = value.get("result").and_then(|r| r.as_str());
                            Ok(result == Some("ok"))
                        }
                        Err(_) => Ok(false),
                    },
                    Err(_) => Ok(false),
                }
            }

            pub async fn get_cluster_info(&self) -> Result<ClusterInfo> {
                let body = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "getClusterNodes"
                });

                match self.http_client.post(&self.endpoint)
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(response) => match response.json::<RpcResponse<Vec<NodeInfo>>>().await {
                        Ok(rpc_response) => {
                            let nodes = rpc_response.result.unwrap_or_default();
                            Ok(ClusterInfo {
                                total_nodes: nodes.len() as u64,
                                endpoint: self.endpoint.clone(),
                            })
                        }
                        Err(e) => Err(BeastError::RpcError(format!(
                            "Failed to parse cluster info: {}",
                            e
                        ))),
                    },
                    Err(e) => Err(BeastError::RpcError(format!(
                        "Failed to get cluster info: {}",
                        e
                    ))),
                }
            }
        }

        #[derive(Debug, Deserialize)]
        pub struct RpcResponse<T> {
            #[serde(default)]
            pub result: Option<T>,
        }

        #[derive(Debug, Deserialize)]
        pub struct NodeInfo {
            pub pubkey: String,
        }

        #[derive(Debug)]
        pub struct ClusterInfo {
            pub total_nodes: u64,
            pub endpoint: String,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use core::rpc_client::SolanaRpcClient;

    println!("🔗 Solana RPC Integration Example");
    println!("==================================\n");

    // Initialize RPC client with Solana mainnet endpoint
    let rpc = SolanaRpcClient::new(
        "https://api.mainnet-beta.solana.com".to_string()
    );

    // 1. Health Check
    println!("1️⃣  Checking RPC Health...");
    match rpc.health_check().await {
        Ok(healthy) => {
            println!("   Status: {}\n", if healthy { "✅ Healthy" } else { "❌ Unhealthy" });
        }
        Err(e) => {
            eprintln!("   Error: {}\n", e);
        }
    }

    // 2. Get Cluster Information
    println!("2️⃣  Fetching Cluster Information...");
    match rpc.get_cluster_info().await {
        Ok(cluster) => {
            println!("   Endpoint: {}", cluster.endpoint);
            println!("   Active Validators: {}\n", cluster.total_nodes);
        }
        Err(e) => {
            eprintln!("   Error: {}\n", e);
        }
    }

    println!("✅ Example complete!");

    Ok(())
}

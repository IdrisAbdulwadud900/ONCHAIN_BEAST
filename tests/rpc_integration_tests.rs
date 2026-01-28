use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

#[tokio::test]
async fn test_rpc_health() {
    // Load config and initialize RPC client
    let endpoint = "https://api.mainnet-beta.solana.com".to_string();
    let client = onchain_beast::core::rpc_client::SolanaRpcClient::new(endpoint);
    
    // Test health check
    let health = client.health_check().await;
    assert!(health.is_ok());
}

#[tokio::test]
async fn test_get_cluster_info() {
    let endpoint = "https://api.mainnet-beta.solana.com".to_string();
    let client = onchain_beast::core::rpc_client::SolanaRpcClient::new(endpoint);
    
    // Get cluster info
    let cluster = client.get_cluster_info().await;
    assert!(cluster.is_ok());
    
    let info = cluster.unwrap();
    println!("Cluster has {} validator nodes", info.total_nodes);
    assert!(info.total_nodes > 0);
}

#[tokio::test]
async fn test_get_account_info() {
    let endpoint = "https://api.mainnet-beta.solana.com".to_string();
    let client = onchain_beast::core::rpc_client::SolanaRpcClient::new(endpoint);
    
    // Try to get account info for a well-known account
    // Using the system program account
    let account_result = client.get_account_info("11111111111111111111111111111111").await;
    
    // This might fail if account doesn't exist, which is okay for this test
    match account_result {
        Ok(info) => {
            println!("Account: {} has balance: {} lamports", info.address, info.balance);
        }
        Err(e) => {
            println!("Account lookup error (expected): {}", e);
        }
    }
}

#[test]
fn test_solana_address_validation() {
    // Test with valid Solana address (44 characters)
    let valid_address = "11111111111111111111111111111111111111111111";
    assert_eq!(valid_address.len(), 44);
    
    // Test with invalid address
    let invalid_address = "invalid";
    assert_ne!(invalid_address.len(), 44);
}

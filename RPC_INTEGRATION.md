# Solana RPC Integration Guide

## Overview

OnChain Beast now includes full Solana RPC (Remote Procedure Call) integration for direct blockchain interaction and wallet analysis. This enables real-time queries of account data, transaction signatures, and cluster information.

## Features

### 1. Account Information
- **Method**: `get_account_info(address: &str)`
- **Returns**: `AccountInfo` with:
  - Wallet balance in lamports
  - Account owner
  - Executable status
  - Rent epoch
- **Validation**: Automatic Solana address format validation

### 2. Transaction Signatures
- **Method**: `get_signatures(address: &str, limit: u64)`
- **Returns**: List of `TransactionSignature` objects
- **Includes**: Signature, slot, block time, and memo
- **Limit**: Capped at 1000 per request

### 3. Transaction Details
- **Method**: `get_transaction(signature: &str)`
- **Returns**: `RpcTransaction` with block time and slot information
- **Encoding**: Supports JSON parsing format

### 4. Health Checks
- **Method**: `health_check()`
- **Returns**: Boolean indicating RPC endpoint health
- **Use Case**: Verify connectivity before analysis operations

### 5. Cluster Information
- **Method**: `get_cluster_info()`
- **Returns**: `ClusterInfo` with active validator count
- **Use Case**: Monitor network health and participation

## Setup

### Environment Variables

```bash
# Default: https://api.mainnet-beta.solana.com
export SOLANA_RPC_ENDPOINT="https://api.mainnet-beta.solana.com"

# Optional: Custom RPC endpoint (e.g., Devnet)
export SOLANA_RPC_ENDPOINT="https://api.devnet.solana.com"
```

### Dependencies Added

```toml
solana-sdk = "1.18"
solana-client = "1.18"
solana-rpc-client = "1.18"
```

## Usage Examples

### Basic Initialization

```rust
use onchain_beast::core::rpc_client::SolanaRpcClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rpc = SolanaRpcClient::new(
        "https://api.mainnet-beta.solana.com".to_string()
    );
    
    // Verify connection
    if rpc.health_check().await? {
        println!("Connected to Solana!");
    }
    
    Ok(())
}
```

### Get Account Information

```rust
let account_info = rpc.get_account_info("11111111111111111111111111111111").await?;

println!("Address: {}", account_info.address);
println!("Balance: {} lamports", account_info.balance);
println!("Owner: {}", account_info.owner);
println!("Executable: {}", account_info.executable);
```

### Fetch Transaction History

```rust
let wallet = "YOUR_WALLET_ADDRESS";
let signatures = rpc.get_signatures(wallet, 50).await?;

for sig in signatures {
    println!("Tx: {}", sig.signature);
    println!("  Slot: {}", sig.slot);
    println!("  Time: {}", sig.block_time);
    if let Some(memo) = sig.memo {
        println!("  Memo: {}", memo);
    }
}
```

### Get Transaction Details

```rust
let tx_signature = "YOUR_TX_SIGNATURE";
let transaction = rpc.get_transaction(tx_signature).await?;

println!("Signature: {}", transaction.signature);
println!("Slot: {}", transaction.slot);
println!("Block Time: {}", transaction.block_time);
```

### Monitor Network Health

```rust
let cluster = rpc.get_cluster_info().await?;
println!("Active Validators: {}", cluster.total_nodes);
println!("Endpoint: {}", cluster.endpoint);

// Periodically check health
if rpc.health_check().await? {
    println!("RPC endpoint is responsive");
}
```

## Error Handling

The RPC client returns `Result<T>` with proper error types:

```rust
use onchain_beast::core::errors::BeastError;

match rpc.get_account_info(address).await {
    Ok(info) => {
        println!("Account found: {} lamports", info.balance);
    }
    Err(BeastError::InvalidAddress(msg)) => {
        eprintln!("Invalid address: {}", msg);
    }
    Err(BeastError::RpcError(msg)) => {
        eprintln!("RPC error: {}", msg);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Data Structures

### AccountInfo
```rust
pub struct AccountInfo {
    pub address: String,        // Wallet address
    pub balance: u64,           // Balance in lamports
    pub owner: String,          // Owner program ID
    pub executable: bool,       // Is executable program account
    pub rent_epoch: u64,        // Rent epoch
}
```

### TransactionSignature
```rust
pub struct TransactionSignature {
    pub signature: String,      // Transaction signature
    pub slot: u64,              // Blockchain slot
    pub block_time: u64,        // Unix timestamp
    pub memo: Option<String>,   // Optional memo
}
```

### RpcTransaction
```rust
pub struct RpcTransaction {
    pub signature: String,      // Transaction signature
    pub block_time: u64,        // Unix timestamp
    pub slot: u64,              // Blockchain slot
}
```

### ClusterInfo
```rust
pub struct ClusterInfo {
    pub total_nodes: u64,       // Active validator count
    pub endpoint: String,       // RPC endpoint URL
}
```

## Performance Considerations

### Rate Limiting
- Default RPC endpoints have rate limits (~100 requests/minute for free tier)
- Consider implementing retry logic for production use

### Batch Operations
```rust
// Better: Batch multiple requests
let wallets = vec!["wallet1", "wallet2", "wallet3"];
let results = futures::future::join_all(
    wallets.iter().map(|w| rpc.get_account_info(w))
).await;
```

### Caching
The database layer can cache frequently accessed data:
```rust
match db.get_wallet(address).await? {
    Some(cached_data) => {
        // Use cached data
    }
    None => {
        // Fetch from RPC and cache
        let info = rpc.get_account_info(address).await?;
        db.save_wallet(address, &serde_json::to_string(&info)?).await?;
    }
}
```

## Integration with Analysis Engine

The RPC client is integrated with the analysis pipeline:

```rust
let rpc = SolanaRpcClient::new(config.rpc_endpoint);
let sigs = rpc.get_signatures("wallet_address", 100).await?;

for sig in sigs {
    let tx = rpc.get_transaction(&sig.signature).await?;
    // Process transaction for wallet clustering analysis
}
```

## Advanced Features

### Custom Endpoints
```rust
// Devnet
let devnet_rpc = SolanaRpcClient::new(
    "https://api.devnet.solana.com".to_string()
);

// Testnet
let testnet_rpc = SolanaRpcClient::new(
    "https://api.testnet.solana.com".to_string()
);

// Custom RPC provider
let custom_rpc = SolanaRpcClient::new(
    "https://your-custom-rpc.com".to_string()
);
```

### JSON-RPC Protocol
All methods use Solana's JSON-RPC 2.0 specification:
- Proper error handling with RPC error codes
- Method-specific parameter validation
- Standardized response parsing

## Testing

Run the integration tests:
```bash
cargo test --test rpc_integration_tests
```

Run the example:
```bash
cargo run --example solana_integration
```

## Troubleshooting

### Connection Issues
```
Error: Failed to get account info: Connection refused
```
- Check RPC endpoint is accessible
- Verify network connectivity
- Try alternative RPC provider

### Invalid Address
```
Error: Invalid wallet address: Invalid Solana address length
```
- Ensure address is 44 characters (base58 encoded)
- Validate address format before querying

### Rate Limits
```
Error: RPC Error: 429 Too Many Requests
```
- Implement exponential backoff retry logic
- Use a private RPC endpoint for production
- Cache frequently accessed data

## Future Enhancements

- [ ] WebSocket subscription support for real-time updates
- [ ] Program instruction parser for token transfers
- [ ] Batch RPC calls optimization
- [ ] Custom RPC middleware for response caching
- [ ] Solana token program integration (SPL)
- [ ] NFT metadata fetching
- [ ] DeFi protocol interaction analysis

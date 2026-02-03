/// Solana RPC Client wrapper for blockchain interactions
use crate::core::errors::{BeastError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

#[derive(Clone)]
pub struct SolanaRpcClient {
    endpoint: String,
    http_client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
    max_retries: usize,
}

struct RateLimiter {
    min_interval: Duration,
    next_allowed: Mutex<Instant>,
}

impl RateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            next_allowed: Mutex::new(Instant::now()),
        }
    }

    async fn acquire(&self) {
        if self.min_interval.is_zero() {
            return;
        }

        let mut next = self.next_allowed.lock().await;
        let now = Instant::now();
        if *next > now {
            sleep(*next - now).await;
        }
        *next = Instant::now() + self.min_interval;
    }
}

impl SolanaRpcClient {
    pub fn new(endpoint: String) -> Self {
        // `reqwest::Client::new()` can read system proxy configuration on macOS.
        // In sandboxed environments this can panic (SystemConfiguration returning NULL),
        // so we explicitly disable proxy auto-detection.
        let http_client = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(25))
            .build()
            .expect("Failed to build reqwest client");

        let min_interval_ms = std::env::var("RPC_MIN_INTERVAL_MS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(120);

        let max_retries = std::env::var("RPC_MAX_RETRIES")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(5)
            .clamp(1, 15);

        SolanaRpcClient {
            endpoint,
            http_client,
            rate_limiter: Arc::new(RateLimiter::new(Duration::from_millis(min_interval_ms))),
            max_retries,
        }
    }

    /// Get account information from Solana blockchain
    pub async fn get_account_info(&self, address: &str) -> Result<AccountInfo> {
        // Validate Solana address format
        if address.len() != 44 && address.len() != 32 {
            return Err(BeastError::InvalidAddress(format!(
                "Invalid Solana address length: {}",
                address.len()
            )));
        }

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [address, { "encoding": "jsonParsed" }]
        });

        self.rate_limiter.acquire().await;
        match self
            .http_client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
        {
            Ok(response) => match response.json::<RpcResponse<AccountData>>().await {
                Ok(rpc_response) => {
                    if let Some(data) = rpc_response.result {
                        Ok(AccountInfo {
                            address: address.to_string(),
                            balance: data.value.lamports,
                            owner: data.value.owner,
                            executable: data.value.executable,
                            rent_epoch: data.value.rent_epoch,
                        })
                    } else {
                        Err(BeastError::RpcError("Account not found".to_string()))
                    }
                }
                Err(e) => Err(BeastError::RpcError(format!(
                    "Failed to parse account info response: {}",
                    e
                ))),
            },
            Err(e) => Err(BeastError::RpcError(format!(
                "Failed to get account info: {}",
                e
            ))),
        }
    }

    /// Get transaction signatures for a wallet
    pub async fn get_signatures(
        &self,
        address: &str,
        limit: u64,
    ) -> Result<Vec<TransactionSignature>> {
        // Validate Solana address format
        if address.len() != 44 && address.len() != 32 {
            return Err(BeastError::InvalidAddress(format!(
                "Invalid Solana address length: {}",
                address.len()
            )));
        }

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignaturesForAddress",
            "params": [address, { "limit": limit.min(1000) }]
        });

        for attempt in 0..self.max_retries {
            self.rate_limiter.acquire().await;

            let resp = match self
                .http_client
                .post(&self.endpoint)
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    if attempt + 1 < self.max_retries {
                        sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    return Err(BeastError::RpcError(format!(
                        "Failed to get signatures: {}",
                        e
                    )));
                }
            };

            let status = resp.status();
            let text = resp.text().await.map_err(|e| {
                BeastError::RpcError(format!("Failed to read signatures response: {}", e))
            })?;

            if !status.is_success() {
                if status.as_u16() == 429 && attempt + 1 < self.max_retries {
                    // Exponential backoff capped at ~3s.
                    let backoff = (250_u64 << attempt.min(4)).min(3_000);
                    sleep(Duration::from_millis(backoff)).await;
                    continue;
                }
                return Err(BeastError::RpcError(format!(
                    "RPC HTTP {}: {}",
                    status.as_u16(),
                    text
                )));
            }

            let rpc_response: RpcResponse<Vec<SignatureData>> =
                serde_json::from_str(&text).map_err(|e| {
                    BeastError::RpcError(format!("Failed to parse signatures response: {}", e))
                })?;

            if let Some(err) = rpc_response.error {
                if err.code == 429 && attempt + 1 < self.max_retries {
                    let backoff = (250_u64 << attempt.min(4)).min(3_000);
                    sleep(Duration::from_millis(backoff)).await;
                    continue;
                }
                return Err(BeastError::RpcError(format!(
                    "RPC error {}: {}",
                    err.code, err.message
                )));
            }

            let sigs = rpc_response.result.unwrap_or_default();
            let signatures = sigs
                .into_iter()
                .map(|sig_data| TransactionSignature {
                    signature: sig_data.signature,
                    slot: sig_data.slot,
                    block_time: sig_data.block_time.unwrap_or(0),
                    memo: sig_data.memo,
                })
                .collect();
            return Ok(signatures);
        }

        Ok(vec![])
    }

    /// Get full transaction details with enhanced data
    pub async fn get_transaction(&self, signature: &str) -> Result<RpcTransaction> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                signature,
                {
                    "encoding": "jsonParsed",
                    "maxSupportedTransactionVersion": 0,
                    "commitment": "confirmed"
                }
            ]
        });

        // The public Solana RPC can occasionally return `result: null` for very recent
        // transactions and frequently rate-limits (`429`). We retry with backoff.
        for attempt in 0..self.max_retries {
            self.rate_limiter.acquire().await;

            let resp = match self.http_client.post(&self.endpoint).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    if attempt + 1 < self.max_retries {
                        sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    return Err(BeastError::RpcError(format!(
                        "Failed to get transaction: {}",
                        e
                    )));
                }
            };

            let status = resp.status();
            let text = resp
                .text()
                .await
                .map_err(|e| BeastError::RpcError(format!("Failed to read RPC response: {}", e)))?;

            if !status.is_success() {
                if status.as_u16() == 429 && attempt + 1 < self.max_retries {
                    let backoff = (250_u64 << attempt.min(4)).min(3_000);
                    sleep(Duration::from_millis(backoff)).await;
                    continue;
                }
                return Err(BeastError::RpcError(format!(
                    "RPC HTTP {}: {}",
                    status.as_u16(),
                    text
                )));
            }

            let rpc_response: RpcResponse<TransactionData> = serde_json::from_str(&text).map_err(|e| {
                BeastError::RpcError(format!("Failed to parse transaction response: {}", e))
            })?;

            if let Some(err) = rpc_response.error {
                // Surface the real JSON-RPC error instead of masking it as "not found".
                if err.code == 429 && attempt + 1 < self.max_retries {
                    let backoff = (250_u64 << attempt.min(4)).min(3_000);
                    sleep(Duration::from_millis(backoff)).await;
                    continue;
                }
                return Err(BeastError::RpcError(format!(
                    "RPC error {}: {}",
                    err.code, err.message
                )));
            }

            if let Some(tx_data) = rpc_response.result {
                let meta = tx_data.meta.as_ref();
                let fee = meta
                    .and_then(|m| m.get("fee"))
                    .and_then(|f| f.as_u64())
                    .unwrap_or(0);
                let error = meta.and_then(|m| m.get("err")).map(|e| format!("{:?}", e));
                let success = error.is_none();

                return Ok(RpcTransaction {
                    signature: signature.to_string(),
                    block_time: tx_data.block_time.unwrap_or(0),
                    slot: tx_data.slot,
                    fee,
                    success,
                    error,
                    raw_data: serde_json::to_value(&tx_data).unwrap_or(serde_json::Value::Null),
                });
            }

            if attempt + 1 < self.max_retries {
                sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
                continue;
            }

            return Err(BeastError::RpcError("Transaction not found".to_string()));
        }

        Err(BeastError::RpcError("Transaction not found".to_string()))
    }

    /// Check if RPC endpoint is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getHealth"
        });

        self.rate_limiter.acquire().await;
        match self
            .http_client
            .post(&self.endpoint)
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

    /// Get current cluster info
    pub async fn get_cluster_info(&self) -> Result<ClusterInfo> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getClusterNodes"
        });

        self.rate_limiter.acquire().await;
        match self
            .http_client
            .post(&self.endpoint)
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

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub address: String,
    pub balance: u64,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
}

#[derive(Debug, Clone)]
pub struct TransactionSignature {
    pub signature: String,
    pub slot: u64,
    pub block_time: u64,
    pub memo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RpcTransaction {
    pub signature: String,
    pub block_time: u64,
    pub slot: u64,
    pub fee: u64,
    pub success: bool,
    pub error: Option<String>,
    pub raw_data: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ClusterInfo {
    pub total_nodes: u64,
    pub endpoint: String,
}

// RPC Response structures for proper deserialization
#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    #[serde(default)]
    result: Option<T>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct AccountData {
    value: AccountValue,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct AccountValue {
    lamports: u64,
    owner: String,
    executable: bool,
    rent_epoch: u64,
}

#[derive(Debug, Deserialize)]
struct SignatureData {
    signature: String,
    slot: u64,
    #[serde(default, rename = "blockTime")]
    block_time: Option<u64>,
    #[serde(default)]
    memo: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct TransactionData {
    slot: u64,
    #[serde(default, rename = "blockTime")]
    block_time: Option<u64>,
    #[serde(default)]
    meta: Option<serde_json::Value>,
    #[serde(default)]
    transaction: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct NodeInfo {
}

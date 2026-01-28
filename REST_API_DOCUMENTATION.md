# OnChain Beast REST API Documentation

## Overview

The OnChain Beast REST API provides comprehensive endpoints for Solana blockchain analysis, wallet tracking, pattern detection, and network monitoring. Built with Actix-web for high performance and reliability.

**Base URL**: `http://localhost:8080` (default)

**API Version**: v1.0

---

## Server Setup

### Environment Variables

```bash
API_HOST=0.0.0.0          # API server host (default: 127.0.0.1)
API_PORT=8080             # API server port (default: 8080)
RPC_ENDPOINT=https://api.mainnet-beta.solana.com  # Solana RPC endpoint
```

### Starting the Server

```bash
cargo build --release
./target/release/onchain_beast

# OR with custom environment variables
API_HOST=0.0.0.0 API_PORT=3000 cargo run --release
```

---

## API Endpoints

### 1. Health & Status Endpoints

#### GET `/health`
Health check endpoint that verifies RPC connection status.

**Response (200 OK)**:
```json
{
  "status": "healthy",
  "service": "onchain_beast",
  "rpc": "connected"
}
```

**Response (503 Service Unavailable)**:
```json
{
  "status": "unhealthy",
  "service": "onchain_beast",
  "rpc": "disconnected"
}
```

---

#### GET `/status`
Get comprehensive system status including cluster information.

**Response (200 OK)**:
```json
{
  "status": "operational",
  "cluster": {
    "nodes": 456,
    "active_validators": 456
  },
  "timestamp": "2024-01-15T10:30:45.123Z"
}
```

---

#### GET `/`
Root endpoint with service information and available endpoints.

**Response (200 OK)**:
```json
{
  "service": "OnChain Beast - Solana Blockchain Analysis",
  "version": "0.1.0",
  "description": "Powerful on-chain analysis tool for Solana blockchain",
  "endpoints": {
    "health": "/health",
    "status": "/status",
    "wallet_analysis": "/api/v1/analyze/wallet/{address}",
    "side_wallets": "/api/v1/wallet/{address}/side-wallets",
    "fund_tracing": "/api/v1/trace/funds",
    "pattern_detection": "/api/v1/detect/patterns"
  }
}
```

---

### 2. Wallet Analysis Endpoints

#### GET `/api/v1/analyze/wallet/{address}`
Analyze a single wallet address and retrieve basic information.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "balance_lamports": 5000000000,
  "balance_sol": 5.0,
  "owner": "TokenkegQfeZyiNwAJsyFbPVwwQQfKP",
  "executable": false,
  "analysis_ready": true
}
```

**Response (404 Not Found)**:
```json
{
  "error": "Wallet not found: Account not found",
  "wallet": "invalid_address"
}
```

---

#### POST `/api/v1/analyze/wallet`
Analyze wallet with additional options.

**Request Body**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "include_transactions": true,
  "depth": 3
}
```

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "balance_lamports": 5000000000,
  "balance_sol": 5.0,
  "owner": "TokenkegQfeZyiNwAJsyFbPVwwQQfKP",
  "executable": false,
  "recent_transactions": 42
}
```

---

#### GET `/api/v1/wallet/{address}/risk`
Calculate wallet risk score based on transaction history.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "risk_score": 0.45,
  "transaction_count": 87,
  "risk_level": "medium"
}
```

**Risk Levels**:
- `low`: risk_score < 0.3
- `medium`: 0.3 ≤ risk_score < 0.6
- `high`: risk_score ≥ 0.6

---

#### GET `/api/v1/wallet/{address}/transactions`
Get recent transactions for a wallet.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Query Parameters**:
- `limit` (integer, optional): Number of transactions to return (default: 10, max: 100)
- `before` (string, optional): Get transactions before this signature

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "transactions": [
    {
      "signature": "5sKzxq...",
      "slot": 123456789,
      "block_time": 1705327845
    },
    {
      "signature": "7kLmn9...",
      "slot": 123456788,
      "block_time": 1705327840
    }
  ],
  "total": 342,
  "returned": 2
}
```

**Example Requests**:
```bash
# Get 5 transactions
curl http://localhost:8080/api/v1/wallet/{address}/transactions?limit=5

# Get 50 transactions
curl http://localhost:8080/api/v1/wallet/{address}/transactions?limit=50
```

---

### 3. Side Wallet Detection (Graph Analysis)

#### GET `/api/v1/wallet/{address}/side-wallets`
Find potential side wallets connected to a primary wallet using graph analysis.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "main_wallet": "11111111111111111111111111111111",
  "side_wallets": [],
  "confidence_threshold": 0.7,
  "analysis_depth": 3,
  "message": "Graph analysis integration pending"
}
```

---

#### GET `/api/v1/wallet/{address}/cluster`
Get wallet cluster information (connected wallets through transaction patterns).

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "primary_wallet": "11111111111111111111111111111111",
  "cluster_size": 1,
  "wallets": ["11111111111111111111111111111111"],
  "connection_strength": 1.0,
  "message": "Graph analysis integration pending"
}
```

---

### 4. Fund Tracing Endpoints

#### POST `/api/v1/trace/funds`
Trace fund movements between two wallets or addresses.

**Request Body**:
```json
{
  "from": "wallet_address_1",
  "to": "wallet_address_2",
  "max_depth": 5
}
```

**Response (200 OK)**:
```json
{
  "from": "wallet_address_1",
  "to": "wallet_address_2",
  "paths_found": 0,
  "status": "analysis_ready",
  "message": "Graph analysis integration pending"
}
```

---

#### POST `/api/v1/trace/exchange-routes`
Trace fund routes through exchange accounts.

**Request Body**:
```json
{
  "source": "wallet_address_1",
  "destination": "wallet_address_2"
}
```

**Response (200 OK)**:
```json
{
  "source": "wallet_address_1",
  "destination": "wallet_address_2",
  "routes": [],
  "exchanges_detected": 0,
  "message": "Graph analysis integration pending"
}
```

---

### 5. Pattern Detection Endpoints

#### POST `/api/v1/detect/patterns`
Detect trading or transaction patterns in wallet activity.

**Request Body**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "pattern_type": "wash_trading"
}
```

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "patterns_detected": [],
  "anomaly_score": 0.0,
  "analysis_type": "wash_trading",
  "message": "Pattern detection integration pending"
}
```

**Supported Pattern Types**:
- `wash_trading`: Detect circular trading patterns
- `pump_and_dump`: Detect coordinated price manipulation
- `front_running`: Detect transaction ordering issues
- `mev_exploit`: Detect MEV-related activities

---

#### GET `/api/v1/detect/anomalies`
Detect network-wide anomalies and unusual activities.

**Response (200 OK)**:
```json
{
  "unusual_wallets": [],
  "suspicious_patterns": 0,
  "high_risk_count": 0,
  "analysis_timestamp": "2024-01-15T10:30:45.123Z"
}
```

---

#### GET `/api/v1/detect/wash-trading/{address}`
Detect wash trading patterns for a specific wallet.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "wash_trading_detected": false,
  "cycles_found": 0,
  "suspicious_score": 0.0,
  "message": "Graph analysis integration pending"
}
```

---

### 6. Network Analysis Endpoints

#### GET `/api/v1/network/metrics`
Get overall network metrics and health information.

**Response (200 OK)**:
```json
{
  "network": "solana",
  "cluster": "mainnet-beta",
  "active_validators": 456,
  "network_health": "good",
  "tps": 400,
  "timestamp": "2024-01-15T10:30:45.123Z"
}
```

---

#### POST `/api/v1/network/analysis`
Perform comprehensive network analysis.

**Request Body**:
```json
{
  "analysis_type": "correlation"
}
```

**Response (200 OK)**:
```json
{
  "analysis_type": "correlation",
  "wallets_analyzed": 0,
  "relationships_found": 0,
  "clusters_detected": 0,
  "message": "Network analysis integration pending"
}
```

**Supported Analysis Types**:
- `correlation`: Find correlated wallet activities
- `cluster_detection`: Identify wallet clusters
- `hierarchy`: Detect hierarchical relationships
- `anomaly`: Find network anomalies

---

### 7. Account Information Endpoints

#### GET `/api/v1/account/{address}/balance`
Get account balance in lamports and SOL.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "wallet": "11111111111111111111111111111111",
  "balance_lamports": 5000000000,
  "balance_sol": 5.0,
  "owner": "TokenkegQfeZyiNwAJsyFbPVwwQQfKP"
}
```

**Response (404 Not Found)**:
```json
{
  "error": "Account not found",
  "wallet": "invalid_address"
}
```

---

#### GET `/api/v1/account/{address}/info`
Get detailed account information.

**Path Parameters**:
- `address` (string, required): Solana wallet address

**Response (200 OK)**:
```json
{
  "address": "11111111111111111111111111111111",
  "balance_lamports": 5000000000,
  "balance_sol": 5.0,
  "owner": "TokenkegQfeZyiNwAJsyFbPVwwQQfKP",
  "executable": false,
  "rent_epoch": 500
}
```

---

### 8. Cluster Information Endpoints

#### GET `/api/v1/cluster/info`
Get Solana cluster information.

**Response (200 OK)**:
```json
{
  "cluster": "mainnet-beta",
  "total_validators": 456,
  "network_version": "1.18",
  "health": "operational"
}
```

---

#### GET `/api/v1/cluster/health`
Get cluster health status.

**Response (200 OK)**:
```json
{
  "cluster": "solana-mainnet",
  "health": "healthy",
  "rpc_status": "operational"
}
```

**Response (503 Service Unavailable)**:
```json
{
  "cluster": "solana-mainnet",
  "health": "unhealthy",
  "error": "RPC connection failed"
}
```

---

## Error Responses

### Common HTTP Status Codes

| Status Code | Description |
|------------|-------------|
| 200 | Success |
| 400 | Bad request (invalid parameters) |
| 404 | Resource not found |
| 500 | Internal server error |
| 503 | Service unavailable |

### Error Response Format

```json
{
  "error": "Description of what went wrong",
  "details": "Additional context (if applicable)"
}
```

---

## Authentication

Currently, the API operates without authentication. For production use, implement:

- API key authentication
- JWT tokens
- OAuth2 integration
- Rate limiting per client

---

## Rate Limiting

Default rate limits (to be implemented):
- 1000 requests per minute per IP
- 10 requests per second for intensive operations

---

## Response Codes and Examples

### Success Responses

**200 OK**: Standard success response
```json
{
  "status": "success",
  "data": { /* response data */ }
}
```

### Error Responses

**400 Bad Request**: Invalid parameters
```json
{
  "error": "Invalid wallet address format",
  "code": "INVALID_ADDRESS"
}
```

**404 Not Found**: Resource doesn't exist
```json
{
  "error": "Wallet not found",
  "wallet": "invalid_address"
}
```

**500 Internal Server Error**: Server-side issue
```json
{
  "error": "Internal server error",
  "request_id": "req_12345"
}
```

---

## Usage Examples

### cURL

```bash
# Health check
curl http://localhost:8080/health

# Analyze wallet
curl http://localhost:8080/api/v1/analyze/wallet/11111111111111111111111111111111

# Get wallet transactions with limit
curl "http://localhost:8080/api/v1/wallet/11111111111111111111111111111111/transactions?limit=25"

# POST wallet analysis
curl -X POST http://localhost:8080/api/v1/analyze/wallet \
  -H "Content-Type: application/json" \
  -d '{"wallet":"11111111111111111111111111111111","include_transactions":true}'

# Trace funds
curl -X POST http://localhost:8080/api/v1/trace/funds \
  -H "Content-Type: application/json" \
  -d '{"from":"addr1","to":"addr2","max_depth":5}'

# Detect patterns
curl -X POST http://localhost:8080/api/v1/detect/patterns \
  -H "Content-Type: application/json" \
  -d '{"wallet":"11111111111111111111111111111111","pattern_type":"wash_trading"}'
```

### Python (requests)

```python
import requests

BASE_URL = "http://localhost:8080"

# Health check
response = requests.get(f"{BASE_URL}/health")
print(response.json())

# Analyze wallet
wallet = "11111111111111111111111111111111"
response = requests.get(f"{BASE_URL}/api/v1/analyze/wallet/{wallet}")
print(response.json())

# POST wallet analysis with options
data = {
    "wallet": "11111111111111111111111111111111",
    "include_transactions": True,
    "depth": 3
}
response = requests.post(f"{BASE_URL}/api/v1/analyze/wallet", json=data)
print(response.json())

# Get transactions with limit
response = requests.get(
    f"{BASE_URL}/api/v1/wallet/{wallet}/transactions",
    params={"limit": 25}
)
print(response.json())
```

### JavaScript/Node.js

```javascript
const BASE_URL = "http://localhost:8080";

// Health check
fetch(`${BASE_URL}/health`)
  .then(res => res.json())
  .then(data => console.log(data));

// Analyze wallet
const wallet = "11111111111111111111111111111111";
fetch(`${BASE_URL}/api/v1/analyze/wallet/${wallet}`)
  .then(res => res.json())
  .then(data => console.log(data));

// POST wallet analysis
fetch(`${BASE_URL}/api/v1/analyze/wallet`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({
    wallet: "11111111111111111111111111111111",
    include_transactions: true,
    depth: 3
  })
})
  .then(res => res.json())
  .then(data => console.log(data));
```

---

## Performance Notes

- Account info queries: ~100-200ms
- Transaction history (limit 10): ~150-300ms
- Graph analysis (depth 3): ~500-1000ms
- Network metrics: ~50-100ms

For optimal performance:
- Use query limits to reduce response sizes
- Implement client-side caching for frequently accessed data
- Use pagination for large result sets
- Batch requests when possible

---

## Roadmap

### Phase 1 (Current)
- ✅ Core API structure with Actix-web
- ✅ Basic wallet analysis endpoints
- ✅ Account information endpoints
- ✅ Cluster status endpoints
- ⏳ Graph analysis integration

### Phase 2 (Planned)
- Graph-based side wallet detection
- Advanced pattern detection
- Fund tracing across wallets
- Exchange route mapping

### Phase 3 (Planned)
- Real-time monitoring endpoints
- WebSocket support for live updates
- Advanced filtering and search
- Data export endpoints

### Phase 4 (Planned)
- Machine learning predictions
- Risk scoring improvements
- Custom analytics dashboards
- Historical data analysis

---

## Support

For issues, feature requests, or questions:
- Open an issue on GitHub
- Check existing documentation
- Review examples in `/examples`

---

## License

Part of the OnChain Beast project.

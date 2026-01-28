# REST API Quick Start Guide

## Overview

The OnChain Beast REST API is a high-performance Solana blockchain analysis tool providing 20+ endpoints for wallet analysis, pattern detection, and network monitoring.

---

## 🚀 Quick Start

### 1. Build the Project

```bash
cd onchain_beast
cargo build --release
```

### 2. Start the Server

```bash
# Using default settings (localhost:8080)
./target/release/onchain_beast

# OR with custom host/port
API_HOST=0.0.0.0 API_PORT=3000 ./target/release/onchain_beast

# OR with custom RPC endpoint
RPC_ENDPOINT=https://api.mainnet-beta.solana.com ./target/release/onchain_beast
```

### 3. Test the API

```bash
# Health check
curl http://localhost:8080/health

# Get cluster status
curl http://localhost:8080/status

# Analyze a wallet
curl http://localhost:8080/api/v1/analyze/wallet/11111111111111111111111111111111
```

---

## 🔧 Configuration

### Environment Variables

```bash
# Server Configuration
API_HOST=127.0.0.1          # Default: 127.0.0.1 (localhost)
API_PORT=8080               # Default: 8080
RUST_LOG=info               # Logging level: debug, info, warn, error

# RPC Configuration
RPC_ENDPOINT=https://api.mainnet-beta.solana.com  # Default RPC endpoint
```

### Example: Production Setup

```bash
# Start on all interfaces, port 3000, with verbose logging
API_HOST=0.0.0.0 \
API_PORT=3000 \
RUST_LOG=debug \
RPC_ENDPOINT=https://api.mainnet-beta.solana.com \
./target/release/onchain_beast
```

---

## 📊 Endpoint Categories

### Health & Status (3 endpoints)
- `GET /health` - Health check
- `GET /status` - System status
- `GET /` - Service info

### Wallet Analysis (5 endpoints)
- `GET /api/v1/analyze/wallet/{address}` - Analyze wallet
- `POST /api/v1/analyze/wallet` - Analyze with options
- `GET /api/v1/wallet/{address}/risk` - Risk score
- `GET /api/v1/wallet/{address}/transactions` - Transaction history
- `GET /api/v1/wallet/{address}/...` - More wallet endpoints

### Pattern Detection (3 endpoints)
- `POST /api/v1/detect/patterns` - Detect patterns
- `GET /api/v1/detect/anomalies` - Network anomalies
- `GET /api/v1/detect/wash-trading/{address}` - Wash trading detection

### Graph Analysis (2 endpoints)
- `GET /api/v1/wallet/{address}/side-wallets` - Find side wallets
- `GET /api/v1/wallet/{address}/cluster` - Get wallet cluster

### Fund Tracing (2 endpoints)
- `POST /api/v1/trace/funds` - Trace fund movements
- `POST /api/v1/trace/exchange-routes` - Trace exchange routes

### Network Analysis (2 endpoints)
- `GET /api/v1/network/metrics` - Network metrics
- `POST /api/v1/network/analysis` - Network analysis

### Account Info (2 endpoints)
- `GET /api/v1/account/{address}/balance` - Get balance
- `GET /api/v1/account/{address}/info` - Get account info

### Cluster Info (2 endpoints)
- `GET /api/v1/cluster/info` - Cluster info
- `GET /api/v1/cluster/health` - Cluster health

---

## 💡 Common Usage Patterns

### 1. Check Service Health

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "service": "onchain_beast",
  "rpc": "connected"
}
```

### 2. Analyze a Wallet

```bash
curl http://localhost:8080/api/v1/analyze/wallet/11111111111111111111111111111111
```

Response:
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

### 3. Get Recent Transactions

```bash
# Get 10 most recent transactions
curl "http://localhost:8080/api/v1/wallet/11111111111111111111111111111111/transactions?limit=10"

# Get 50 most recent transactions
curl "http://localhost:8080/api/v1/wallet/11111111111111111111111111111111/transactions?limit=50"
```

### 4. Get Risk Score

```bash
curl http://localhost:8080/api/v1/wallet/11111111111111111111111111111111/risk
```

Response:
```json
{
  "wallet": "11111111111111111111111111111111",
  "risk_score": 0.45,
  "transaction_count": 87,
  "risk_level": "medium"
}
```

### 5. Analyze with POST (Advanced)

```bash
curl -X POST http://localhost:8080/api/v1/analyze/wallet \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "11111111111111111111111111111111",
    "include_transactions": true,
    "depth": 3
  }'
```

---

## 📈 Performance Tips

1. **Use appropriate limits**
   ```bash
   # Good - returns 10 transactions
   curl "http://localhost:8080/api/v1/wallet/{address}/transactions?limit=10"
   
   # Avoid - no limit specified, may be slower
   curl "http://localhost:8080/api/v1/wallet/{address}/transactions"
   ```

2. **Batch requests efficiently**
   - Make multiple requests in parallel
   - Implement client-side caching
   - Use response caching headers

3. **Monitor latency**
   ```bash
   # Check response time
   curl -w "@curl-format.txt" http://localhost:8080/api/v1/analyze/wallet/{address}
   ```

---

## 🔍 Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug ./target/release/onchain_beast
```

### Check Server Status

```bash
# Via health endpoint
curl http://localhost:8080/health

# Via status endpoint
curl http://localhost:8080/status

# Via cluster health
curl http://localhost:8080/api/v1/cluster/health
```

### Test RPC Connection

```bash
# This will fail if RPC is unreachable
curl http://localhost:8080/api/v1/cluster/info
```

---

## 🛠️ Testing with Python

```python
import requests
import json

BASE_URL = "http://localhost:8080"

# Test 1: Health check
print("Health Check:")
response = requests.get(f"{BASE_URL}/health")
print(json.dumps(response.json(), indent=2))

# Test 2: Analyze wallet
wallet = "11111111111111111111111111111111"
print(f"\nAnalyzing wallet {wallet}:")
response = requests.get(f"{BASE_URL}/api/v1/analyze/wallet/{wallet}")
print(json.dumps(response.json(), indent=2))

# Test 3: Get transactions
print(f"\nGetting transactions for {wallet}:")
response = requests.get(
    f"{BASE_URL}/api/v1/wallet/{wallet}/transactions",
    params={"limit": 5}
)
print(json.dumps(response.json(), indent=2))

# Test 4: Get risk score
print(f"\nGetting risk score for {wallet}:")
response = requests.get(f"{BASE_URL}/api/v1/wallet/{wallet}/risk")
print(json.dumps(response.json(), indent=2))

# Test 5: Network metrics
print("\nNetwork Metrics:")
response = requests.get(f"{BASE_URL}/api/v1/network/metrics")
print(json.dumps(response.json(), indent=2))
```

---

## 🛠️ Testing with curl

```bash
#!/bin/bash

BASE_URL="http://localhost:8080"
WALLET="11111111111111111111111111111111"

echo "=== Health Check ==="
curl $BASE_URL/health | jq

echo -e "\n=== System Status ==="
curl $BASE_URL/status | jq

echo -e "\n=== Wallet Analysis ==="
curl $BASE_URL/api/v1/analyze/wallet/$WALLET | jq

echo -e "\n=== Risk Score ==="
curl $BASE_URL/api/v1/wallet/$WALLET/risk | jq

echo -e "\n=== Transactions (limit=10) ==="
curl "$BASE_URL/api/v1/wallet/$WALLET/transactions?limit=10" | jq

echo -e "\n=== Network Metrics ==="
curl $BASE_URL/api/v1/network/metrics | jq

echo -e "\n=== Cluster Info ==="
curl $BASE_URL/api/v1/cluster/info | jq

echo -e "\n=== Account Balance ==="
curl $BASE_URL/api/v1/account/$WALLET/balance | jq
```

Save as `test_api.sh` and run:
```bash
chmod +x test_api.sh
./test_api.sh
```

---

## 🚨 Common Issues & Solutions

### Issue: Connection Refused
```
curl: (7) Failed to connect to localhost port 8080
```

**Solution**: Make sure the server is running
```bash
./target/release/onchain_beast
```

### Issue: RPC Connection Error
```json
{
  "status": "error",
  "error": "RPC connection failed"
}
```

**Solution**: Check RPC endpoint
```bash
RPC_ENDPOINT=https://api.mainnet-beta.solana.com ./target/release/onchain_beast
```

### Issue: Invalid Address Format
```json
{
  "error": "Wallet not found: Invalid address"
}
```

**Solution**: Use valid Solana address format (base58, 44 characters)

### Issue: Rate Limited
```json
{
  "error": "Rate limit exceeded"
}
```

**Solution**: Implement request throttling or contact server admin

---

## 📚 API Response Status Codes

| Code | Status | Description |
|------|--------|-------------|
| 200 | OK | Successful request |
| 400 | Bad Request | Invalid parameters |
| 404 | Not Found | Resource not found |
| 500 | Server Error | Internal server error |
| 503 | Unavailable | RPC/Service unavailable |

---

## 🔗 Integration Examples

### With JavaScript Fetch API

```javascript
async function analyzeWallet(address) {
  try {
    const response = await fetch(
      `http://localhost:8080/api/v1/analyze/wallet/${address}`
    );
    const data = await response.json();
    console.log(data);
    return data;
  } catch (error) {
    console.error('API Error:', error);
  }
}

// Usage
analyzeWallet('11111111111111111111111111111111');
```

### With Axios

```javascript
const axios = require('axios');

const api = axios.create({
  baseURL: 'http://localhost:8080',
  timeout: 10000
});

// Analyze wallet
api.get('/api/v1/analyze/wallet/11111111111111111111111111111111')
  .then(response => console.log(response.data))
  .catch(error => console.error(error));
```

---

## 📖 Full API Documentation

See [REST_API_DOCUMENTATION.md](REST_API_DOCUMENTATION.md) for comprehensive endpoint documentation.

---

## 🎯 Next Steps

1. **Start the server** with `./target/release/onchain_beast`
2. **Test basic endpoints** with the curl examples above
3. **Integrate with your application** using the provided examples
4. **Monitor performance** and adjust rate limits as needed
5. **Explore advanced features** like graph analysis and pattern detection

---

## 📞 Support

- Check [REST_API_DOCUMENTATION.md](REST_API_DOCUMENTATION.md) for detailed endpoint docs
- Review [GRAPH_INTEGRATION_GUIDE.md](GRAPH_INTEGRATION_GUIDE.md) for graph analysis features
- See example curl commands in this guide

---

**Happy analyzing! 🚀**

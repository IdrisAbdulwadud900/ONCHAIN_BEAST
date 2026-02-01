# DEX Swap Extraction System - Complete Documentation

## Overview

A fully operational DEX swap detection and analysis system for Solana blockchain, automatically extracting swap events from all processed transactions and providing comprehensive query APIs.

## Architecture

### 1. **DEX Decoder Module** (`src/dex/`)

```
src/dex/
├── mod.rs          # Main router with dual extraction strategies
├── types.rs        # Data structures and constants
└── raydium.rs      # Raydium V4 AMM decoder
```

**Key Components:**

- **SwapEvent**: Unified swap data structure
- **DexPrograms**: Constants for known DEX program IDs
- **QuoteTokens**: SOL/USDC/USDT helpers for swap direction detection
- **RaydiumDecoder**: Instruction-level parsing for Raydium swaps
- **DexDecoder**: Main router with fallback inference

**Extraction Strategies:**

1. **Instruction Parsing** (Primary)
   - Decodes instruction data directly
   - Reads discriminator, amounts, accounts
   - Most accurate for known DEXes

2. **Transfer Pattern Inference** (Fallback)
   - Analyzes token transfer patterns
   - Matches outbound + inbound transfers
   - Works for unknown DEXes

### 2. **Database Schema**

**swap_events Table:**
```sql
CREATE TABLE swap_events (
    signature VARCHAR(88) NOT NULL,
    event_index INTEGER NOT NULL,
    slot BIGINT NOT NULL,
    block_time BIGINT NOT NULL,
    wallet VARCHAR(44) NOT NULL,
    dex_program VARCHAR(44) NOT NULL,
    dex_name VARCHAR(100) NOT NULL,
    token_in VARCHAR(44) NOT NULL,
    amount_in BIGINT NOT NULL,
    token_out VARCHAR(44) NOT NULL,
    amount_out BIGINT NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    min_amount_out BIGINT,
    pool_address VARCHAR(44),
    PRIMARY KEY (signature, event_index)
);
```

**Indexes (6 total):**
- `idx_swap_events_wallet` - Wallet swap history queries
- `idx_swap_events_token_in` - Token buy timeline
- `idx_swap_events_token_out` - Token sell timeline
- `idx_swap_events_block_time` - Time-range queries
- `idx_swap_events_dex` - DEX-specific analytics
- `idx_swap_events_signature` - Transaction lookups

### 3. **Transaction Pipeline Integration**

**Flow:**
```
Transaction Received
      ↓
TransferAnalytics.analyze_transaction()
      ↓
DexDecoder.extract_swaps()
      ├── Try instruction parsing (Raydium, Orca, etc.)
      └── Fallback to transfer inference
      ↓
store_swap_event() for each swap
      ↓
Swap stored in database
```

**Code Location:**
- Integration point: `src/modules/transfer_analytics.rs:52-62`
- Automatic extraction on every transaction
- No manual triggering required

### 4. **Query APIs**

**Base URL:** `http://localhost:8080/api/v1/swaps`

#### GET /api/v1/swaps/wallet/{address}

Get swap history for a wallet.

**Parameters:**
- `address` (path) - Wallet address
- `limit` (query, optional) - Max results (default: 100, max: 1000)
- `offset` (query, optional) - Pagination offset (default: 0)

**Response:**
```json
{
  "success": true,
  "wallet": "...",
  "count": 10,
  "data": [
    {
      "signature": "...",
      "event_index": 0,
      "slot": 123456789,
      "block_time": 1706745600,
      "wallet": "...",
      "dex_program": "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8",
      "dex_name": "Raydium V4",
      "token_in": "So11111111111111111111111111111111111111112",
      "amount_in": 1000000000,
      "token_out": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      "amount_out": 100000000,
      "price": 0.1
    }
  ]
}
```

**Use Cases:**
- Wallet trading history
- PnL calculation input
- Trading pattern analysis

#### GET /api/v1/swaps/token/{mint}

Get swap timeline for a specific token.

**Parameters:**
- `mint` (path) - Token mint address
- `limit` (query, optional) - Max results
- `offset` (query, optional) - Pagination offset

**Response:** Same format as wallet endpoint

**Use Cases:**
- Token trading volume analysis
- Liquidity event tracking
- Price discovery timeline

#### GET /api/v1/swaps/stats/{wallet}

Get aggregated swap statistics for a wallet.

**Parameters:**
- `wallet` (path) - Wallet address

**Response:**
```json
{
  "success": true,
  "wallet": "...",
  "data": {
    "total_swaps": 156,
    "unique_tokens": 42,
    "dex_breakdown": [
      {
        "dex_name": "Raydium V4",
        "swap_count": 89
      },
      {
        "dex_name": "Orca Whirlpool",
        "swap_count": 45
      }
    ],
    "first_swap": 1704067200,
    "last_swap": 1706745600
  }
}
```

**Use Cases:**
- Trader profiling
- DEX preference analysis
- Activity timeline
- Trading frequency metrics

## Supported DEXes

### Currently Implemented:

1. **Raydium V4 AMM**
   - Program: `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`
   - Instruction parsing: ✅
   - Swap directions: Both buy and sell
   - Slippage detection: ✅

### Ready to Implement:

2. **Raydium Stable**
   - Program: `5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h`

3. **Orca Whirlpool**
   - Program: `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`

4. **Orca V1/V2**
   - Programs: Multiple

5. **Jupiter Aggregator**
   - Programs: V4, V6
   - Note: Requires inner instruction parsing

6. **Meteora**
   - Dynamic pools

7. **Phoenix**
   - Order book DEX

### Fallback Coverage:

- **All Unknown DEXes**: Transfer pattern inference
- Detects any wallet with matching outbound + inbound transfers
- Less accurate but provides coverage

## Testing

### Automated Test Suite

Run comprehensive tests:
```bash
./test_swap_extraction.sh
```

**Tests:**
1. ✅ Database schema validation
2. ✅ Index verification (6 indexes)
3. ✅ API endpoint functionality
4. ✅ Data insertion and retrieval
5. ✅ Statistics calculation

### Manual Testing

**1. Check swap count:**
```bash
psql postgresql://$(whoami)@localhost/onchain_beast_personal \
  -c "SELECT COUNT(*) FROM swap_events;"
```

**2. View recent swaps:**
```bash
psql postgresql://$(whoami)@localhost/onchain_beast_personal \
  -c "SELECT dex_name, wallet, token_in, token_out, amount_in, amount_out 
      FROM swap_events 
      ORDER BY block_time DESC 
      LIMIT 10;"
```

**3. Test API:**
```bash
# Wallet swaps
curl "http://localhost:8080/api/v1/swaps/wallet/YOUR_WALLET?limit=10" | jq '.'

# Token swaps
curl "http://localhost:8080/api/v1/swaps/token/TOKEN_MINT?limit=10" | jq '.'

# Statistics
curl "http://localhost:8080/api/v1/swaps/stats/YOUR_WALLET" | jq '.'
```

## Data Ingestion

### Automatic Extraction

Swaps are **automatically extracted** from all transactions processed through:
- Event ingestion endpoints
- Side-wallet analysis
- Transfer analytics
- Any transaction parsing

No manual configuration required!

### Manual Ingestion

Ingest transactions for a specific wallet:
```bash
curl -X POST 'http://localhost:8080/transfer/ingest/wallet' \
  -H 'Content-Type: application/json' \
  -d '{
    "wallet": "WALLET_ADDRESS",
    "limit": 100
  }'
```

**Note:** Transactions must be recent (default: last 30 days)

### Batch Ingestion

Process multiple wallets:
```bash
curl -X POST 'http://localhost:8080/transfer/ingest/batch' \
  -H 'Content-Type: application/json' \
  -d '{
    "wallets": ["WALLET1", "WALLET2", "WALLET3"],
    "limit": 50
  }'
```

### Backfill from Relationships

Auto-discover and ingest from wallet graph:
```bash
curl -X POST 'http://localhost:8080/transfer/ingest/backfill' \
  -H 'Content-Type: application/json' \
  -d '{
    "wallet": "SEED_WALLET",
    "max_depth": 2
  }'
```

## Performance

### Query Optimization

All queries are indexed for performance:
- Wallet queries: O(log n) via `idx_swap_events_wallet`
- Token queries: O(log n) via token_in/token_out indexes
- Time-range: O(log n) via `idx_swap_events_block_time`

### Expected Throughput

- **Swap extraction**: ~10-50ms per transaction
- **API queries**: <20ms for typical requests
- **Batch ingestion**: 5-10 transactions/second

### Scaling Considerations

- Database can handle millions of swaps
- Indexes scale logarithmically
- Consider partitioning at 10M+ swaps

## Integration Points

### For PnL Calculation

```rust
// Get wallet's swap history
let swaps = db.get_wallet_swaps(wallet, limit, offset).await?;

// For each swap, calculate:
// - Entry/exit prices
// - Position size changes
// - Realized/unrealized PnL

// Example: SOL → USDC swap
if swap.token_in == SOL_MINT {
    // Selling SOL, buying USDC
    let sol_price = swap.amount_out as f64 / swap.amount_in as f64;
}
```

### For Trading Pattern Analysis

```rust
// Get DEX preferences
let stats = db.get_wallet_swap_stats(wallet).await?;
let preferred_dex = stats.dex_breakdown.first();

// Analyze token diversity
let unique_tokens = stats.unique_tokens;
```

### For Timeline Queries

```sql
-- All SOL buys in last 7 days
SELECT * FROM swap_events 
WHERE token_out = 'So11111111111111111111111111111111111111112'
  AND block_time > EXTRACT(EPOCH FROM NOW() - INTERVAL '7 days')
ORDER BY block_time DESC;
```

## Troubleshooting

### No swaps detected

**Possible causes:**
1. Wallet has no recent transactions
2. Transactions don't contain swaps
3. DEX not yet supported (check `dex_name = 'Inferred from transfers'`)

**Solutions:**
- Check wallet activity on Solscan
- Verify transaction contains DEX interactions
- Add decoder for specific DEX

### Wrong swap amounts

**Likely cause:** Transfer inference vs instruction parsing

**Solution:**
- Implement instruction decoder for specific DEX
- Check `min_amount_out` field for slippage

### API returns 404

**Causes:**
- Wrong endpoint path (use `/api/v1/swaps/...`)
- Server not running
- Route not registered

**Solution:**
```bash
# Check server logs
tail -f /tmp/beast.log

# Verify health
curl http://localhost:8080/health
```

## Roadmap

### Phase 4.2: Price Oracle Integration
- [ ] Connect to Jupiter Price API
- [ ] Create `token_prices` table
- [ ] Cache historical prices
- [ ] Enrich swaps with USD values

### Phase 4.3: PnL Engine
- [ ] Entry/exit detection
- [ ] Position tracking
- [ ] Realized PnL calculation
- [ ] Unrealized PnL calculation
- [ ] Claim verification endpoint

### Phase 4.4: Additional DEXes
- [ ] Orca Whirlpool decoder
- [ ] Jupiter aggregator support
- [ ] Meteora pools
- [ ] Phoenix order book

### Phase 5: Advanced Analytics
- [ ] Trading patterns
- [ ] Win rate calculation
- [ ] Profit factor
- [ ] Sharpe ratio
- [ ] Drawdown analysis

## Examples

### Complete Trading History

```bash
# Get all swaps for a wallet
curl "http://localhost:8080/api/v1/swaps/wallet/WALLET?limit=1000" \
  | jq '.data[] | {time: .block_time, dex: .dex_name, 
                   from: .token_in, to: .token_out, price: .price}'
```

### Token Trading Volume

```bash
# All trades involving a token
curl "http://localhost:8080/api/v1/swaps/token/TOKEN_MINT?limit=500" \
  | jq '[.data[] | {wallet: .wallet, amount_in: .amount_in, 
                     amount_out: .amount_out}]'
```

### Trader Profile

```bash
# Get statistics
curl "http://localhost:8080/api/v1/swaps/stats/WALLET" | jq '.'
```

## Conclusion

The DEX swap extraction system is **fully operational** and ready for:
- ✅ Live transaction processing
- ✅ Historical data queries  
- ✅ PnL calculation foundation
- ✅ Trading analytics

Next step: Integrate price oracle for USD-denominated PnL tracking!

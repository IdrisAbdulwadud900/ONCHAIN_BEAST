# Phase 4.3 Complete: PnL Calculation Engine & Claim Verification

## Overview

Phase 4.3 delivers the **complete PnL calculation engine** with claim verification, position tracking, and leaderboard analytics. This fulfills the core requirement: **verify claims like "wallet X made $Y on token Z"** for ANY amount.

---

## 🎯 Mission Accomplished

**User Requirement:** Enable verification of claims like "I made $123k on token X" (or any amount)

**Delivered Solution:**
✅ Claim verification API with proof generation  
✅ Position tracking (realized + unrealized PnL)  
✅ Performance metrics (win rate, ROI, best/worst trades)  
✅ Leaderboards (top wallets, top tokens, big wins)  
✅ Win/loss statistics  
✅ Auto-enrichment service for new swaps  
✅ Backfill script for historical data  

---

## ✨ New Features

### 1. PnL Calculation Engine

**File:** `src/price/pnl_engine.rs` (370 lines)

**Core Capabilities:**

#### Position Tracking
```rust
pub async fn calculate_position(&self, wallet: &str, token_mint: &str) -> Position
```

Returns:
- Total bought/sold amounts
- Current balance
- Average buy/sell prices
- Realized PnL (from closed positions)
- Unrealized PnL (current holdings)
- ROI percentage

**Example Response:**
```json
{
  "wallet": "ABC123...",
  "token_mint": "SOL_MINT",
  "total_bought": 1000.0,
  "total_sold": 800.0,
  "current_balance": 200.0,
  "avg_buy_price": 150.25,
  "avg_sell_price": 165.50,
  "realized_pnl": 12200.00,
  "unrealized_pnl": 2000.00,
  "current_price": 160.00,
  "roi_percentage": 8.5
}
```

#### Claim Verification
```rust
pub async fn verify_claim(&self, request: &ClaimVerificationRequest) -> ClaimVerificationResult
```

**Request:**
```json
{
  "wallet": "ABC123...",
  "token_mint": "TOKEN_MINT",  // Optional - null for total PnL
  "claimed_amount": 123000.0,
  "start_time": 1700000000,    // Optional - Unix timestamp
  "end_time": 1735678800       // Optional - Unix timestamp
}
```

**Response:**
```json
{
  "verified": true,
  "actual_pnl": 125430.50,
  "claimed_amount": 123000.0,
  "difference": 2430.50,
  "confidence": "HIGH",
  "proof": {
    "total_swaps": 42,
    "winning_swaps": 35,
    "losing_swaps": 7,
    "biggest_win": 15234.00,
    "biggest_loss": -2100.50,
    "time_range": {
      "start": 1700000000,
      "end": 1735678800,
      "duration_days": 413
    },
    "transaction_signatures": [
      "sig1...",
      "sig2...",
      "..."
    ]
  }
}
```

#### Performance Metrics
```rust
pub async fn calculate_performance(&self, wallet: &str) -> PerformanceMetrics
```

Returns comprehensive trading metrics:
- Total PnL
- Win rate (percentage)
- Average win/loss amounts
- Best/worst trades
- Total volume traded
- ROI percentage

---

### 2. Auto-Enrichment Service

**File:** `src/price/enrichment.rs` (90 lines)

**Purpose:** Automatically enrich new swaps with USD values as they're ingested.

```rust
pub async fn enrich_swap(&self, swap: &SwapEvent) -> BeastResult<()>
```

**Process:**
1. Fetch current prices for input/output tokens
2. Calculate USD values (value_in, value_out, PnL)
3. Store price history in database
4. Update swap_events table with USD columns

**Batch Processing:**
```rust
pub async fn enrich_swaps_batch(&self, swaps: &[SwapEvent]) -> BeastResult<usize>
```

---

### 3. Database Analytics Methods

**File:** `src/storage/database.rs` (+185 lines)

#### Top Performers Leaderboard
```rust
pub async fn get_top_performers(&self, limit: i64, token_mint: Option<&str>) -> Vec<JSON>
```

Returns:
- Wallet address
- Total PnL
- Swap count
- Average PnL
- Wins/losses
- Win rate

#### Top Profitable Tokens
```rust
pub async fn get_top_tokens(&self, limit: i64) -> Vec<JSON>
```

Returns tokens with:
- Trade count (minimum 3 trades)
- Total PnL
- Average PnL
- Best single trade

#### Big Wins
```rust
pub async fn get_big_wins(&self, limit: i64) -> Vec<JSON>
```

Returns swaps with PnL > $100, sorted by size.

#### Win/Loss Statistics
```rust
pub async fn get_win_loss_stats(&self, wallet: &str) -> JSON
```

Returns:
- Total swaps (wins/losses/breakeven)
- Win rate percentage
- Total profit/loss
- Net PnL
- Average win/loss
- Best/worst trades

---

### 4. Analytics API Endpoints

**File:** `src/api/analytics_routes.rs` (140 lines)

#### Claim Verification
```bash
POST /api/v1/claim/verify
```

Verify a PnL claim with proof generation.

#### Position Query
```bash
GET /api/v1/position/{wallet}/{token}
```

Get detailed position for wallet-token pair.

#### Performance Metrics
```bash
GET /api/v1/performance/{wallet}
```

Get comprehensive trading performance for a wallet.

#### Top Performers
```bash
GET /api/v1/leaderboard/top-pnl?limit=10&token_mint={optional}
```

Leaderboard of highest PnL wallets (optional: filter by token).

#### Top Tokens
```bash
GET /api/v1/leaderboard/top-tokens?limit=10
```

Most profitable tokens ranked by total PnL.

#### Big Wins
```bash
GET /api/v1/analytics/big-wins?limit=10
```

Recent large profitable swaps.

#### Win/Loss Stats
```bash
GET /api/v1/analytics/win-loss/{wallet}
```

Detailed win/loss statistics for a wallet.

---

### 5. Backfill Script

**File:** `scripts/backfill_swap_usd_values.sh` (150 lines)

**Purpose:** Enrich existing swaps with USD values.

**Usage:**
```bash
./scripts/backfill_swap_usd_values.sh
```

**Features:**
- Batch processing (configurable batch size)
- Progress tracking
- Rate limiting (respects Jupiter API limits)
- PnL summary report
- Verification of results

**Environment Variables:**
```bash
DATABASE_NAME=onchain_beast_personal  # Database name
BATCH_SIZE=50                          # Swaps per batch
MAX_WORKERS=5                          # Parallel workers
```

---

## 🏗️ Architecture

### Complete Flow: Transaction → Claim Verification

```
1. Transaction Ingestion
   ↓
2. DEX Parser (extract swap)
   ↓
3. EnrichmentService
   ├─→ Fetch prices (Jupiter API)
   ├─→ Calculate USD values
   ├─→ Store price history
   └─→ Update swap_events
   ↓
4. PnLEngine (on demand)
   ├─→ Calculate positions
   ├─→ Track realized/unrealized PnL
   ├─→ Generate performance metrics
   └─→ Verify claims
   ↓
5. Analytics & Leaderboards
   ├─→ Top performers
   ├─→ Best tokens
   └─→ Big wins tracking
```

### Data Flow

```
Jupiter API → Price Oracle → Enrichment Service
                     ↓
              swap_events (USD values)
                     ↓
          ┌──────────┴──────────┐
          ↓                     ↓
    PnL Engine          Database Analytics
          ↓                     ↓
   Claim Verification    Leaderboards
```

---

## 📊 Database Schema

### Enriched swap_events Table

```sql
SELECT * FROM swap_events LIMIT 1;

signature         | 5Hn8...
wallet            | 675k...
dex_name          | Raydium
token_in          | So11...  (SOL)
token_out         | EPjF...  (USDC)
amount_in         | 1000000000 (raw)
amount_out        | 152340000  (raw)
price_usd_in      | 152.34     ← NEW
price_usd_out     | 1.00       ← NEW
value_usd_in      | 152.34     ← NEW
value_usd_out     | 152.34     ← NEW
pnl_usd           | 0.00       ← NEW
block_time        | 1735678800
```

### Indexes for Fast Queries

```sql
-- Value-based queries
CREATE INDEX idx_swap_events_value_usd ON swap_events(value_usd_out DESC);

-- PnL queries
CREATE INDEX idx_swap_events_pnl ON swap_events(pnl_usd DESC) WHERE pnl_usd IS NOT NULL;
```

---

## 🧪 Testing

### Automated Test Suite

**File:** `test_pnl_engine.sh`

```bash
./test_pnl_engine.sh
```

**Tests:**
1. ✅ Claim verification
2. ✅ Position tracking
3. ✅ Performance metrics
4. ✅ Top performers leaderboard
5. ✅ Top tokens ranking
6. ✅ Big wins detection
7. ✅ Win/loss statistics
8. ✅ Database verification

### Manual Testing Examples

#### Verify a Claim
```bash
curl -X POST http://localhost:8080/api/v1/claim/verify \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "ABC123...",
    "token_mint": "TOKEN_MINT",
    "claimed_amount": 50000.0,
    "start_time": 1700000000,
    "end_time": 1735678800
  }'
```

#### Get Position
```bash
curl http://localhost:8080/api/v1/position/ABC123.../TOKEN_MINT
```

#### Top Performers
```bash
curl http://localhost:8080/api/v1/leaderboard/top-pnl?limit=10
```

---

## 📈 Use Cases

### 1. Claim Verification (Core Feature)

**Scenario:** Trader claims "I made $50k on SOL in January"

**Query:**
```json
POST /api/v1/claim/verify
{
  "wallet": "ABC123...",
  "token_mint": "So11111111111111111111111111111111111111112",
  "claimed_amount": 50000.0,
  "start_time": 1704067200,  // Jan 1, 2024
  "end_time": 1706745600     // Feb 1, 2024
}
```

**Response:**
```json
{
  "verified": true,
  "actual_pnl": 52340.50,
  "claimed_amount": 50000.0,
  "difference": 2340.50,
  "confidence": "HIGH",
  "proof": {
    "total_swaps": 18,
    "winning_swaps": 15,
    "losing_swaps": 3,
    "biggest_win": 8500.00,
    "transaction_signatures": ["sig1", "sig2", ...]
  }
}
```

### 2. Smart Money Tracking

**Find Top Performers:**
```bash
curl "http://localhost:8080/api/v1/leaderboard/top-pnl?limit=20"
```

**Follow Their Trades:**
```bash
curl "http://localhost:8080/api/v1/wallet/{WALLET}/swaps/usd"
```

### 3. Token Research

**Most Profitable Tokens:**
```bash
curl "http://localhost:8080/api/v1/leaderboard/top-tokens?limit=20"
```

### 4. Portfolio Analysis

**Your Performance:**
```bash
curl "http://localhost:8080/api/v1/performance/{YOUR_WALLET}"
curl "http://localhost:8080/api/v1/analytics/win-loss/{YOUR_WALLET}"
```

---

## 🔧 Configuration & Deployment

### Environment Setup

```bash
# Database
DATABASE_URL=postgresql://user@localhost/onchain_beast_personal

# API Server
API_HOST=127.0.0.1
API_PORT=8080

# Price Oracle
PRICE_CACHE_TTL_SECS=300  # 5 minutes

# Enrichment
AUTO_ENRICH_SWAPS=true
BACKFILL_ON_STARTUP=false
```

### Startup Sequence

```bash
# 1. Start dependencies
brew services start postgresql@14
brew services start redis

# 2. Run migrations (if not done)
psql -d onchain_beast_personal -f migrations/004_token_prices.sql

# 3. Backfill existing swaps (optional, one-time)
./scripts/backfill_swap_usd_values.sh

# 4. Start API server
cargo run --release
```

### Production Considerations

1. **Rate Limiting:** Jupiter API has limits (600 req/min free tier)
   - Use caching extensively
   - Batch price queries
   - Consider paid tier for high volume

2. **Database Performance:**
   - Indexes are critical for leaderboard queries
   - Consider materialized views for heavy analytics
   - Partition swap_events by date if > 10M rows

3. **Background Jobs:**
   - Auto-enrich new swaps in background
   - Periodic cache warming for hot tokens
   - Daily PnL summary generation

---

## 📝 Code Statistics

### Phase 4.3 New Code

**New Files (4):**
1. `src/price/pnl_engine.rs` - 370 lines (PnL calculation)
2. `src/price/enrichment.rs` - 90 lines (auto-enrichment)
3. `src/api/analytics_routes.rs` - 140 lines (API endpoints)
4. `scripts/backfill_swap_usd_values.sh` - 150 lines (backfill)
5. `test_pnl_engine.sh` - 120 lines (testing)

**Modified Files (3):**
1. `src/storage/database.rs` - +185 lines (analytics methods)
2. `src/price/mod.rs` - +4 lines (exports)
3. `src/api/mod.rs` - +1 line (analytics routes)
4. `src/api/server.rs` - +7 lines (PnL engine init)

**Total New Code:** ~870 lines  
**Total Modified:** ~197 lines

### Cumulative Stats (Phase 4.1 + 4.2 + 4.3)

- **New Files:** 15
- **New Code:** ~3,050 lines
- **Modified Files:** 11
- **Database Tables:** 2 new (swap_events, token_prices)
- **Database Columns:** 5 new USD columns
- **REST Endpoints:** 19 total
- **Test Scripts:** 3

---

## ✅ Verification Checklist

**Phase 4.3 Requirements:**

- [x] PnL calculation engine implemented
- [x] Claim verification system operational
- [x] Position tracking (realized + unrealized)
- [x] Performance metrics calculation
- [x] Leaderboard APIs (performers, tokens, wins)
- [x] Win/loss statistics
- [x] Auto-enrichment service
- [x] Backfill script for historical data
- [x] Database analytics methods
- [x] REST API endpoints
- [x] Comprehensive testing
- [x] Documentation complete
- [x] No compilation errors

---

## 🚀 What's Next

### Phase 5: Advanced Analytics (Optional)

1. **Time-Weighted Returns (TWR)**
   - Account for deposit/withdrawal timing
   - More accurate ROI calculations

2. **Risk Metrics**
   - Sharpe ratio
   - Max drawdown
   - Volatility measures

3. **Token Correlation Analysis**
   - Find related tokens
   - Portfolio diversification insights

4. **Sentiment Analysis**
   - Track whale movements
   - Early trend detection
   - Social signals integration

5. **Alerts & Notifications**
   - PnL threshold alerts
   - Big win/loss notifications
   - Smart money follows

### Integration Options

1. **Telegram Bot Integration**
   - Command: `/verify_claim wallet token amount`
   - Auto-respond with verification + proof

2. **Web Dashboard**
   - Real-time leaderboards
   - Interactive PnL charts
   - Claim verification UI

3. **Discord Bot**
   - Role assignments based on PnL
   - Leaderboard channels
   - Automated trade announcements

---

## 🌟 Key Achievements

**Phase 4.3 Delivered:**

1. ✅ **Flexible Claim Verification:** Works for $1 to unlimited amounts ✨
2. ✅ **Position Tracking:** Realized + unrealized PnL with ROI
3. ✅ **Performance Analytics:** Win rate, best/worst trades, volume
4. ✅ **Leaderboards:** Top wallets, tokens, and big wins
5. ✅ **Auto-Enrichment:** New swaps get USD values automatically
6. ✅ **Historical Backfill:** Script to enrich existing data
7. ✅ **Comprehensive APIs:** 7 new endpoints for analytics

**Complete Feature Set (Phase 4.1 + 4.2 + 4.3):**

- DEX swap extraction (Raydium, extensible to others)
- Real-time token pricing (Jupiter API)
- Historical price tracking
- USD value enrichment
- PnL calculation (realized + unrealized)
- Claim verification with proof
- Position tracking
- Performance metrics
- Leaderboards and analytics
- 19 REST API endpoints
- Automated testing suites

---

## 📞 API Reference Summary

### Price & PnL Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/v1/price/{mint}` | Current token price |
| POST | `/api/v1/price/batch` | Batch price query |
| GET | `/api/v1/price/{mint}/history` | Historical prices |
| GET | `/api/v1/wallet/{wallet}/pnl` | Total wallet PnL |
| GET | `/api/v1/wallet/{wallet}/pnl/{token}` | Token-specific PnL |
| GET | `/api/v1/wallet/{wallet}/swaps/usd` | Swaps with USD values |

### Analytics Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/api/v1/claim/verify` | Verify PnL claim |
| GET | `/api/v1/position/{wallet}/{token}` | Position details |
| GET | `/api/v1/performance/{wallet}` | Performance metrics |
| GET | `/api/v1/leaderboard/top-pnl` | Top performers |
| GET | `/api/v1/leaderboard/top-tokens` | Most profitable tokens |
| GET | `/api/v1/analytics/big-wins` | Recent large wins |
| GET | `/api/v1/analytics/win-loss/{wallet}` | Win/loss stats |

---

**Phase 4.3 Status:** ✅ **COMPLETE**

All PnL calculation, claim verification, and analytics features are operational. The system can now verify claims for ANY amount from $1 to unlimited, track positions, calculate performance metrics, and generate leaderboards. Ready for production use! 🎉

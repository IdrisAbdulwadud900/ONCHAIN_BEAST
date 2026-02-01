# OnChain Beast - Complete Project Roadmap & Status

**Project Goal:** Build a sophisticated Solana wallet intelligence system for cabal detection, PnL verification, and alpha discovery.

**Date:** February 1, 2026  
**Current Phase:** Evidence Framework Complete → Advanced Features

---

## ✅ PHASE 1: FOUNDATION (100% Complete)

### Core Infrastructure
- [x] Rust/Actix-web API server
- [x] PostgreSQL database with proper schema
- [x] Redis caching layer
- [x] Solana RPC integration
- [x] Transaction parser (SOL + SPL tokens)
- [x] Error handling and logging
- [x] Rate limiting and metrics

### Database Schema
- [x] `transactions` table (enriched transaction data)
- [x] `wallet_relationships` table (aggregate transfers)
- [x] `transfer_events` table (event-level data)
- [x] `wallet_analyses` table (risk scores, patterns)
- [x] Proper indexes for query optimization

---

## ✅ PHASE 2: WALLET CLUSTERING (100% Complete)

### Evidence-Based Heuristic Model
All 5 signals operational with transparent scoring:

#### Signal 1: Graph Connectivity (30% weight) ✅
- **Status:** Operational
- **Implementation:** BFS traversal over `wallet_relationships`
- **API:** `/api/v1/wallet/{address}/side-wallets`
- **Evidence:** Direct transaction links with counts/amounts

#### Signal 2: Shared Inbound Funders (25% weight) ✅
- **Status:** Operational
- **Implementation:** `get_shared_inbound_senders()` query
- **Data Source:** `transfer_events` table
- **Evidence:** Common funding source wallets

#### Signal 3: Shared Counterparties (20% weight) ✅
- **Status:** Operational
- **Implementation:** `get_top_counterparties()` query
- **Data Source:** `transfer_events` table
- **Evidence:** Common destination wallets (DEXes, exchanges)

#### Signal 4: Behavioral Correlation (15% weight) ✅
- **Status:** Operational
- **Implementation:** `get_behavioral_profile()` + similarity computation
- **Metrics:** TX amounts (40%), frequency (35%), time-of-day (25%)
- **Evidence:** Numerical similarity score

#### Signal 5: Temporal Alignment (10% weight) ✅
- **Status:** Operational
- **Implementation:** `get_temporal_overlap()` with time bucketing
- **Sub-signals:** Same-block transactions, synchronized activity windows
- **Evidence:** Overlap ratio + same-block count

### Event Ingestion Pipeline ✅
- [x] `EventIngestionWorker` for background processing
- [x] `POST /transfer/ingest/wallet` - single wallet ingestion
- [x] `POST /transfer/ingest/batch` - parallel batch ingestion
- [x] `POST /transfer/ingest/backfill` - auto-backfill from relationships
- [x] Configurable batch sizes, concurrency, age filters
- [x] Statistics tracking and error handling

**API Endpoints:**
- ✅ `/api/v1/wallet/{address}/side-wallets` - Find related wallets
- ✅ `/api/v1/wallet/{address}/cluster` - Full cluster analysis
- ✅ `/transfer/ingest/*` - Event ingestion endpoints

**Documentation:**
- ✅ `EVIDENCE_FRAMEWORK.md` - Full model documentation (436 lines)
- ✅ `EVIDENCE_FRAMEWORK_COMPLETE.md` - Completion certificate (260 lines)

---

## ⚠️ PHASE 3: EXCHANGE INTELLIGENCE (30% Complete)

### What We Have
- [x] Basic `ExchangeDetector` module structure
- [x] Known exchange wallet list (hardcoded)
- [x] `is_exchange_wallet()` method

### What's Missing
- [ ] **Public label database** - Scrape/maintain exchange deposit addresses
- [ ] **Exchange wallet clustering** - Group hot/cold/deposit wallets
- [ ] **Route tracking** - "Wallet → Exchange → ?" analysis
- [ ] **Timing analysis** - Speed to exchange after events
- [ ] **Frequency metrics** - How often each cluster uses exchanges
- [ ] **API endpoints** - `/api/v1/exchange/trace`, `/api/v1/exchange/clusters`

### Priority: HIGH
Exchange tracing is critical for:
- Detecting cash-out patterns
- Identifying laundering routes
- Understanding cluster liquidity strategies

---

## ❌ PHASE 4: PnL VERIFICATION (0% Complete)

### Requirements
User wants: `"verify claim: wallet X made $123k on token Y"` → computed PnL with evidence

### What We Need

#### 4.1 DEX Program Decoding
- [ ] **Raydium swap decoder** - Extract swap details (in/out tokens, amounts, price)
- [ ] **Orca swap decoder**
- [ ] **Jupiter aggregator decoder**
- [ ] **Generic AMM decoder** - Handle unknown DEXes
- [ ] Store decoded swaps in new table: `swap_events`

#### 4.2 Price Oracle Integration
- [ ] **Historical price API** - Jupiter/Birdeye/CoinGecko
- [ ] **Price caching** - Store snapshots in `token_prices` table
- [ ] **Fallback logic** - Handle missing price data
- [ ] **TWAP calculation** - For illiquid tokens

#### 4.3 PnL Computation Engine
- [ ] **Entry detection** - First buy timestamp, amount, cost basis
- [ ] **Exit detection** - Sells, transfers out
- [ ] **Unrealized PnL** - Current holdings × current price
- [ ] **Realized PnL** - Closed positions
- [ ] **Confidence scoring** - Based on data quality/assumptions

#### 4.4 API Endpoints
- [ ] `POST /api/v1/pnl/verify` - Verify specific claim
- [ ] `GET /api/v1/pnl/wallet/{address}` - Full wallet PnL
- [ ] `GET /api/v1/pnl/token/{mint}` - PnL by token
- [ ] `GET /api/v1/pnl/top-performers` - Leaderboard

### Database Schema
```sql
CREATE TABLE swap_events (
    signature TEXT,
    event_index INT,
    slot BIGINT,
    block_time BIGINT,
    wallet TEXT,
    dex_program TEXT,
    token_in TEXT,
    amount_in NUMERIC,
    token_out TEXT,
    amount_out NUMERIC,
    price_at_swap NUMERIC,
    PRIMARY KEY (signature, event_index)
);

CREATE TABLE token_prices (
    mint TEXT,
    timestamp BIGINT,
    price_usd NUMERIC,
    source TEXT,
    PRIMARY KEY (mint, timestamp)
);
```

### Priority: HIGH
PnL verification is a core feature for alpha/scam detection.

---

## ❌ PHASE 5: TIME-BASED QUERIES (20% Complete)

### What We Have
- [x] `transfer_events` table with `block_time`
- [x] Indexes on timestamps

### What's Missing

#### 5.1 Swap Indexing
- [ ] Decode all swaps from transaction instructions
- [ ] Index: `(token, wallet, block_time, action)` where action = 'buy' | 'sell'
- [ ] First acquisition timestamp per wallet per token

#### 5.2 Query APIs
- [ ] `GET /api/v1/timeline/buyers?token={mint}&from={t1}&to={t2}` - Who bought when
- [ ] `GET /api/v1/timeline/earliest?token={mint}&limit=100` - First buyers
- [ ] `GET /api/v1/timeline/profitable?token={mint}` - Winners sorted by PnL
- [ ] `GET /api/v1/timeline/volume?token={mint}&interval=1h` - Volume spikes

#### 5.3 Analytics
- [ ] Accumulation phase detection (wallets buying before pump)
- [ ] Distribution phase detection (wallets selling during pump)
- [ ] Coordinated buying patterns (multiple wallets buying same token same time)

### Priority: MEDIUM
Important for alpha discovery but depends on swap decoding (Phase 4).

---

## ❌ PHASE 6: PATTERN MINING (0% Complete)

### Goal
Identify what strategies make money, not just what wallets do.

### 6.1 Feature Extraction
Extract these features per wallet/cluster:

**Trading Behavior:**
- [ ] Average hold time (minutes/hours/days)
- [ ] Position sizing (% of portfolio, SOL/token ratio)
- [ ] Entry timing (pre-shill? post-pump?)
- [ ] Exit timing (TP/SL levels, panic sells?)
- [ ] Slippage tolerance
- [ ] DEX preference (Raydium vs Orca vs Jupiter)

**Network Behavior:**
- [ ] Wash trading score (round-trip transfers)
- [ ] Repeated counterparties count
- [ ] Exchange usage frequency
- [ ] Cluster coordination score

**Success Metrics:**
- [ ] Win rate (% profitable trades)
- [ ] Avg profit per trade
- [ ] Total PnL
- [ ] Sharpe ratio / risk-adjusted returns

### 6.2 Strategy Clustering
- [ ] K-means on feature vectors → identify archetypes
- [ ] Label clusters: "MEV bot", "degen trader", "insider", "sniper", etc.
- [ ] Confidence scoring per label

### 6.3 Predictive Modeling
- [ ] Train classifier: features → will_profit (binary)
- [ ] Extract feature importance (what correlates with wins?)
- [ ] Real-time scoring: "This cluster has 78% win rate when doing X"

### 6.4 API Endpoints
- [ ] `GET /api/v1/patterns/wallet/{address}` - Detected strategy
- [ ] `GET /api/v1/patterns/successful` - What works now
- [ ] `GET /api/v1/patterns/predict?features={...}` - Predict success

### Priority: LOW
Advanced feature requiring Phases 4-5 complete first.

---

## ❌ PHASE 7: SHILL CORRELATION (0% Complete)

### Goal
Correlate Telegram/Twitter posts with on-chain activity to detect pre-shill accumulation.

### 7.1 Message Ingestion
- [ ] Telegram channel scraper (your own bot or logs)
- [ ] Twitter API integration (or manual exports)
- [ ] Store in `shill_messages` table:
  ```sql
  CREATE TABLE shill_messages (
      id SERIAL PRIMARY KEY,
      source TEXT,  -- 'telegram' | 'twitter'
      channel_id TEXT,
      message_text TEXT,
      timestamp BIGINT,
      mentioned_tokens TEXT[],
      mentioned_wallets TEXT[]
  );
  ```

### 7.2 Token Mention Extraction
- [ ] NLP/regex to extract token symbols, contract addresses, names
- [ ] Fuzzy matching (handle typos, variations)
- [ ] Link to canonical `mint` addresses

### 7.3 Correlation Engine
For each shill event:
- [ ] Find first buyers (T-24h to T+1h window)
- [ ] Find volume spikes (compare to baseline)
- [ ] Find cluster buys (multiple related wallets buying)
- [ ] Compute "anticipation score" (how many bought before shill?)

### 7.4 Cluster Profiling
- [ ] Track which clusters consistently buy pre-shill
- [ ] "Insider score" per cluster
- [ ] API: `GET /api/v1/shill/frontrunners?token={mint}`

### 7.5 Real-Time Monitoring
- [ ] WebSocket feed of new shills
- [ ] Alert when known-good clusters start accumulating
- [ ] Dashboard showing shill timeline + on-chain response

### Priority: MEDIUM
Very valuable for alpha discovery, but requires Phases 4-5.

---

## 🎯 RECOMMENDED PRIORITIZATION

### Immediate Next Steps (Week 1-2)
1. **Complete Exchange Intelligence** (Phase 3)
   - Add proper exchange wallet labeling
   - Build route tracking endpoints
   - Critical for understanding fund flows

2. **Start DEX Decoding** (Phase 4.1)
   - Focus on Raydium first (most volume)
   - Parse swap instructions
   - Store in `swap_events` table

### Short-Term (Week 3-4)
3. **Price Oracle Integration** (Phase 4.2)
   - Start with Jupiter API
   - Cache aggressively
   - Handle missing data gracefully

4. **Basic PnL Computation** (Phase 4.3)
   - Simple realized PnL first
   - Claim verification endpoint
   - Expand to unrealized later

### Medium-Term (Month 2)
5. **Time-Based Queries** (Phase 5)
   - Leverage swap data from Phase 4
   - Build timeline APIs
   - Enable "who bought when" queries

6. **Pattern Mining Foundations** (Phase 6.1-6.2)
   - Extract basic features
   - Simple clustering
   - Manual pattern identification

### Long-Term (Month 3+)
7. **Advanced Pattern Mining** (Phase 6.3-6.4)
   - Predictive models
   - Strategy detection
   - Real-time scoring

8. **Shill Correlation** (Phase 7)
   - Once we have solid swap/timeline data
   - Build correlation engine
   - Real-time monitoring

---

## 🔍 GAP ANALYSIS

### What We're Missing for Core Use Cases

#### Use Case 1: "Find side wallets for X"
- ✅ **READY:** All 5 signals operational, API working

#### Use Case 2: "Verify claim: wallet X made $123k on token Y"
- ❌ **BLOCKED:** Need DEX decoding + price oracle + PnL engine
- **Workaround:** Can manually trace transfers, but no automated PnL

#### Use Case 3: "Show me wallets that bought token Y before the shill"
- ⚠️ **PARTIAL:** Have timestamps, but need swap decoding + shill tracking
- **Workaround:** Can query transfer_events for time ranges

#### Use Case 4: "What strategies are working right now?"
- ❌ **BLOCKED:** Need full pattern mining pipeline
- **Dependency:** Requires Phases 4-6 complete

#### Use Case 5: "Trace where this wallet cashed out"
- ⚠️ **PARTIAL:** Basic exchange detection exists
- **Missing:** Route tracking, timing analysis, cluster patterns

---

## 🏗️ ARCHITECTURAL CONSIDERATIONS

### Data Pipeline
```
Solana RPC → Parser → Event Storage → Analysis → API
     ↓           ↓          ↓            ↓          ↓
  Raw TX    SOL/Token   transfer_   Evidence   REST
            transfers    events     framework  endpoints
                           ↓
                      swap_events
                      (MISSING)
                           ↓
                      token_prices
                      (MISSING)
                           ↓
                      PnL engine
                      (MISSING)
```

### Storage Requirements
- **Current:** ~1MB per 100 transactions (transfer events)
- **With swaps:** ~2MB per 100 transactions (need swap decoding)
- **With prices:** +500MB for historical price cache
- **Scaling:** PostgreSQL partitioning by block_time when > 1B events

### Performance Targets
- Side wallet query: <2s (currently ~1s ✅)
- PnL verification: <5s (not implemented)
- Timeline query: <3s (not implemented)
- Pattern detection: <10s (not implemented)

---

## 📊 TESTING STRATEGY

### Current Test Coverage
- ✅ Unit tests for parsers
- ✅ Integration tests for API
- ⚠️ No tests for evidence framework
- ❌ No tests for pattern mining (doesn't exist)

### Needed Tests
- [ ] Evidence signal accuracy (compare with ground truth)
- [ ] PnL computation accuracy (backtesting)
- [ ] Pattern detection precision/recall
- [ ] Shill correlation false positive rate

---

## 🚀 DEPLOYMENT STATUS

### Production Readiness
- ✅ **API:** Running, stable, documented
- ✅ **Database:** Optimized with indexes
- ✅ **Caching:** Redis operational
- ⚠️ **Monitoring:** Basic metrics, need alerting
- ⚠️ **Scaling:** Single instance, need load balancing

### Missing for Production
- [ ] Authentication/authorization (currently disabled)
- [ ] Rate limiting per user (currently global)
- [ ] Database backups automated
- [ ] Error alerting (Sentry/PagerDuty)
- [ ] Load balancer + multiple instances
- [ ] CI/CD pipeline

---

## 💡 RECOMMENDATIONS

### Before Building New Features
1. **Test current evidence framework thoroughly**
   - Run backfill on 100+ known cabal wallets
   - Validate signal accuracy
   - Tune weights if needed

2. **Document API comprehensively**
   - OpenAPI/Swagger spec
   - Usage examples
   - Rate limits and quotas

3. **Benchmark performance**
   - Load test with concurrent requests
   - Identify bottlenecks
   - Optimize hot paths

### Feature Development Order
Based on value/effort ratio:

1. **DEX Decoding** (HIGH value, MEDIUM effort) ← START HERE
2. **Exchange Tracking** (HIGH value, LOW effort)
3. **Price Oracle** (HIGH value, LOW effort)
4. **PnL Computation** (HIGH value, MEDIUM effort)
5. **Timeline Queries** (MEDIUM value, LOW effort)
6. **Pattern Mining** (HIGH value, HIGH effort)
7. **Shill Correlation** (MEDIUM value, HIGH effort)

### Technical Debt
- [ ] Refactor `TransactionHandler` (too many responsibilities)
- [ ] Split large API files (server.rs is 1400+ lines)
- [ ] Add comprehensive error messages
- [ ] Improve logging (structured logs with context)
- [ ] Add health check for all dependencies

---

## 📈 SUCCESS METRICS

### Phase 2 (Current) - Wallet Clustering
- ✅ API returns results in <2s
- ✅ Evidence framework 100% complete
- ⚠️ Need: Accuracy validation against known clusters

### Phase 4 - PnL Verification
- Target: 95% price coverage for top 1000 tokens
- Target: PnL accuracy within ±5% vs manual calculation
- Target: <5s response time for claim verification

### Phase 6 - Pattern Mining
- Target: Identify 5+ distinct strategy archetypes
- Target: 70%+ accuracy predicting profitable trades
- Target: Discover 3+ repeatable alpha signals

### Phase 7 - Shill Correlation
- Target: Detect pre-shill accumulation with 80%+ precision
- Target: <10% false positive rate
- Target: Real-time alerts within 60s of shill event

---

## 🎯 NEXT SESSION PRIORITIES

1. **Decide:** DEX decoding vs Exchange tracking (which first?)
2. **Plan:** Database schema for swap_events and token_prices
3. **Build:** Core DEX decoder for Raydium swaps
4. **Test:** Validate swap extraction on known transactions
5. **Iterate:** Extend to other DEXes (Orca, Jupiter)

**Question for you:** Should we start with DEX decoding (unlock PnL + timelines) or exchange tracking (simpler, immediate value)?

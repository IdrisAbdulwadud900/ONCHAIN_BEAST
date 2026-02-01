# OnChain Beast - Evidence-Based Heuristic Model

## Overview

OnChain Beast uses a **multi-signal heuristic model** to identify wallet clusters and side-wallets. Unlike black-box ML, each signal is independently verifiable on-chain, giving users full transparency into why wallets are linked.

---

## Signals & Evidence Types

### 1. **Graph Connectivity** (30% weight)
**What it is:** Direct wallet-to-wallet relationships from `wallet_relationships` table.

**How it works:**
- BFS traversal over aggregate transfer relationships
- Scores based on: transaction count, SOL volume, token volume
- Deeper connections (2+ hops) are penalized
- Recent relationships are favored (30-day decay)

**Evidence shown:**
```
Link: wallet_a ↔ wallet_b (45 tx, 12.5 SOL)
```

**Query:** `GET /wallet_relationships` → from/to wallets with totals

---

### 2. **Shared Inbound Funders** (25% weight)
**What it is:** Wallets that sent funds to **both** the main wallet and a candidate.

**Why it matters:** If wallet X funded both A and B, they may be coordinated.

**How it works:**
- Event-level query: `SELECT from_wallet FROM transfer_events WHERE to_wallet IN (A, B)`
- Find intersection of senders
- Count events per shared funder
- Lookback window: configurable (default 30 days)

**Evidence shown:**
```
Shared inbound funder: 0xABC... (23 events; last_seen=1704067200)
```

**Query:** `GET /shared-inbound-senders?lookback_days=30` → wallet addresses with event counts

---

### 3. **Shared Counterparties** (20% weight)
**What it is:** Wallets that **both** interact with the same external entities (DEXes, exchanges, contracts).

**Why it matters:** If A and B both use the same DEX or send to the same address, coordination is likely.

**How it works:**
- Event-level query: `SELECT to_wallet FROM transfer_events WHERE from_wallet = A`
- Event-level query: `SELECT to_wallet FROM transfer_events WHERE from_wallet = B`
- Find intersection of to_wallets
- Lookback window: configurable

**Evidence shown:**
```
Shared counterparty: 0xXYZ... (Jupiter DEX or Exchange)
```

**Query:** `GET /top-counterparties` → returns ranked list of destinations with event counts

---

### 4. **Behavioral Correlation** (15% weight)
**What it is:** Similar transaction patterns (amounts, frequency, time-of-day).

**Why it matters:** If A and B have matching behavior patterns, coordination is likely.

**How it works:**
- Event-level query: `GET /behavioral-profile` → avg_sol_per_tx, tx_per_day, most_active_hour_utc
- Compute similarity score:
  - **TX amount similarity** (40%): log-ratio of avg_sol_per_tx with exponential decay
  - **Frequency similarity** (35%): log-ratio of tx_per_day with exponential decay
  - **Time-of-day clustering** (25%): circular distance between most_active_hour (0-23)
- Similarity > 0.65 → adds "Behavioral pattern match" evidence + score boost

**Evidence shown:**
```
Behavioral pattern match (similarity: 0.82)
```

**Example:**
- Wallet A: avg 2.5 SOL/tx, 15 tx/day, active at 3am UTC
- Wallet B: avg 2.8 SOL/tx, 18 tx/day, active at 2am UTC
- Similarity: 0.78 → Strong behavioral match

**Query:** `GET /behavioral-profile?lookback_days=30` → returns BehavioralProfile struct

**Status:** ✅ **Implemented**

---

### 5. **Temporal Alignment** (10% weight - future)
**What it is:** Suspicious synchronization in activity timing.

**Signals:**
- Same transaction within N minutes
- Both trade same token in same time window
- Synchronized buy/sell patterns

**Status:** Pending implementation

---

## Scoring Formula

```
score = (
  (graph_connectivity_score × 0.30)
  + (shared_funders_strength × 0.25)
  + (shared_counterparties_strength × 0.20)
  + (behavioral_correlation × 0.15)
  + (temporal_alignment × 0.10)
)
```

**Score range:** 0.0 - 1.0 (higher = more likely related)

**Tuning:** All weights are in `src/api/server.rs` and can be adjusted per deployment.

---

## API Endpoints

### Find Side Wallets (Heuristic-based)
```
GET /api/v1/wallet/{address}/side-wallets
  ?depth=2
  &threshold=0.10
  &limit=15
  &lookback_days=30
  &bootstrap=true
  &bootstrap_limit=25
```

**Response:**
```json
{
  "main_wallet": "wallet_address",
  "side_wallets": [
    {
      "address": "...",
      "score": 0.67,
      "depth": 2,
      "direction": "inbound",
      "shared_funders_count": 3,
      "shared_counterparties_count": 5,
      "behavioral_similarity": 0.78,
      "shared_funders": ["wallet_x (12 events)", "wallet_y (8 events)"],
      "shared_counterparties": ["dex_address_1", "dex_address_2", "exchange_address"],
      "reasons": [
        "Link: wallet_a ↔ wallet_b (45 tx, 12.5 SOL)",
        "Shared inbound funder: 0xABC... (23 events; last_seen=...)",
        "Shared counterparty: 0xJupiter...",
        "Behavioral pattern match (similarity: 0.78)"
      ]
    }
  ],
  "lookback_days": 30
}
```
**What it is:** Suspicious synchronization in activity timing.

**Signals:**
- Same transaction within N minutes
- Both trade same token in same time window
- Synchronized buy/sell patterns

**Status:** Pending implementation

---

## Scoring Formula

```
score = (
  (graph_connectivity_score × 0.30)
  + (shared_funders_strength × 0.25)
  + (shared_counterparties_strength × 0.20)
  + (behavioral_correlation × 0.15)
  + (temporal_alignment × 0.10)
)
```

**Score range:** 0.0 - 1.0 (higher = more likely related)

**Tuning:** All weights are in `src/api/server.rs` and can be adjusted per deployment.

---

## API Endpoints

### Find Side Wallets (Heuristic-based)
```
GET /api/v1/wallet/{address}/side-wallets
  ?depth=2
  &threshold=0.10
  &limit=15
  &lookback_days=30
  &bootstrap=true
  &bootstrap_limit=25
```

**Response:**
```json
{
  "main_wallet": "wallet_address",
  "side_wallets": [
    {
      "address": "...",
      "score": 0.67,
      "depth": 2,
      "direction": "inbound",
      "shared_funders_count": 3,
      "shared_counterparties_count": 5,
      "shared_funders": ["wallet_x (12 events)", "wallet_y (8 events)"],
      "shared_counterparties": ["dex_address_1", "dex_address_2", "exchange_address"],
      "reasons": [
        "Link: wallet_a ↔ wallet_b (45 tx, 12.5 SOL)",
        "Shared inbound funder: 0xABC... (23 events; last_seen=...)",
        "Shared counterparty: 0xJupiter..."
      ]
    }
  ],
  "lookback_days": 30
}
```

### Get Wallet Cluster
```
GET /api/v1/wallet/{address}/cluster
```

Same signals, returns primary wallet + all discovered members.

---

## Database Tables

### `wallet_relationships` (aggregate)
```sql
from_wallet, to_wallet, sol_amount, token_amount, 
transaction_count, first_seen, last_seen
```

Used for graph traversal (BFS).

### `transfer_events` (event-level)
```sql
signature, event_index, slot, block_time,
kind ('sol' | 'token'),
from_wallet, to_wallet, mint, amounts...
```

Used for:
- Shared funder detection
- Shared counterparty detection
- Behavioral profile computation (avg amounts, frequency, timing)
- Temporal analysis (future)

---

## Interpretation Guide

**What does a high score mean?**
- Score > 0.7: Strong evidence of relationship
  - Multiple independent signals align (graph + shared funders + behavioral match)
  - User should review evidence details
  
- Score 0.4-0.7: Moderate evidence
  - One or two signals present
  - Could be coincidence or indirect connection
  
- Score < 0.4: Weak evidence
  - Minimal signals
  - Likely unrelated or very indirect

**Example case:**
```
Wallet A and B have:
  ✅ Graph path (base score: 0.30)
  ✅ Shared funder (3 events) → +0.18 bump
  ✅ Behavioral match (similarity: 0.75) → +0.09 bump
  ❌ Shared counterparties (0) → +0 bump
  
  Total: 0.30 + 0.18 + 0.09 = 0.57 → Moderate confidence
```

---

## Configuration

### Deploy-time parameters
Set in `.env` or environment:
```bash
# Lookback window for event-level queries
LOOKBACK_DAYS=30
```

### Signal weights (edit src/api/server.rs)
```rust
// Scoring weights
GRAPH_WEIGHT=0.30
SHARED_FUNDERS_WEIGHT=0.25
SHARED_COUNTERPARTIES_WEIGHT=0.20
BEHAVIORAL_WEIGHT=0.15
TEMPORAL_WEIGHT=0.10

// Behavioral similarity components
AVG_SOL_WEIGHT=0.40
FREQUENCY_WEIGHT=0.35
HOUR_OF_DAY_WEIGHT=0.25
```

### Query-time parameters
Adjustable per request:
```bash
?depth=2                    # BFS depth (1-5)
&threshold=0.10            # Min score to return (0.0-1.0)
&limit=15                  # Max results
&lookback_days=30          # Event window (1-365 days)
&bootstrap=true            # Ingest recent txs before analyzing
```

---

## Known Limitations

1. **No training data required** → But also means no ML optimization
2. **Depends on RPC availability** → Bootstrap ingestion needs Solana RPC
3. **Only public data** → Can't see private fund movements
4. **No cross-chain** → Solana-only (for now)
5. **Timing-dependent** → Results improve as more events accumulate in DB

---

## Future Enhancements

- [ ] Temporal alignment (synchronized activity windows)
- [ ] MEV sandwich pattern detection
- [ ] Token volatility correlation
- [ ] Cross-chain bridge tracking
- [ ] PnL verification endpoints
- [ ] Custom weight tuning per cluster

---

## References

- Database schema: [src/storage/database.rs](src/storage/database.rs)
- Side-wallet logic: [src/api/server.rs](src/api/server.rs) (`compute_side_wallets`)
- Evidence enrichment: [src/api/server.rs](src/api/server.rs) (`enrich_candidates_with_event_signals`)
- Behavioral similarity: [src/api/server.rs](src/api/server.rs) (`compute_behavioral_similarity`)

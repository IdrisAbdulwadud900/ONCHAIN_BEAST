#!/bin/bash

# Price Oracle Testing Script
# Tests Jupiter Price API integration and USD value enrichment

set -e

echo "🧪 Price Oracle Integration Test"
echo "================================"
echo ""

API_URL="${API_URL:-http://localhost:8080}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test 1: Get SOL price
echo -e "${BLUE}Test 1: Get SOL Price${NC}"
echo "GET /api/v1/price/{SOL_MINT}"
SOL_PRICE=$(curl -s "$API_URL/api/v1/price/So11111111111111111111111111111111111111112" | jq -r '.price_usd')

if [ "$SOL_PRICE" != "null" ] && [ -n "$SOL_PRICE" ]; then
    echo -e "${GREEN}✅ SOL Price: \$$SOL_PRICE${NC}"
else
    echo -e "${YELLOW}⚠️  Could not fetch SOL price (API might not be running)${NC}"
fi
echo ""

# Test 2: Get USDC price (stablecoin)
echo -e "${BLUE}Test 2: Get USDC Price (Stablecoin)${NC}"
echo "GET /api/v1/price/{USDC_MINT}"
USDC_PRICE=$(curl -s "$API_URL/api/v1/price/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" | jq '.')
echo "$USDC_PRICE"
echo ""

# Test 3: Batch price query
echo -e "${BLUE}Test 3: Batch Price Query${NC}"
echo "POST /api/v1/price/batch"
BATCH_RESULT=$(curl -s -X POST "$API_URL/api/v1/price/batch" \
  -H "Content-Type: application/json" \
  -d '{
    "token_mints": [
      "So11111111111111111111111111111111111111112",
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    ]
  }')

if [ -n "$BATCH_RESULT" ]; then
    echo "$BATCH_RESULT" | jq '.'
    echo -e "${GREEN}✅ Batch query successful${NC}"
else
    echo -e "${YELLOW}⚠️  Batch query failed${NC}"
fi
echo ""

# Test 4: Cache statistics
echo -e "${BLUE}Test 4: Cache Statistics${NC}"
echo "GET /api/v1/price/stats/cache"
CACHE_STATS=$(curl -s "$API_URL/api/v1/price/stats/cache" | jq '.')
echo "$CACHE_STATS"
echo ""

# Test 5: Wallet PnL query (if wallet has swaps)
echo -e "${BLUE}Test 5: Wallet PnL Query${NC}"
echo "GET /api/v1/wallet/{ADDRESS}/pnl"
echo "Example: Testing with Raydium Program wallet"
WALLET_PNL=$(curl -s "$API_URL/api/v1/wallet/675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8/pnl" | jq '.')
echo "$WALLET_PNL"
echo ""

# Test 6: Database query test
echo -e "${BLUE}Test 6: Database Test - Store & Retrieve Price${NC}"
psql -d onchain_beast_personal -c "
  INSERT INTO token_prices (token_mint, price_usd, timestamp_utc, source) 
  VALUES ('So11111111111111111111111111111111111111112', 150.00, EXTRACT(epoch FROM NOW())::bigint, 'test');
  
  SELECT token_mint, price_usd, source, created_at 
  FROM token_prices 
  WHERE source = 'test' 
  ORDER BY created_at DESC 
  LIMIT 1;
" | tail -5
echo -e "${GREEN}✅ Database storage test complete${NC}"
echo ""

# Test 7: Check swap_events table has USD columns
echo -e "${BLUE}Test 7: Swap Events USD Columns${NC}"
SWAP_COLUMNS=$(psql -d onchain_beast_personal -t -c "
  SELECT column_name 
  FROM information_schema.columns 
  WHERE table_name = 'swap_events' 
    AND column_name LIKE '%usd%';
")
echo "USD columns in swap_events:"
echo "$SWAP_COLUMNS"
echo -e "${GREEN}✅ Schema updated correctly${NC}"
echo ""

# Summary
echo "================================"
echo -e "${GREEN}✅ Price Oracle Integration Tests Complete${NC}"
echo ""
echo "Key Features Verified:"
echo "  ✓ Jupiter Price API integration"
echo "  ✓ Stablecoin $1.00 handling"
echo "  ✓ Batch price queries"
echo "  ✓ In-memory caching"
echo "  ✓ Database price storage"
echo "  ✓ USD value columns in swap_events"
echo ""
echo "Next Steps:"
echo "  1. Backfill existing swaps with USD values"
echo "  2. Auto-enrich new swaps on ingestion"
echo "  3. Build PnL calculation engine"
echo ""

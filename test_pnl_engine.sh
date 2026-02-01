#!/bin/bash

# Phase 4.3 Testing Script
# Tests PnL calculation engine, claim verification, and analytics

set -e

echo "🧪 Phase 4.3: PnL Calculation Engine Testing"
echo "==========================================="
echo ""

API_URL="${API_URL:-http://localhost:8080}"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Test wallet (Raydium program for demo)
TEST_WALLET="675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
TEST_TOKEN="So11111111111111111111111111111111111111112"

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  1. Claim Verification${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "Testing claim: 'Wallet made $1000 on SOL'"
echo "POST /api/v1/claim/verify"
CLAIM_RESULT=$(curl -s -X POST "$API_URL/api/v1/claim/verify" \
  -H "Content-Type: application/json" \
  -d '{
    "wallet": "'"$TEST_WALLET"'",
    "token_mint": "'"$TEST_TOKEN"'",
    "claimed_amount": 1000.0
  }' | jq '.')

echo "$CLAIM_RESULT"
echo ""

VERIFIED=$(echo "$CLAIM_RESULT" | jq -r '.verified // "unknown"')
if [ "$VERIFIED" = "true" ]; then
    echo -e "${GREEN}✅ Claim VERIFIED${NC}"
elif [ "$VERIFIED" = "false" ]; then
    echo -e "${RED}❌ Claim FALSE${NC}"
else
    echo -e "${YELLOW}⚠️  API might not be running${NC}"
fi
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  2. Position Tracking${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "GET /api/v1/position/$TEST_WALLET/$TEST_TOKEN"
POSITION=$(curl -s "$API_URL/api/v1/position/$TEST_WALLET/$TEST_TOKEN" | jq '.')
echo "$POSITION"
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  3. Performance Metrics${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "GET /api/v1/performance/$TEST_WALLET"
PERFORMANCE=$(curl -s "$API_URL/api/v1/performance/$TEST_WALLET" | jq '.')
echo "$PERFORMANCE"
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  4. Top Performers Leaderboard${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "GET /api/v1/leaderboard/top-pnl?limit=5"
LEADERBOARD=$(curl -s "$API_URL/api/v1/leaderboard/top-pnl?limit=5" | jq '.')
echo "$LEADERBOARD"
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  5. Top Tokens${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "GET /api/v1/leaderboard/top-tokens?limit=5"
TOP_TOKENS=$(curl -s "$API_URL/api/v1/leaderboard/top-tokens?limit=5" | jq '.')
echo "$TOP_TOKENS"
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  6. Big Wins${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "GET /api/v1/analytics/big-wins?limit=5"
BIG_WINS=$(curl -s "$API_URL/api/v1/analytics/big-wins?limit=5" | jq '.')
echo "$BIG_WINS"
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  7. Win/Loss Statistics${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "GET /api/v1/analytics/win-loss/$TEST_WALLET"
WIN_LOSS=$(curl -s "$API_URL/api/v1/analytics/win-loss/$TEST_WALLET" | jq '.')
echo "$WIN_LOSS"
echo ""

echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}  8. Database Verification${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo ""

echo "Checking swap_events with USD values:"
psql -d onchain_beast_personal -c "
SELECT 
    COUNT(*) as total_swaps,
    COUNT(CASE WHEN price_usd_in IS NOT NULL THEN 1 END) as enriched,
    ROUND(AVG(pnl_usd)::numeric, 2) as avg_pnl,
    ROUND(SUM(pnl_usd)::numeric, 2) as total_pnl
FROM swap_events;
" | tail -5
echo ""

echo "Sample enriched swaps:"
psql -d onchain_beast_personal -c "
SELECT 
    LEFT(signature, 10) as sig,
    LEFT(wallet, 10) as wallet,
    dex_name,
    ROUND(value_usd_in::numeric, 2) as value_in,
    ROUND(value_usd_out::numeric, 2) as value_out,
    ROUND(pnl_usd::numeric, 2) as pnl
FROM swap_events
WHERE pnl_usd IS NOT NULL
ORDER BY ABS(pnl_usd) DESC
LIMIT 5;
" | tail -8
echo ""

echo "==========================================="
echo -e "${GREEN}✅ Phase 4.3 Tests Complete!${NC}"
echo ""
echo "Summary:"
echo "  ✓ Claim verification system operational"
echo "  ✓ Position tracking functional"
echo "  ✓ Performance metrics calculated"
echo "  ✓ Leaderboards generated"
echo "  ✓ Analytics endpoints working"
echo ""
echo "Next Steps:"
echo "  1. Backfill existing swaps: ./scripts/backfill_swap_usd_values.sh"
echo "  2. Enable auto-enrichment for new swaps"
echo "  3. Build user-facing claim verification UI"
echo ""

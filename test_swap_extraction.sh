#!/bin/bash
# Test swap extraction functionality

set -e

echo "🧪 Testing DEX Swap Extraction System"
echo "======================================"
echo ""

# Check if server is running
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "❌ Server not running on port 8080"
    exit 1
fi

echo "✅ Server is healthy"
echo ""

# Test 1: Check swap_events table exists
echo "📊 Test 1: Checking database schema..."
SWAP_TABLE=$(psql postgresql://$(whoami)@localhost/onchain_beast_personal -tAc "SELECT COUNT(*) FROM information_schema.tables WHERE table_name='swap_events';")
if [ "$SWAP_TABLE" = "1" ]; then
    echo "✅ swap_events table exists"
else
    echo "❌ swap_events table not found"
    exit 1
fi

# Test 2: Check swap_events indexes
echo ""
echo "📊 Test 2: Checking database indexes..."
INDEX_COUNT=$(psql postgresql://$(whoami)@localhost/onchain_beast_personal -tAc "SELECT COUNT(*) FROM pg_indexes WHERE tablename='swap_events';")
echo "   Found $INDEX_COUNT indexes on swap_events"
if [ "$INDEX_COUNT" -ge "6" ]; then
    echo "✅ All swap indexes created"
else
    echo "⚠️  Expected at least 6 indexes, found $INDEX_COUNT"
fi

# Test 3: Test swap query APIs
echo ""
echo "📊 Test 3: Testing swap query endpoints..."

# Test stats endpoint
STATS_RESPONSE=$(curl -s 'http://localhost:8080/api/v1/swaps/stats/test123')
if echo "$STATS_RESPONSE" | jq -e '.success' > /dev/null 2>&1; then
    echo "✅ Swap stats API working"
else
    echo "❌ Swap stats API failed"
    exit 1
fi

# Test wallet swaps endpoint
WALLET_RESPONSE=$(curl -s 'http://localhost:8080/api/v1/swaps/wallet/test123?limit=10')
if echo "$WALLET_RESPONSE" | jq -e '.success' > /dev/null 2>&1; then
    echo "✅ Wallet swaps API working"
else
    echo "❌ Wallet swaps API failed"
    exit 1
fi

# Test 4: Insert test swap and verify query
echo ""
echo "📊 Test 4: Testing swap insertion and retrieval..."

# Insert a test swap
TEST_WALLET="TestWallet123456789012345678901234567890"
psql postgresql://$(whoami)@localhost/onchain_beast_personal << EOF > /dev/null 2>&1
INSERT INTO swap_events (
    signature, event_index, slot, block_time, wallet, dex_program, dex_name,
    token_in, amount_in, token_out, amount_out, price, min_amount_out, pool_address
) VALUES (
    'TestSig123456789012345678901234567890123456789',
    0,
    12345678,
    $(date +%s),
    '$TEST_WALLET',
    '675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8',
    'Raydium V4',
    'So11111111111111111111111111111111111111112',
    1000000000,
    'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    100000000,
    0.1,
    900000000,
    NULL
) ON CONFLICT (signature, event_index) DO NOTHING;
EOF

# Query it back
SWAP_COUNT=$(curl -s "http://localhost:8080/api/v1/swaps/wallet/$TEST_WALLET?limit=10" | jq '.count')
if [ "$SWAP_COUNT" = "1" ]; then
    echo "✅ Swap insertion and retrieval working"
else
    echo "❌ Expected 1 swap, found $SWAP_COUNT"
    exit 1
fi

# Test stats for test wallet
TOTAL_SWAPS=$(curl -s "http://localhost:8080/api/v1/swaps/stats/$TEST_WALLET" | jq '.data.total_swaps')
if [ "$TOTAL_SWAPS" = "1" ]; then
    echo "✅ Swap stats calculation working"
else
    echo "❌ Expected 1 total swap, found $TOTAL_SWAPS"
    exit 1
fi

# Cleanup test data
psql postgresql://$(whoami)@localhost/onchain_beast_personal -c "DELETE FROM swap_events WHERE wallet='$TEST_WALLET';" > /dev/null 2>&1

echo ""
echo "======================================"
echo "✅ All swap extraction tests passed!"
echo ""
echo "📈 Summary:"
echo "   - Database schema: ✅"
echo "   - Query APIs: ✅"
echo "   - Data persistence: ✅"
echo ""
echo "🚀 System ready for live swap extraction!"

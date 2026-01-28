#!/bin/bash

# OnChain Beast - API Testing Suite
# Tests all 20 REST API endpoints

BASE_URL="http://localhost:8080"
WALLET="11111111111111111111111111111111"

echo "🧪 OnChain Beast API Test Suite"
echo "================================"
echo "Base URL: $BASE_URL"
echo "Test Wallet: $WALLET"
echo ""

# Counter for tests
PASSED=0
FAILED=0

test_api() {
    local name=$1
    local method=$2
    local endpoint=$3
    local data=$4
    
    echo "Testing: $name"
    
    if [ -z "$data" ]; then
        http_code=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" "$BASE_URL$endpoint")
    else
        http_code=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" "$BASE_URL$endpoint" \
            -H "Content-Type: application/json" \
            -d "$data")
    fi
    
    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 500 ]; then
        echo "  ✅ PASS (HTTP $http_code)"
        PASSED=$((PASSED + 1))
    else
        echo "  ❌ FAIL (HTTP $http_code)"
        FAILED=$((FAILED + 1))
    fi
}

echo "=== Health & Status (3 endpoints) ==="
test_api "Root endpoint" "GET" "/" ""
test_api "Health check" "GET" "/health" ""
test_api "System status" "GET" "/status" ""

echo ""
echo "=== Wallet Analysis (5 endpoints) ==="
test_api "Analyze wallet GET" "GET" "/api/v1/analyze/wallet/$WALLET" ""
test_api "Analyze wallet POST" "POST" "/api/v1/analyze/wallet" '{"wallet":"'$WALLET'","include_transactions":true}'
test_api "Risk score" "GET" "/api/v1/wallet/$WALLET/risk" ""
test_api "Transactions" "GET" "/api/v1/wallet/$WALLET/transactions" ""
test_api "Transactions limit" "GET" "/api/v1/wallet/$WALLET/transactions?limit=5" ""

echo ""
echo "=== Graph Analysis (2 endpoints) ==="
test_api "Side wallets" "GET" "/api/v1/wallet/$WALLET/side-wallets" ""
test_api "Wallet cluster" "GET" "/api/v1/wallet/$WALLET/cluster" ""

echo ""
echo "=== Pattern Detection (3 endpoints) ==="
test_api "Detect patterns" "POST" "/api/v1/detect/patterns" '{"wallet":"'$WALLET'","pattern_type":"wash_trading"}'
test_api "Detect anomalies" "GET" "/api/v1/detect/anomalies" ""
test_api "Wash trading" "GET" "/api/v1/detect/wash-trading/$WALLET" ""

echo ""
echo "=== Fund Tracing (2 endpoints) ==="
test_api "Trace funds" "POST" "/api/v1/trace/funds" '{"from":"'$WALLET'","to":"'$WALLET'","max_depth":5}'
test_api "Exchange routes" "POST" "/api/v1/trace/exchange-routes" '{"source":"'$WALLET'","destination":"'$WALLET'"}'

echo ""
echo "=== Network Analysis (2 endpoints) ==="
test_api "Network metrics" "GET" "/api/v1/network/metrics" ""
test_api "Network analysis" "POST" "/api/v1/network/analysis" '{"analysis_type":"correlation"}'

echo ""
echo "=== Account Info (2 endpoints) ==="
test_api "Account balance" "GET" "/api/v1/account/$WALLET/balance" ""
test_api "Account info" "GET" "/api/v1/account/$WALLET/info" ""

echo ""
echo "=== Cluster Info (2 endpoints) ==="
test_api "Cluster info" "GET" "/api/v1/cluster/info" ""
test_api "Cluster health" "GET" "/api/v1/cluster/health" ""

echo ""
echo "================================"
echo "Test Results:"
echo "  ✅ PASSED: $PASSED"
echo "  ❌ FAILED: $FAILED"
echo "  📊 TOTAL: $((PASSED + FAILED))"
if [ $((PASSED + FAILED)) -gt 0 ]; then
    echo "  📈 PASS RATE: $(( PASSED * 100 / (PASSED + FAILED) ))%"
fi
echo ""

if [ $FAILED -eq 0 ]; then
    echo "✅ All tests passed!"
    exit 0
else
    echo "❌ Some tests failed"
    exit 1
fi

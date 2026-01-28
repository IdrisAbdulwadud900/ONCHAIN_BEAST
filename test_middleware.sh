#!/bin/bash
# Test script for middleware functionality

echo "🧪 Testing Middleware Integration"
echo "=================================="
echo ""

# Start server in background
echo "Starting server..."
./target/release/onchain_beast > /dev/null 2>&1 &
SERVER_PID=$!

# Wait for server to start
sleep 3

echo "✅ Server started (PID: $SERVER_PID)"
echo ""

# Test 1: Public endpoint (should work without auth)
echo "Test 1: Public endpoint /health (no auth required)"
curl -s -w "\nStatus: %{http_code}\n" http://localhost:8080/health | head -3
echo ""

# Test 2: Protected endpoint without API key (should fail if auth enabled)
echo "Test 2: Protected endpoint /api/v1/cluster/info (no API key)"
curl -s -w "\nStatus: %{http_code}\n" http://localhost:8080/api/v1/cluster/info | head -3
echo ""

# Test 3: Check request ID header
echo "Test 3: Check X-Request-ID header"
curl -s -I http://localhost:8080/health | grep -i "x-request-id"
echo ""

# Test 4: Rate limiting (make multiple requests)
echo "Test 4: Rate limiting (making 5 rapid requests)"
for i in {1..5}; do
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/status)
    echo "Request $i: HTTP $STATUS"
done
echo ""

# Test 5: With valid API key (if auth is enabled)
echo "Test 5: With API key (if auth enabled)"
curl -s -w "\nStatus: %{http_code}\n" -H "X-API-Key: test-key-123" http://localhost:8080/api/v1/cluster/info | head -3
echo ""

# Cleanup
echo "Stopping server..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo ""
echo "✅ Middleware tests complete!"

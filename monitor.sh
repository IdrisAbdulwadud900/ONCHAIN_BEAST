#!/bin/bash
# Monitor OnChain Beast application

set -e

echo "📊 OnChain Beast - Monitoring Dashboard"
echo "======================================"
echo ""

# Function to check service status
check_service() {
    local service=$1
    local port=$2
    
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1 ; then
        echo "✓ $service is running (port $port)"
    else
        echo "✗ $service is NOT running (port $port)"
    fi
}

# Function to check database
check_database() {
    if psql onchain_beast_personal -c "SELECT 1" >/dev/null 2>&1; then
        local count=$(psql onchain_beast_personal -tc "SELECT COUNT(*) FROM transactions;")
        echo "✓ PostgreSQL is running (transactions: $count)"
    else
        echo "✗ PostgreSQL is NOT running"
    fi
}

# Function to check redis
check_redis() {
    if redis-cli ping >/dev/null 2>&1; then
        local mem=$(redis-cli info memory | grep used_memory_human | cut -d: -f2 | tr -d '\r')
        echo "✓ Redis is running (memory: $mem)"
    else
        echo "✗ Redis is NOT running"
    fi
}

# Run checks
echo "Services:"
check_service "OnChain Beast API" 8080
check_service "Prometheus Metrics" 9090
echo ""

echo "Databases:"
check_database
check_redis
echo ""

# Get API health
echo "API Health:"
if curl -s http://127.0.0.1:8080/health > /dev/null 2>&1; then
    echo "✓ API endpoint healthy"
else
    echo "✗ API endpoint not responding"
fi

echo ""
echo "Logs:"
tail -5 logs/onchain_beast.log 2>/dev/null || echo "No logs available yet"

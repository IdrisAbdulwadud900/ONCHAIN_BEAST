#!/bin/bash
# Start OnChain Beast application

set -e

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$PROJECT_DIR"

echo "🚀 Starting OnChain Beast..."
echo "=============================="
echo ""

# Load environment variables
if [ -f .env ]; then
    # Safer than `export $(cat .env | ...)` and supports comments/quoting.
    set -a
    # shellcheck disable=SC1091
    . ./.env
    set +a
else
    echo "⚠️  .env not found, using defaults..."
    export SERVER_HOST="127.0.0.1"
    export SERVER_PORT="8080"
    export METRICS_PORT="9090"
fi

echo "📊 Checking dependencies..."

# Check if PostgreSQL is available
if ! command -v psql &> /dev/null; then
    echo "❌ PostgreSQL not found. Please install PostgreSQL first:"
    echo "   MacOS: brew install postgresql && brew services start postgresql"
    echo "   Ubuntu: sudo apt-get install postgresql && sudo systemctl start postgresql"
    exit 1
fi
echo "  ✓ PostgreSQL available"

# Check if Redis is available
if ! command -v redis-cli &> /dev/null; then
    echo "❌ Redis not found. Please install Redis first:"
    echo "   MacOS: brew install redis && brew services start redis"
    echo "   Ubuntu: sudo apt-get install redis-server && sudo systemctl start redis"
    exit 1
fi
echo "  ✓ Redis available"

# Verify PostgreSQL is running
echo "Checking PostgreSQL status..."
if command -v pg_isready &> /dev/null; then
    if ! pg_isready -q; then
        echo ""
        echo "⚠️  PostgreSQL not responding. Starting..."
        if command -v brew &> /dev/null; then
            brew services start postgresql 2>/dev/null || true
        fi
        sleep 2
    fi
else
    if ! psql -d postgres -c "SELECT 1" > /dev/null 2>&1; then
        echo ""
        echo "⚠️  PostgreSQL not responding. Please start it (e.g. via brew services)."
    fi
fi
echo "  ✓ PostgreSQL running"

# Verify Redis is running
if ! redis-cli -h 127.0.0.1 -p 6379 ping > /dev/null 2>&1; then
    echo ""
    echo "⚠️  Redis not responding. Starting..."
    if command -v brew &> /dev/null; then
        brew services start redis 2>/dev/null || true
    fi
    sleep 1
fi
echo "  ✓ Redis running"

echo ""
echo "✅ All dependencies ready"
echo ""

# If already running, avoid a confusing bind failure.
if command -v lsof &> /dev/null; then
    EXISTING_PID="$(lsof -ti tcp:${SERVER_PORT} 2>/dev/null | head -n 1)"
    if [ -n "$EXISTING_PID" ]; then
        echo "ℹ️  API already running on ${SERVER_HOST}:${SERVER_PORT} (pid ${EXISTING_PID})"
        echo "   Health: curl http://${SERVER_HOST}:${SERVER_PORT}/health"
        exit 0
    fi
fi

# Start the application
echo "⚙️  Starting application server..."
echo "  API Server: http://${SERVER_HOST}:${SERVER_PORT}"
echo "  Metrics: http://${SERVER_HOST}:${METRICS_PORT}/metrics"
echo "  Health Check: curl http://${SERVER_HOST}:${SERVER_PORT}/health"
echo ""
echo "📋 Press Ctrl+C to stop"
echo ""

if [ ! -x ./target/release/onchain_beast ]; then
    echo "⚙️  Release binary not found. Building..."
    cargo build --release
    echo "✅ Build complete"
    echo ""
fi

exec ./target/release/onchain_beast "$@"

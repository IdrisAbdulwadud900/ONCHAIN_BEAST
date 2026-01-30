#!/bin/bash
# OnChain Beast - Deployment Setup Script for Personal Use
# Creates configuration files and directories for local deployment

set -e

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$PROJECT_DIR"

echo "🚀 OnChain Beast - Personal Deployment Setup"
echo "=============================================="
echo ""

# Create necessary directories
echo "📁 Creating directories..."
mkdir -p data
mkdir -p logs
mkdir -p config
mkdir -p backups

# Create .env file for personal use
echo "⚙️  Creating configuration files..."

if [ -f .env ]; then
    echo "ℹ️  .env already exists; not overwriting."
else
cat > .env << 'EOF'
# OnChain Beast - Personal Configuration
# For personal use only

# Solana RPC Configuration
SOLANA_RPC_ENDPOINT=https://api.mainnet-beta.solana.com
RPC_TIMEOUT_SECS=30
RPC_RETRY_ATTEMPTS=3

# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
METRICS_PORT=9090

# Database Configuration (PostgreSQL)
DATABASE_URL=postgresql://localhost/onchain_beast_personal
DB_MAX_CONNECTIONS=20

# Redis Configuration
REDIS_URL=redis://127.0.0.1:6379
REDIS_POOL_SIZE=10

# API Configuration
RATE_LIMIT_PER_MINUTE=1000
MAX_TRANSACTIONS_PER_REQUEST=100
ANALYSIS_CACHE_TTL_SECONDS=1800

# Logging
LOG_LEVEL=info
LOG_FILE=logs/onchain_beast.log

# Feature Flags
ENABLE_METRICS=true
ENABLE_ANALYSIS=true
ENABLE_CACHING=true
ENABLE_PERSISTENCE=true
EOF

fi

chmod 600 .env

if [ -f config/database.sql ]; then
    echo "ℹ️  config/database.sql already exists; not overwriting."
else
cat > config/database.sql << 'EOF'
-- OnChain Beast Database Setup
-- Run this script to initialize PostgreSQL database

-- Create main transactions table
CREATE TABLE IF NOT EXISTS transactions (
    signature TEXT PRIMARY KEY,
    slot BIGINT NOT NULL,
    block_time BIGINT,
    success BOOLEAN NOT NULL,
    fee BIGINT NOT NULL,
    sol_transfers_count INTEGER NOT NULL DEFAULT 0,
    token_transfers_count INTEGER NOT NULL DEFAULT 0,
    data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    indexed_at TIMESTAMPTZ
);

-- Create wallet_analyses table
CREATE TABLE IF NOT EXISTS wallet_analyses (
    id SERIAL PRIMARY KEY,
    wallet_address TEXT NOT NULL UNIQUE,
    transaction_count INTEGER NOT NULL,
    total_sol_in DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_sol_out DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_token_transferred BIGINT NOT NULL DEFAULT 0,
    risk_level TEXT NOT NULL,
    confidence_score DOUBLE PRECISION NOT NULL,
    fund_flow_graph JSONB,
    pattern_analysis JSONB,
    analyzed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create token_metadata table
CREATE TABLE IF NOT EXISTS token_metadata (
    mint TEXT PRIMARY KEY,
    symbol TEXT NOT NULL,
    name TEXT NOT NULL,
    decimals SMALLINT NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT false,
    fetched_at BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create wallet_relationships table
CREATE TABLE IF NOT EXISTS wallet_relationships (
    id SERIAL PRIMARY KEY,
    from_wallet TEXT NOT NULL,
    to_wallet TEXT NOT NULL,
    sol_amount DOUBLE PRECISION NOT NULL DEFAULT 0,
    token_amount BIGINT NOT NULL DEFAULT 0,
    transaction_count INTEGER NOT NULL DEFAULT 1,
    first_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(from_wallet, to_wallet)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot);
CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time);
CREATE INDEX IF NOT EXISTS idx_wallet_analyses_address ON wallet_analyses(wallet_address);
CREATE INDEX IF NOT EXISTS idx_token_metadata_symbol ON token_metadata(symbol);
CREATE INDEX IF NOT EXISTS idx_relationships_from ON wallet_relationships(from_wallet);
CREATE INDEX IF NOT EXISTS idx_relationships_to ON wallet_relationships(to_wallet);

-- Create views for analysis
CREATE OR REPLACE VIEW v_high_risk_wallets AS
SELECT 
    wallet_address,
    transaction_count,
    total_sol_in,
    total_sol_out,
    risk_level,
    confidence_score,
    analyzed_at
FROM wallet_analyses
WHERE risk_level IN ('High', 'Critical')
ORDER BY confidence_score DESC
LIMIT 100;

GRANT SELECT ON v_high_risk_wallets TO PUBLIC;
EOF

fi

echo "✅ Configuration files created"
echo ""

# Create startup script
if [ -f start.sh ]; then
    echo "ℹ️  start.sh already exists; not overwriting."
else
cat > start.sh << 'EOF'
#!/bin/bash
# Start OnChain Beast application

set -e

PROJECT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$PROJECT_DIR"

echo "🚀 Starting OnChain Beast..."
echo "=============================="
echo ""

# Check if PostgreSQL is running
if ! command -v psql &> /dev/null; then
    echo "❌ PostgreSQL not found. Please install PostgreSQL first:"
    echo "   MacOS: brew install postgresql"
    echo "   Ubuntu: sudo apt-get install postgresql"
    exit 1
fi

# Check if Redis is running
if ! command -v redis-cli &> /dev/null; then
    echo "❌ Redis not found. Please install Redis first:"
    echo "   MacOS: brew install redis"
    echo "   Ubuntu: sudo apt-get install redis-server"
    exit 1
fi

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '#' | xargs)
fi

echo "📊 Checking dependencies..."
echo "  ✓ PostgreSQL"
echo "  ✓ Redis"
echo ""

# Create database if it doesn't exist
echo "🗄️  Setting up database..."
createdb onchain_beast_personal 2>/dev/null || true
psql onchain_beast_personal -f config/database.sql > /dev/null 2>&1

echo "✅ Database ready"
echo ""

# Start the application
echo "⚙️  Starting application server..."
echo "  Listen: http://${SERVER_HOST}:${SERVER_PORT}"
echo "  Metrics: http://${SERVER_HOST}:${METRICS_PORT}/metrics"
echo ""

exec ./target/release/onchain_beast
EOF

fi

chmod +x start.sh

# Create systemd service file (optional, for production use)
if [ -f onchain_beast.service ]; then
    echo "ℹ️  onchain_beast.service already exists; not overwriting."
else
cat > onchain_beast.service << 'EOF'
[Unit]
Description=OnChain Beast - Solana Transaction Analyzer
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=onchain
WorkingDirectory=/opt/onchain_beast
ExecStart=/opt/onchain_beast/target/release/onchain_beast
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
EnvironmentFile=/opt/onchain_beast/.env

[Install]
WantedBy=multi-user.target
EOF

fi

echo "✅ Service file created (optional)"
echo ""

# Create monitoring script
if [ -f monitor.sh ]; then
    echo "ℹ️  monitor.sh already exists; not overwriting."
else
cat > monitor.sh << 'EOF'
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
EOF

fi

chmod +x monitor.sh

echo "✅ Monitoring script created"
echo ""

# Summary
echo "🎉 Setup Complete!"
echo "=================="
echo ""
echo "📋 Next Steps:"
echo ""
echo "1. Install PostgreSQL (if not already installed):"
echo "   MacOS: brew install postgresql && brew services start postgresql"
echo "   Ubuntu: sudo apt-get install postgresql && sudo systemctl start postgresql"
echo ""
echo "2. Install Redis (if not already installed):"
echo "   MacOS: brew install redis && brew services start redis"
echo "   Ubuntu: sudo apt-get install redis-server && sudo systemctl start redis"
echo ""
echo "3. Update .env file with your Solana RPC endpoint (optional)"
echo ""
echo "4. Build the project:"
echo "   cargo build --release"
echo ""
echo "5. Start the application:"
echo "   ./start.sh"
echo ""
echo "6. Monitor the application:"
echo "   ./monitor.sh"
echo ""
echo "7. Access the application:"
echo "   API: http://127.0.0.1:8080"
echo "   Metrics: http://127.0.0.1:9090/metrics"
echo ""
echo "📚 Available Endpoints:"
echo "   POST /api/v1/parse/transaction - Parse transaction"
echo "   GET  /metadata/token/{mint} - Get token metadata"
echo "   POST /transfer/batch-analyze - Batch analyze transfers"
echo "   GET  /analysis/wallet/{address} - Analyze wallet"
echo "   GET  /analysis/stats - Analysis statistics"
echo ""
echo "✅ Ready for deployment!"

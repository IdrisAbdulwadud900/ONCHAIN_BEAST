#!/bin/bash
# Initialize PostgreSQL database for OnChain Beast

set -e

echo "🗄️  Initializing PostgreSQL database for OnChain Beast..."
echo "========================================================"
echo ""

# Get the current user
CURRENT_USER=$(whoami)
echo "Current user: $CURRENT_USER"
echo ""

# Check if PostgreSQL is installed
if ! command -v psql &> /dev/null; then
    echo "❌ PostgreSQL not found. Installing..."
    brew install postgresql
fi

# Check if PostgreSQL is running
echo "Checking PostgreSQL status..."
if ! brew services list | grep -q "postgresql.*started"; then
    echo "Starting PostgreSQL..."
    brew services start postgresql
    sleep 2
fi

# Check if local user role exists
echo "Setting up PostgreSQL user..."
psql -d postgres -c "CREATE USER $CURRENT_USER WITH CREATEDB SUPERUSER;" 2>/dev/null || true

# Create the database
echo "Creating onchain_beast database..."
psql -U "$CURRENT_USER" -c "CREATE DATABASE onchain_beast_personal;" 2>/dev/null || true

# Create the schema file location
mkdir -p config

# Create database schema
echo "Initializing schema..."
cat > /tmp/init_schema.sql << 'EOF'
-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    signature TEXT PRIMARY KEY,
    slot BIGINT NOT NULL,
    block_time BIGINT,
    success BOOLEAN NOT NULL,
    fee BIGINT NOT NULL,
    sol_transfers_count INTEGER NOT NULL DEFAULT 0,
    token_transfers_count INTEGER NOT NULL DEFAULT 0,
    data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot);
CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time);

-- Wallet analyses table
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
    analyzed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Relationships table
CREATE TABLE IF NOT EXISTS relationships (
    from_wallet TEXT NOT NULL,
    to_wallet TEXT NOT NULL,
    transaction_count INTEGER NOT NULL DEFAULT 1,
    total_amount BIGINT NOT NULL DEFAULT 0,
    relationship_type TEXT NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (from_wallet, to_wallet, relationship_type)
);

-- Transfer metadata table
CREATE TABLE IF NOT EXISTS transfer_metadata (
    signature TEXT NOT NULL,
    wallet_address TEXT NOT NULL,
    transfer_type TEXT NOT NULL,
    amount BIGINT NOT NULL,
    mint_address TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (signature, wallet_address, transfer_type)
);

-- Pattern results table
CREATE TABLE IF NOT EXISTS pattern_results (
    id SERIAL PRIMARY KEY,
    wallet_address TEXT NOT NULL,
    pattern_type TEXT NOT NULL,
    confidence DOUBLE PRECISION NOT NULL,
    details JSONB,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO postgres;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO postgres;
EOF

psql -U "$CURRENT_USER" -d onchain_beast_personal -f /tmp/init_schema.sql 2>&1 || true
rm /tmp/init_schema.sql

echo ""
echo "✅ Database initialization complete!"
echo ""
echo "Database Details:"
echo "  Name: onchain_beast_personal"
echo "  User: $CURRENT_USER"
echo "  Port: 5432"
echo "  Connection String: postgresql://$CURRENT_USER@localhost/onchain_beast_personal"
echo ""

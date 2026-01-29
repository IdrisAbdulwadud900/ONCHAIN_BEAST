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

-- OnChain Beast Database Schema
-- PostgreSQL 14+

-- Wallets table: Core wallet information
CREATE TABLE IF NOT EXISTS wallets (
    address VARCHAR(44) PRIMARY KEY,
    balance BIGINT NOT NULL DEFAULT 0,
    owner VARCHAR(44),
    executable BOOLEAN NOT NULL DEFAULT FALSE,
    rent_epoch BIGINT,
    first_seen TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    risk_score FLOAT,
    is_exchange BOOLEAN DEFAULT FALSE,
    is_mixer BOOLEAN DEFAULT FALSE,
    metadata JSONB DEFAULT '{}'::jsonb
);

CREATE INDEX idx_wallets_balance ON wallets(balance DESC);
CREATE INDEX idx_wallets_risk ON wallets(risk_score DESC) WHERE risk_score IS NOT NULL;
CREATE INDEX idx_wallets_updated ON wallets(last_updated DESC);
CREATE INDEX idx_wallets_exchange ON wallets(is_exchange) WHERE is_exchange = TRUE;

-- Transactions table: All blockchain transactions
CREATE TABLE IF NOT EXISTS transactions (
    signature VARCHAR(88) PRIMARY KEY,
    slot BIGINT NOT NULL,
    block_time TIMESTAMP WITH TIME ZONE,
    fee BIGINT DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'success',
    error TEXT,
    from_addresses TEXT[],  -- Array of involved addresses
    to_addresses TEXT[],
    program_ids TEXT[],     -- Programs called
    raw_data JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tx_slot ON transactions(slot DESC);
CREATE INDEX idx_tx_time ON transactions(block_time DESC) WHERE block_time IS NOT NULL;
CREATE INDEX idx_tx_from ON transactions USING GIN(from_addresses);
CREATE INDEX idx_tx_to ON transactions USING GIN(to_addresses);
CREATE INDEX idx_tx_programs ON transactions USING GIN(program_ids);

-- SOL transfers table: Native SOL movements
CREATE TABLE IF NOT EXISTS sol_transfers (
    id BIGSERIAL PRIMARY KEY,
    signature VARCHAR(88) NOT NULL REFERENCES transactions(signature) ON DELETE CASCADE,
    from_address VARCHAR(44) NOT NULL,
    to_address VARCHAR(44) NOT NULL,
    amount BIGINT NOT NULL,
    transfer_index INT NOT NULL,  -- Order within transaction
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sol_sig ON sol_transfers(signature);
CREATE INDEX idx_sol_from ON sol_transfers(from_address);
CREATE INDEX idx_sol_to ON sol_transfers(to_address);
CREATE INDEX idx_sol_amount ON sol_transfers(amount DESC);
CREATE INDEX idx_sol_time ON sol_transfers(created_at DESC);

-- Token transfers table: SPL token movements
CREATE TABLE IF NOT EXISTS token_transfers (
    id BIGSERIAL PRIMARY KEY,
    signature VARCHAR(88) NOT NULL REFERENCES transactions(signature) ON DELETE CASCADE,
    mint VARCHAR(44) NOT NULL,  -- Token mint address
    from_address VARCHAR(44) NOT NULL,
    to_address VARCHAR(44) NOT NULL,
    amount BIGINT NOT NULL,
    decimals SMALLINT NOT NULL,
    authority VARCHAR(44),
    transfer_index INT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_token_sig ON token_transfers(signature);
CREATE INDEX idx_token_mint ON token_transfers(mint);
CREATE INDEX idx_token_from ON token_transfers(from_address);
CREATE INDEX idx_token_to ON token_transfers(to_address);
CREATE INDEX idx_token_amount ON token_transfers(amount DESC);
CREATE INDEX idx_token_time ON token_transfers(created_at DESC);

-- Token metadata table: Known token information
CREATE TABLE IF NOT EXISTS tokens (
    mint VARCHAR(44) PRIMARY KEY,
    symbol VARCHAR(20),
    name VARCHAR(100),
    decimals SMALLINT NOT NULL,
    supply BIGINT,
    is_nft BOOLEAN DEFAULT FALSE,
    is_suspicious BOOLEAN DEFAULT FALSE,
    liquidity_usd NUMERIC(20, 2),
    holder_count INT,
    metadata JSONB DEFAULT '{}'::jsonb,
    first_seen TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_updated TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tokens_symbol ON tokens(symbol);
CREATE INDEX idx_tokens_suspicious ON tokens(is_suspicious) WHERE is_suspicious = TRUE;
CREATE INDEX idx_tokens_liquidity ON tokens(liquidity_usd DESC NULLS LAST);

-- Wallet relationships table: Graph edges
CREATE TABLE IF NOT EXISTS wallet_relationships (
    id BIGSERIAL PRIMARY KEY,
    from_wallet VARCHAR(44) NOT NULL,
    to_wallet VARCHAR(44) NOT NULL,
    total_sol BIGINT NOT NULL DEFAULT 0,
    total_transactions INT NOT NULL DEFAULT 0,
    first_interaction TIMESTAMP WITH TIME ZONE NOT NULL,
    last_interaction TIMESTAMP WITH TIME ZONE NOT NULL,
    relationship_strength FLOAT,  -- 0.0 to 1.0
    UNIQUE(from_wallet, to_wallet)
);

CREATE INDEX idx_rel_from ON wallet_relationships(from_wallet);
CREATE INDEX idx_rel_to ON wallet_relationships(to_wallet);
CREATE INDEX idx_rel_strength ON wallet_relationships(relationship_strength DESC);
CREATE INDEX idx_rel_last ON wallet_relationships(last_interaction DESC);

-- Analysis results cache table
CREATE TABLE IF NOT EXISTS analysis_cache (
    cache_key VARCHAR(128) PRIMARY KEY,
    cache_type VARCHAR(50) NOT NULL,  -- 'wallet_risk', 'side_wallets', etc.
    result JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL
);

CREATE INDEX idx_cache_type ON analysis_cache(cache_type);
CREATE INDEX idx_cache_expires ON analysis_cache(expires_at);

-- Pattern detection results
CREATE TABLE IF NOT EXISTS detected_patterns (
    id BIGSERIAL PRIMARY KEY,
    wallet_address VARCHAR(44) NOT NULL,
    pattern_type VARCHAR(50) NOT NULL,  -- 'wash_trading', 'pump_dump', etc.
    confidence FLOAT NOT NULL,
    details JSONB NOT NULL,
    detected_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pattern_wallet ON detected_patterns(wallet_address);
CREATE INDEX idx_pattern_type ON detected_patterns(pattern_type);
CREATE INDEX idx_pattern_confidence ON detected_patterns(confidence DESC);
CREATE INDEX idx_pattern_time ON detected_patterns(detected_at DESC);

-- RPC call log for monitoring
CREATE TABLE IF NOT EXISTS rpc_calls (
    id BIGSERIAL PRIMARY KEY,
    method VARCHAR(100) NOT NULL,
    params JSONB,
    success BOOLEAN NOT NULL,
    duration_ms INT NOT NULL,
    error TEXT,
    called_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_rpc_method ON rpc_calls(method);
CREATE INDEX idx_rpc_success ON rpc_calls(success);
CREATE INDEX idx_rpc_time ON rpc_calls(called_at DESC);

-- Create a function to auto-update last_updated timestamp
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.last_updated = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply the trigger to wallets table
CREATE TRIGGER wallets_updated_at
    BEFORE UPDATE ON wallets
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at();

-- Create a function to clean expired cache
CREATE OR REPLACE FUNCTION clean_expired_cache()
RETURNS void AS $$
BEGIN
    DELETE FROM analysis_cache WHERE expires_at < NOW();
END;
$$ LANGUAGE plpgsql;

-- Materialized view for wallet statistics
CREATE MATERIALIZED VIEW IF NOT EXISTS wallet_stats AS
SELECT 
    w.address,
    w.balance,
    w.risk_score,
    COUNT(DISTINCT st.signature) as sol_tx_count,
    COUNT(DISTINCT tt.signature) as token_tx_count,
    COALESCE(SUM(st.amount), 0) as total_sol_sent,
    COUNT(DISTINCT wr.to_wallet) as unique_receivers,
    COUNT(DISTINCT tt.mint) as unique_tokens
FROM wallets w
LEFT JOIN sol_transfers st ON st.from_address = w.address
LEFT JOIN token_transfers tt ON tt.from_address = w.address
LEFT JOIN wallet_relationships wr ON wr.from_wallet = w.address
GROUP BY w.address, w.balance, w.risk_score;

CREATE INDEX idx_wallet_stats_address ON wallet_stats(address);
CREATE INDEX idx_wallet_stats_tx ON wallet_stats(sol_tx_count + token_tx_count DESC);

-- Comments for documentation
COMMENT ON TABLE wallets IS 'Core wallet information from Solana blockchain';
COMMENT ON TABLE transactions IS 'All processed transactions';
COMMENT ON TABLE sol_transfers IS 'Native SOL transfers extracted from transactions';
COMMENT ON TABLE token_transfers IS 'SPL token transfers extracted from transactions';
COMMENT ON TABLE tokens IS 'Known SPL token metadata';
COMMENT ON TABLE wallet_relationships IS 'Graph edges representing wallet interactions';
COMMENT ON TABLE analysis_cache IS 'Cached analysis results with TTL';
COMMENT ON TABLE detected_patterns IS 'Suspicious patterns detected by analysis engine';

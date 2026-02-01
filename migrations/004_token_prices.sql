-- Token Prices Table
-- Stores historical price data for tokens
CREATE TABLE IF NOT EXISTS token_prices (
    id BIGSERIAL PRIMARY KEY,
    token_mint VARCHAR(44) NOT NULL,
    price_usd DOUBLE PRECISION NOT NULL,
    timestamp_utc BIGINT NOT NULL,
    source VARCHAR(20) NOT NULL DEFAULT 'jupiter',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_token_prices_mint_timestamp ON token_prices(token_mint, timestamp_utc DESC);
CREATE INDEX IF NOT EXISTS idx_token_prices_timestamp ON token_prices(timestamp_utc DESC);
CREATE INDEX IF NOT EXISTS idx_token_prices_source ON token_prices(source);

-- Add USD columns to swap_events table
ALTER TABLE swap_events 
ADD COLUMN IF NOT EXISTS price_usd_in DOUBLE PRECISION,
ADD COLUMN IF NOT EXISTS price_usd_out DOUBLE PRECISION,
ADD COLUMN IF NOT EXISTS value_usd_in DOUBLE PRECISION,
ADD COLUMN IF NOT EXISTS value_usd_out DOUBLE PRECISION,
ADD COLUMN IF NOT EXISTS pnl_usd DOUBLE PRECISION;

-- Index for USD value queries
CREATE INDEX IF NOT EXISTS idx_swap_events_value_usd ON swap_events(value_usd_out DESC);
CREATE INDEX IF NOT EXISTS idx_swap_events_pnl ON swap_events(pnl_usd DESC) WHERE pnl_usd IS NOT NULL;

-- Comments
COMMENT ON TABLE token_prices IS 'Historical token price data from Jupiter and other sources';
COMMENT ON COLUMN token_prices.token_mint IS 'Token mint address (Solana public key)';
COMMENT ON COLUMN token_prices.price_usd IS 'Price in USD';
COMMENT ON COLUMN token_prices.timestamp_utc IS 'Unix timestamp when price was recorded';
COMMENT ON COLUMN token_prices.source IS 'Price data source (jupiter, birdeye, etc)';

COMMENT ON COLUMN swap_events.price_usd_in IS 'Price of input token at swap time (USD)';
COMMENT ON COLUMN swap_events.price_usd_out IS 'Price of output token at swap time (USD)';
COMMENT ON COLUMN swap_events.value_usd_in IS 'Total USD value of input tokens';
COMMENT ON COLUMN swap_events.value_usd_out IS 'Total USD value of output tokens';
COMMENT ON COLUMN swap_events.pnl_usd IS 'Profit/Loss in USD (value_out - value_in)';

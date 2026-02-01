/// DEX Swap Types
/// Common types for swap extraction and analysis
use serde::{Deserialize, Serialize};

/// Represents a decoded swap from any DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEvent {
    /// Transaction signature
    pub signature: String,
    /// Event index within transaction (for multiple swaps)
    pub event_index: u32,
    /// Block slot
    pub slot: u64,
    /// Block timestamp
    pub block_time: u64,
    /// Wallet that initiated the swap
    pub wallet: String,
    /// DEX program ID
    pub dex_program: String,
    /// DEX name (Raydium, Orca, Jupiter, etc.)
    pub dex_name: String,
    /// Input token mint address
    pub token_in: String,
    /// Amount of input token (raw, not decimal-adjusted)
    pub amount_in: u64,
    /// Output token mint address
    pub token_out: String,
    /// Amount of output token (raw, not decimal-adjusted)
    pub amount_out: u64,
    /// Effective price (amount_out / amount_in, adjusted for decimals)
    pub price: f64,
    /// Minimum amount out (slippage protection)
    pub min_amount_out: Option<u64>,
    /// Pool/liquidity account used
    pub pool_address: Option<String>,
}

/// DEX program identifiers on Solana
pub struct DexPrograms;

impl DexPrograms {
    // Raydium AMM
    pub const RAYDIUM_V4: &'static str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    pub const RAYDIUM_STABLE: &'static str = "5quBtoiQqxF9Jv6KYKctB59NT3gtJD2Y65kdnB1Uev3h";
    
    // Orca
    pub const ORCA_WHIRLPOOL: &'static str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
    pub const ORCA_V1: &'static str = "9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP";
    pub const ORCA_V2: &'static str = "DjVE6JNiYqPL2QXyCUUh8rNjHrbz9hXHNYt99MQ59qw1";
    
    // Jupiter Aggregator
    pub const JUPITER_V4: &'static str = "JUP4Fb2cqiRUcaTHdrPC8h2gNsA2ETXiPDD33WcGuJB";
    pub const JUPITER_V6: &'static str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";
    
    // Meteora
    pub const METEORA_POOLS: &'static str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
    
    /// Check if a program ID is a known DEX
    pub fn is_dex_program(program_id: &str) -> bool {
        matches!(
            program_id,
            Self::RAYDIUM_V4
                | Self::RAYDIUM_STABLE
                | Self::ORCA_WHIRLPOOL
                | Self::ORCA_V1
                | Self::ORCA_V2
                | Self::JUPITER_V4
                | Self::JUPITER_V6
                | Self::METEORA_POOLS
        )
    }
    
    /// Get DEX name from program ID
    pub fn get_dex_name(program_id: &str) -> Option<&'static str> {
        match program_id {
            Self::RAYDIUM_V4 => Some("Raydium V4"),
            Self::RAYDIUM_STABLE => Some("Raydium Stable"),
            Self::ORCA_WHIRLPOOL => Some("Orca Whirlpool"),
            Self::ORCA_V1 => Some("Orca V1"),
            Self::ORCA_V2 => Some("Orca V2"),
            Self::JUPITER_V4 => Some("Jupiter V4"),
            Self::JUPITER_V6 => Some("Jupiter V6"),
            Self::METEORA_POOLS => Some("Meteora"),
            _ => None,
        }
    }
}

/// Swap direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapDirection {
    /// Buy token (SOL/USDC → Token)
    Buy,
    /// Sell token (Token → SOL/USDC)
    Sell,
}

/// Common quote tokens on Solana
pub struct QuoteTokens;

impl QuoteTokens {
    pub const SOL: &'static str = "So11111111111111111111111111111111111111112"; // Wrapped SOL
    pub const USDC: &'static str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    pub const USDT: &'static str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
    
    /// Check if a token is a common quote token
    pub fn is_quote_token(mint: &str) -> bool {
        matches!(mint, Self::SOL | Self::USDC | Self::USDT)
    }
    
    /// Determine swap direction based on token mints
    pub fn get_swap_direction(token_in: &str, token_out: &str) -> SwapDirection {
        if Self::is_quote_token(token_in) {
            SwapDirection::Buy
        } else if Self::is_quote_token(token_out) {
            SwapDirection::Sell
        } else {
            // Token-to-token swap, default to buy
            SwapDirection::Buy
        }
    }
}

/// Swap instruction discriminators (first 8 bytes of instruction data)
#[derive(Debug, Clone, Copy)]
pub struct InstructionDiscriminators;

impl InstructionDiscriminators {
    // Raydium V4 AMM instructions
    pub const RAYDIUM_SWAP: u8 = 9;
    pub const RAYDIUM_SWAP_BASE_IN: u8 = 9;
    pub const RAYDIUM_SWAP_BASE_OUT: u8 = 10;
    
    // Add more as we decode other DEXes
}

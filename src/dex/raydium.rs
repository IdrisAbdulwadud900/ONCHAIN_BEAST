/// Raydium DEX Decoder
/// Extracts swap events from Raydium V4 AMM transactions
use super::types::{DexPrograms, SwapEvent};
use crate::core::errors::{BeastError, BeastResult};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Raydium swap instruction layout
/// Based on Raydium AMM program v4
#[derive(Debug)]
pub struct RaydiumSwapInstruction {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
}

pub struct RaydiumDecoder;

impl RaydiumDecoder {
    /// Decode Raydium swap from instruction data
    pub fn decode_swap(
        signature: &str,
        event_index: u32,
        slot: u64,
        block_time: u64,
        instruction_data: &[u8],
        accounts: &[String],
        wallet: &str,
    ) -> BeastResult<Option<SwapEvent>> {
        // Raydium V4 swap instruction format:
        // [0]: discriminator (9 for swap base in, 10 for swap base out)
        // [1-8]: amount_in (u64 little-endian)
        // [9-16]: minimum_amount_out (u64 little-endian)
        
        if instruction_data.len() < 17 {
            return Ok(None); // Not a valid swap instruction
        }
        
        let discriminator = instruction_data[0];
        if discriminator != 9 && discriminator != 10 {
            return Ok(None); // Not a swap instruction
        }
        
        // Parse amounts
        let amount_in = u64::from_le_bytes(
            instruction_data[1..9]
                .try_into()
                .map_err(|_| BeastError::ParseError("Invalid amount_in".to_string()))?,
        );
        
        let minimum_amount_out = u64::from_le_bytes(
            instruction_data[9..17]
                .try_into()
                .map_err(|_| BeastError::ParseError("Invalid minimum_amount_out".to_string()))?,
        );
        
        // Raydium V4 account layout (typical):
        // 0: Token program
        // 1: AMM ID
        // 2: AMM authority
        // 3: AMM open orders
        // 4: AMM target orders
        // 5: AMM coin vault (base token)
        // 6: AMM pc vault (quote token)
        // 7: Serum market program
        // 8: Serum market
        // 9: Serum bids
        // 10: Serum asks
        // 11: Serum event queue
        // 12: Serum coin vault
        // 13: Serum pc vault
        // 14: Serum vault signer
        // 15: User source token account
        // 16: User destination token account
        // 17: User owner (wallet)
        
        if accounts.len() < 17 {
            return Ok(None); // Not enough accounts
        }
        
        // Extract token accounts (simplified - need token mint lookups for full accuracy)
        let pool_address = accounts.get(1).cloned();
        
        // For now, we'll extract basic swap info
        // In production, we'd need to:
        // 1. Query token account info to get mint addresses
        // 2. Track actual amounts transferred via token program logs
        // 3. Calculate effective price
        
        Ok(Some(SwapEvent {
            signature: signature.to_string(),
            event_index,
            slot,
            block_time,
            wallet: wallet.to_string(),
            dex_program: DexPrograms::RAYDIUM_V4.to_string(),
            dex_name: "Raydium V4".to_string(),
            token_in: "PLACEHOLDER_IN".to_string(), // Need token account lookup
            amount_in,
            token_out: "PLACEHOLDER_OUT".to_string(), // Need token account lookup
            amount_out: 0, // Need to extract from logs or post-balance changes
            price: 0.0, // Calculate after getting real amounts
            min_amount_out: Some(minimum_amount_out),
            pool_address,
        }))
    }
    
    /// Extract swap from inner instructions (for aggregators like Jupiter)
    pub fn decode_from_inner_instruction(
        signature: &str,
        event_index: u32,
        slot: u64,
        block_time: u64,
        instruction_data: &[u8],
        accounts: &[String],
        wallet: &str,
    ) -> BeastResult<Option<SwapEvent>> {
        // Same logic but mark as nested swap
        Self::decode_swap(
            signature,
            event_index,
            slot,
            block_time,
            instruction_data,
            accounts,
            wallet,
        )
    }
    
    /// Extract actual swap amounts from token transfer logs
    /// This is more accurate than parsing instruction data
    pub fn extract_from_token_transfers(
        signature: &str,
        event_index: u32,
        slot: u64,
        block_time: u64,
        wallet: &str,
        token_in_mint: &str,
        token_in_amount: u64,
        token_out_mint: &str,
        token_out_amount: u64,
        pool_address: Option<String>,
    ) -> SwapEvent {
        let price = if token_in_amount > 0 {
            token_out_amount as f64 / token_in_amount as f64
        } else {
            0.0
        };
        
        SwapEvent {
            signature: signature.to_string(),
            event_index,
            slot,
            block_time,
            wallet: wallet.to_string(),
            dex_program: DexPrograms::RAYDIUM_V4.to_string(),
            dex_name: "Raydium V4".to_string(),
            token_in: token_in_mint.to_string(),
            amount_in: token_in_amount,
            token_out: token_out_mint.to_string(),
            amount_out: token_out_amount,
            price,
            min_amount_out: None,
            pool_address,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_raydium_swap_instruction() {
        // Example Raydium swap instruction data
        let mut instruction_data = vec![9u8]; // Swap discriminator
        instruction_data.extend_from_slice(&1000000u64.to_le_bytes()); // amount_in
        instruction_data.extend_from_slice(&900000u64.to_le_bytes()); // min_amount_out
        
        let accounts: Vec<String> = (0..18)
            .map(|i| format!("Account{}", i))
            .collect();
        
        let result = RaydiumDecoder::decode_swap(
            "test_sig",
            0,
            123456,
            1706745600,
            &instruction_data,
            &accounts,
            "wallet123",
        );
        
        assert!(result.is_ok());
        let swap = result.unwrap();
        assert!(swap.is_some());
        
        let swap = swap.unwrap();
        assert_eq!(swap.amount_in, 1000000);
        assert_eq!(swap.min_amount_out, Some(900000));
        assert_eq!(swap.dex_name, "Raydium V4");
    }
}

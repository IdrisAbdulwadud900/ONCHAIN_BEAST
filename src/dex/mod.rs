/// DEX Decoder Module
/// Extracts swap events from various Solana DEX programs
pub mod raydium;
pub mod types;

pub use raydium::RaydiumDecoder;
pub use types::{DexPrograms, QuoteTokens, SwapDirection, SwapEvent};

use crate::core::errors::BeastResult;
use crate::core::EnhancedTransaction;

/// Main DEX decoder that routes to specific decoders
pub struct DexDecoder;

impl DexDecoder {
    /// Extract all swap events from a transaction
    pub fn extract_swaps(tx: &EnhancedTransaction) -> BeastResult<Vec<SwapEvent>> {
        let mut swaps = Vec::new();
        let mut event_index = 0u32;
        
        // Get wallet (first signer or authority)
        let wallet = tx
            .sol_transfers
            .first()
            .map(|t| t.from.clone())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Scan transaction for DEX program invocations
        // Note: In a real implementation, we'd parse the transaction's instructions
        // For now, this is a placeholder that will be enhanced when we integrate
        // with the actual transaction parsing
        
        // TODO: Parse instructions from EnhancedTransaction
        // TODO: Check each instruction's program_id
        // TODO: Route to appropriate decoder (Raydium, Orca, etc.)
        // TODO: Extract swaps from inner instructions for aggregators
        
        Ok(swaps)
    }
    
    /// Infer swaps from token transfer patterns
    /// This is a fallback method when instruction parsing is incomplete
    pub fn infer_from_transfers(tx: &EnhancedTransaction) -> BeastResult<Vec<SwapEvent>> {
        let mut swaps = Vec::new();
        
        // Look for patterns: token_in transfer out + token_out transfer in
        // This catches swaps even when we can't parse instruction data
        
        // Group transfers by wallet
        let mut wallet_transfers: std::collections::HashMap<String, Vec<&crate::core::TokenTransfer>> = std::collections::HashMap::new();
        
        for transfer in &tx.token_transfers {
            if let Some(from) = &transfer.from_owner {
                wallet_transfers.entry(from.clone()).or_default().push(transfer);
            }
            if let Some(to) = &transfer.to_owner {
                wallet_transfers.entry(to.clone()).or_default().push(transfer);
            }
        }
        
        // For each wallet, find outbound + inbound transfers in same tx (likely a swap)
        for (wallet, transfers) in wallet_transfers {
            let outbound: Vec<_> = transfers.iter()
                .filter(|t| t.from_owner.as_ref() == Some(&wallet))
                .collect();
            let inbound: Vec<_> = transfers.iter()
                .filter(|t| t.to_owner.as_ref() == Some(&wallet))
                .collect();
            
            // If wallet has both out and in, likely a swap
            if !outbound.is_empty() && !inbound.is_empty() {
                // Create swap event from transfer pattern
                // This is simplified - in production we'd match specific pairs
                for (i, (out, in_t)) in outbound.iter().zip(inbound.iter()).enumerate() {
                    let swap = SwapEvent {
                        signature: tx.signature.clone(),
                        event_index: i as u32,
                        slot: tx.slot,
                        block_time: tx.block_time.unwrap_or(0),
                        wallet: wallet.clone(),
                        dex_program: "INFERRED".to_string(),
                        dex_name: "Inferred from transfers".to_string(),
                        token_in: out.mint.clone(),
                        amount_in: out.amount,
                        token_out: in_t.mint.clone(),
                        amount_out: in_t.amount,
                        price: if out.amount > 0 {
                            in_t.amount as f64 / out.amount as f64
                        } else {
                            0.0
                        },
                        min_amount_out: None,
                        pool_address: None,
                    };
                    swaps.push(swap);
                }
            }
        }
        
        Ok(swaps)
    }
}

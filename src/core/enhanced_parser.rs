use crate::core::errors::{BeastError, Result};
/// Enhanced Transaction Parser - Extracts SOL and Token Transfers
/// Parses Solana transactions to extract fund flows and transfer details
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<u64>,
    pub fee: u64,
    pub success: bool,
    pub error: Option<String>,

    // Account analysis
    pub accounts: Vec<String>,
    pub signers: Vec<String>,
    pub writable_accounts: Vec<String>,

    // Transfer extraction
    pub sol_transfers: Vec<SolTransfer>,
    pub token_transfers: Vec<TokenTransfer>,

    // Balance changes
    pub balance_changes: Vec<BalanceChange>,

    // Program interaction
    pub programs_called: Vec<String>,
    pub program_names: Vec<String>,

    // Classification
    pub tx_type: TransactionType,
    pub is_versioned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolTransfer {
    pub from: String,
    pub to: String,
    pub amount_lamports: u64,
    pub amount_sol: f64,
    pub instruction_index: usize,
    pub transfer_type: String, // "system", "inner", "balance_change"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    pub mint: String,
    pub from_token_account: String,
    pub to_token_account: String,
    pub from_owner: Option<String>,
    pub to_owner: Option<String>,
    pub amount: u64,
    pub decimals: u8,
    pub amount_ui: f64,
    pub authority: String,
    pub instruction_index: usize,
    pub transfer_type: String, // "transfer", "transferChecked", "inner"

    // Token metadata (enriched fields)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_symbol: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceChange {
    pub account: String,
    pub pre_balance: u64,
    pub post_balance: u64,
    pub change_lamports: i64,
    pub change_sol: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionType {
    SystemTransfer,
    TokenTransfer,
    TokenSwap,
    TokenMint,
    NFTTrade,
    DeFiInteraction,
    ProgramCall,
    Unknown,
}

// ============================================================================
// ENHANCED TRANSACTION PARSER
// ============================================================================

pub struct EnhancedTransactionParser {
    // Known program IDs
    system_program: String,
    token_program: String,
    token_2022_program: String,
    associated_token_program: String,

    // DEX programs
    raydium_v4: String,
    orca_whirlpool: String,
    jupiter_v6: String,

    // NFT programs
    metaplex: String,
    magic_eden: String,
}

impl EnhancedTransactionParser {
    pub fn new() -> Self {
        EnhancedTransactionParser {
            system_program: "11111111111111111111111111111111".to_string(),
            token_program: "TokenkegQfeZyiNwAJbPVwwQQfKP3zHqy5RaCZ1NsqKFP".to_string(),
            token_2022_program: "TokenzQdBNbJPPzh6txJjTpWp8QJKhdfuqJW65PfQG".to_string(),
            associated_token_program: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
            raydium_v4: "675kPX9MHTjS2zt1qfmKe2LdPsyAtg5w6qcCX6qX8W8S".to_string(),
            orca_whirlpool: "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc".to_string(),
            jupiter_v6: "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4".to_string(),
            metaplex: "metaqbxxUerdq28cj1RbAWVQGDiVQB5d5owY8c4DUr".to_string(),
            magic_eden: "M2mx93ekt1fmXSVkTrUL9xVFHkmME8HTUi5Cyc5aF7K".to_string(),
        }
    }

    /// Parse a full transaction from raw RPC data
    pub fn parse(
        &self,
        raw_data: &serde_json::Value,
        signature: String,
    ) -> Result<EnhancedTransaction> {
        let slot = raw_data.get("slot").and_then(|s| s.as_u64()).unwrap_or(0);

        let block_time = raw_data
            .get("blockTime")
            .or_else(|| raw_data.get("block_time"))
            .and_then(|t| t.as_u64());

        // Extract metadata
        let meta = raw_data
            .get("meta")
            .ok_or_else(|| BeastError::RpcError("No meta field in transaction".to_string()))?;

        let fee = meta.get("fee").and_then(|f| f.as_u64()).unwrap_or(0);

        let error = meta
            .get("err")
            .filter(|e| !e.is_null())
            .map(|e| format!("{:?}", e));

        let success = error.is_none();

        // Extract transaction data
        let transaction = raw_data
            .get("transaction")
            .ok_or_else(|| BeastError::RpcError("No transaction field".to_string()))?;

        let message = transaction
            .get("message")
            .ok_or_else(|| BeastError::RpcError("No message field".to_string()))?;

        // Extract accounts
        let accounts = self.extract_accounts(message)?;
        let signers = self.extract_signers(message, &accounts)?;
        let writable_accounts = self.extract_writable_accounts(message, &accounts)?;

        // Extract balances for balance change detection
        let pre_balances = self.extract_balances(meta.get("preBalances"));
        let post_balances = self.extract_balances(meta.get("postBalances"));

        // Calculate balance changes
        let balance_changes =
            self.calculate_balance_changes(&accounts, &pre_balances, &post_balances);

        // Parse instructions
        let instructions_vec: Vec<serde_json::Value> = message
            .get("instructions")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();
        let instructions = instructions_vec.as_slice();

        // Extract SOL transfers from instructions and balance changes
        let mut sol_transfers =
            self.extract_sol_transfers_from_instructions(instructions, &accounts);

        // Add SOL transfers from balance changes (catches inner instructions)
        sol_transfers.extend(self.extract_sol_transfers_from_balances(&balance_changes));

        // Extract token transfers from parsed instructions
        let token_transfers = self.extract_token_transfers(instructions, &accounts, meta)?;

        // Extract program IDs
        let programs_called = self.extract_program_ids(instructions, &accounts);
        let program_names = programs_called
            .iter()
            .map(|id| self.get_program_name(id))
            .collect();

        // Determine transaction type
        let tx_type =
            self.determine_transaction_type(&programs_called, &sol_transfers, &token_transfers);

        // Check if versioned
        let is_versioned =
            message.get("version").is_some() || message.get("addressTableLookups").is_some();

        Ok(EnhancedTransaction {
            signature,
            slot,
            block_time,
            fee,
            success,
            error,
            accounts,
            signers,
            writable_accounts,
            sol_transfers,
            token_transfers,
            balance_changes,
            programs_called,
            program_names,
            tx_type,
            is_versioned,
        })
    }

    // ========================================================================
    // ACCOUNT EXTRACTION
    // ========================================================================

    fn extract_accounts(&self, message: &serde_json::Value) -> Result<Vec<String>> {
        let keys = message
            .get("accountKeys")
            .and_then(|k| k.as_array())
            .ok_or_else(|| BeastError::RpcError("No accountKeys in message".to_string()))?;

        Ok(keys
            .iter()
            .filter_map(|k| {
                // Handle both string format and object format
                k.as_str()
                    .or_else(|| k.get("pubkey").and_then(|p| p.as_str()))
                    .map(|s| s.to_string())
            })
            .collect())
    }

    fn extract_signers(
        &self,
        message: &serde_json::Value,
        accounts: &[String],
    ) -> Result<Vec<String>> {
        let num_required = message
            .get("header")
            .and_then(|h| h.get("numRequiredSignatures"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as usize;

        Ok(accounts.iter().take(num_required).cloned().collect())
    }

    fn extract_writable_accounts(
        &self,
        message: &serde_json::Value,
        accounts: &[String],
    ) -> Result<Vec<String>> {
        let header = message.get("header");

        let num_required = header
            .and_then(|h| h.get("numRequiredSignatures"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as usize;

        let num_readonly_signed = header
            .and_then(|h| h.get("numReadonlySignedAccounts"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as usize;

        let num_readonly_unsigned = header
            .and_then(|h| h.get("numReadonlyUnsignedAccounts"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as usize;

        let writable_signed = num_required.saturating_sub(num_readonly_signed);
        let total_accounts = accounts.len();
        let writable_unsigned = total_accounts.saturating_sub(num_required + num_readonly_unsigned);

        let mut writable = Vec::new();

        // Add writable signers
        writable.extend(accounts.iter().take(writable_signed).cloned());

        // Add writable non-signers
        if num_required < total_accounts {
            let start_idx = num_required;
            let end_idx = (num_required + writable_unsigned).min(total_accounts);
            writable.extend(accounts[start_idx..end_idx].iter().cloned());
        }

        Ok(writable)
    }

    // ========================================================================
    // BALANCE EXTRACTION
    // ========================================================================

    fn extract_balances(&self, balances_json: Option<&serde_json::Value>) -> Vec<u64> {
        balances_json
            .and_then(|b| b.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
            .unwrap_or_default()
    }

    fn calculate_balance_changes(
        &self,
        accounts: &[String],
        pre_balances: &[u64],
        post_balances: &[u64],
    ) -> Vec<BalanceChange> {
        accounts
            .iter()
            .enumerate()
            .filter_map(|(idx, account)| {
                let pre = pre_balances.get(idx).copied()?;
                let post = post_balances.get(idx).copied()?;

                if pre == post {
                    return None; // No change
                }

                let change_lamports = post as i64 - pre as i64;
                let change_sol = change_lamports as f64 / 1_000_000_000.0;

                Some(BalanceChange {
                    account: account.clone(),
                    pre_balance: pre,
                    post_balance: post,
                    change_lamports,
                    change_sol,
                })
            })
            .collect()
    }

    // ========================================================================
    // SOL TRANSFER EXTRACTION
    // ========================================================================

    fn instruction_program_id(
        &self,
        instr: &serde_json::Value,
        accounts: &[String],
    ) -> Option<String> {
        // `getTransaction` with `encoding=jsonParsed` uses `programId`.
        // Other encodings use `programIdIndex`.
        if let Some(program_idx) = instr.get("programIdIndex").and_then(|p| p.as_u64()) {
            return accounts.get(program_idx as usize).cloned();
        }

        instr
            .get("programId")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string())
    }

    fn extract_sol_transfers_from_instructions(
        &self,
        instructions: &[serde_json::Value],
        accounts: &[String],
    ) -> Vec<SolTransfer> {
        instructions
            .iter()
            .enumerate()
            .filter_map(|(idx, instr)| {
                // Get program ID (supports both programIdIndex and jsonParsed programId)
                let program_id = self.instruction_program_id(instr, accounts)?;

                // Only process system program instructions
                if program_id != self.system_program {
                    return None;
                }

                // Check if it's a parsed instruction
                if let Some(parsed) = instr.get("parsed") {
                    return self.extract_sol_transfer_from_parsed(parsed, idx);
                }

                None
            })
            .collect()
    }

    fn extract_sol_transfer_from_parsed(
        &self,
        parsed: &serde_json::Value,
        instruction_index: usize,
    ) -> Option<SolTransfer> {
        let instruction_type = parsed.get("type")?.as_str()?;

        if instruction_type == "transfer" {
            let info = parsed.get("info")?;
            let from = info.get("source")?.as_str()?.to_string();
            let to = info.get("destination")?.as_str()?.to_string();
            let amount_lamports = info.get("lamports")?.as_u64()?;
            let amount_sol = amount_lamports as f64 / 1_000_000_000.0;

            Some(SolTransfer {
                from,
                to,
                amount_lamports,
                amount_sol,
                instruction_index,
                transfer_type: "system".to_string(),
            })
        } else {
            None
        }
    }

    fn extract_sol_transfers_from_balances(
        &self,
        balance_changes: &[BalanceChange],
    ) -> Vec<SolTransfer> {
        // Group balance changes to find transfers
        // This is simplified - in production would need more sophisticated matching
        let increases: Vec<_> = balance_changes
            .iter()
            .filter(|bc| bc.change_lamports > 0)
            .collect();

        let decreases: Vec<_> = balance_changes
            .iter()
            .filter(|bc| bc.change_lamports < 0)
            .collect();

        let mut transfers = Vec::new();

        // Match increases to decreases
        for inc in &increases {
            for dec in &decreases {
                if inc.change_lamports.abs() == dec.change_lamports.abs() {
                    transfers.push(SolTransfer {
                        from: dec.account.clone(),
                        to: inc.account.clone(),
                        amount_lamports: inc.change_lamports.unsigned_abs(),
                        amount_sol: inc.change_sol.abs(),
                        instruction_index: 999, // Inner instruction
                        transfer_type: "balance_change".to_string(),
                    });
                }
            }
        }

        transfers
    }

    // ========================================================================
    // TOKEN TRANSFER EXTRACTION
    // ========================================================================

    fn build_token_account_meta_map(
        &self,
        meta: &serde_json::Value,
        accounts: &[String],
    ) -> HashMap<String, (String, String, u8)> {
        // token_account_pubkey -> (mint, owner, decimals)
        let mut map: HashMap<String, (String, String, u8)> = HashMap::new();

        for key in ["preTokenBalances", "postTokenBalances"] {
            if let Some(entries) = meta.get(key).and_then(|v| v.as_array()) {
                for entry in entries {
                    let Some(account_index) = entry.get("accountIndex").and_then(|v| v.as_u64())
                    else {
                        continue;
                    };
                    let Some(account) = accounts.get(account_index as usize) else {
                        continue;
                    };

                    let mint = entry
                        .get("mint")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let Some(owner) = entry.get("owner").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let decimals = entry
                        .get("uiTokenAmount")
                        .and_then(|v| v.get("decimals"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u8;

                    map.insert(account.clone(), (mint, owner.to_string(), decimals));
                }
            }
        }

        map
    }

    fn extract_token_transfers(
        &self,
        instructions: &[serde_json::Value],
        accounts: &[String],
        meta: &serde_json::Value,
    ) -> Result<Vec<TokenTransfer>> {
        let mut transfers = Vec::new();

        let token_account_meta = self.build_token_account_meta_map(meta, accounts);

        // Extract from main instructions
        for (idx, instr) in instructions.iter().enumerate() {
            if let Some(transfer) = self.extract_token_transfer_from_instruction(
                instr,
                accounts,
                &token_account_meta,
                idx,
            ) {
                transfers.push(transfer);
            }
        }

        // Extract from inner instructions
        if let Some(inner_instructions) = meta.get("innerInstructions").and_then(|i| i.as_array()) {
            for inner_group in inner_instructions {
                if let Some(inner_instrs) =
                    inner_group.get("instructions").and_then(|i| i.as_array())
                {
                    let outer_idx = inner_group
                        .get("index")
                        .and_then(|i| i.as_u64())
                        .unwrap_or(0) as usize;

                    for (inner_idx, inner_instr) in inner_instrs.iter().enumerate() {
                        if let Some(transfer) = self.extract_token_transfer_from_instruction(
                            inner_instr,
                            accounts,
                            &token_account_meta,
                            outer_idx * 1000 + inner_idx, // Encode as inner instruction
                        ) {
                            transfers.push(transfer);
                        }
                    }
                }
            }
        }

        Ok(transfers)
    }

    fn extract_token_transfer_from_instruction(
        &self,
        instr: &serde_json::Value,
        accounts: &[String],
        token_account_meta: &HashMap<String, (String, String, u8)>,
        instruction_index: usize,
    ) -> Option<TokenTransfer> {
        // Get program ID (supports both programIdIndex and jsonParsed programId)
        let program_id = self.instruction_program_id(instr, accounts)?;

        // Only process token program instructions
        if program_id != self.token_program && program_id != self.token_2022_program {
            return None;
        }

        // Check if it's a parsed instruction
        let parsed = instr.get("parsed")?;
        let instruction_type = parsed.get("type")?.as_str()?;

        match instruction_type {
            "transfer" => self.parse_token_transfer(parsed, token_account_meta, instruction_index),
            "transferChecked" => {
                self.parse_token_transfer_checked(parsed, token_account_meta, instruction_index)
            }
            _ => None,
        }
    }

    fn parse_token_transfer(
        &self,
        parsed: &serde_json::Value,
        token_account_meta: &HashMap<String, (String, String, u8)>,
        instruction_index: usize,
    ) -> Option<TokenTransfer> {
        let info = parsed.get("info")?;

        let from_token_account = info.get("source")?.as_str()?.to_string();
        let to_token_account = info.get("destination")?.as_str()?.to_string();
        let authority = info.get("authority")?.as_str()?.to_string();
        let amount = info
            .get("amount")
            .and_then(|a| a.as_str())
            .and_then(|s| s.parse::<u64>().ok())?;

        let (from_mint, from_owner, from_decimals) = token_account_meta
            .get(&from_token_account)
            .map(|(m, o, d)| (m.clone(), Some(o.clone()), *d))
            .unwrap_or_else(|| ("unknown".to_string(), None, 0));
        let (to_mint, to_owner, to_decimals) = token_account_meta
            .get(&to_token_account)
            .map(|(m, o, d)| (m.clone(), Some(o.clone()), *d))
            .unwrap_or_else(|| ("unknown".to_string(), None, 0));

        let mint = if from_mint != "unknown" {
            from_mint
        } else {
            to_mint
        };
        let decimals = if from_decimals != 0 {
            from_decimals
        } else {
            to_decimals
        };
        let amount_ui = if decimals > 0 {
            amount as f64 / 10_u64.pow(decimals as u32) as f64
        } else {
            amount as f64
        };

        Some(TokenTransfer {
            mint,
            from_token_account,
            to_token_account,
            from_owner,
            to_owner,
            amount,
            decimals,
            amount_ui,
            authority,
            instruction_index,
            transfer_type: "transfer".to_string(),
            token_symbol: None,
            token_name: None,
            verified: None,
        })
    }

    fn parse_token_transfer_checked(
        &self,
        parsed: &serde_json::Value,
        token_account_meta: &HashMap<String, (String, String, u8)>,
        instruction_index: usize,
    ) -> Option<TokenTransfer> {
        let info = parsed.get("info")?;

        let from_token_account = info.get("source")?.as_str()?.to_string();
        let to_token_account = info.get("destination")?.as_str()?.to_string();
        let authority = info.get("authority")?.as_str()?.to_string();
        let mint = info
            .get("mint")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                token_account_meta
                    .get(&from_token_account)
                    .map(|(m, _, _)| m.clone())
            })
            .or_else(|| {
                token_account_meta
                    .get(&to_token_account)
                    .map(|(m, _, _)| m.clone())
            })
            .unwrap_or_else(|| "unknown".to_string());
        let decimals = info.get("decimals").and_then(|d| d.as_u64()).unwrap_or(0) as u8;

        let token_amount = info.get("tokenAmount")?;
        let amount = token_amount
            .get("amount")
            .and_then(|a| a.as_str())
            .and_then(|s| s.parse::<u64>().ok())?;
        let from_owner = token_account_meta
            .get(&from_token_account)
            .map(|(_, o, _)| o.clone());
        let to_owner = token_account_meta
            .get(&to_token_account)
            .map(|(_, o, _)| o.clone());
        let amount_ui = token_amount
            .get("uiAmount")
            .and_then(|a| a.as_f64())
            .unwrap_or(amount as f64 / 10_u64.pow(decimals as u32) as f64);

        Some(TokenTransfer {
            mint,
            from_token_account,
            to_token_account,
            from_owner,
            to_owner,
            amount,
            decimals,
            amount_ui,
            authority,
            instruction_index,
            transfer_type: "transferChecked".to_string(),
            token_symbol: None,
            token_name: None,
            verified: None,
        })
    }

    // ========================================================================
    // PROGRAM IDENTIFICATION
    // ========================================================================

    fn extract_program_ids(
        &self,
        instructions: &[serde_json::Value],
        accounts: &[String],
    ) -> Vec<String> {
        instructions
            .iter()
            .filter_map(|instr| self.instruction_program_id(instr, accounts))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    fn get_program_name(&self, program_id: &str) -> String {
        match program_id {
            p if p == self.system_program => "System Program".to_string(),
            p if p == self.token_program => "SPL Token".to_string(),
            p if p == self.token_2022_program => "Token 2022".to_string(),
            p if p == self.associated_token_program => "Associated Token".to_string(),
            p if p == self.raydium_v4 => "Raydium V4".to_string(),
            p if p == self.orca_whirlpool => "Orca Whirlpool".to_string(),
            p if p == self.jupiter_v6 => "Jupiter V6".to_string(),
            p if p == self.metaplex => "Metaplex".to_string(),
            p if p == self.magic_eden => "Magic Eden".to_string(),
            _ => format!(
                "{}...{}",
                &program_id[..8],
                &program_id[program_id.len() - 8..]
            ),
        }
    }

    // ========================================================================
    // TRANSACTION CLASSIFICATION
    // ========================================================================

    fn determine_transaction_type(
        &self,
        programs: &[String],
        sol_transfers: &[SolTransfer],
        token_transfers: &[TokenTransfer],
    ) -> TransactionType {
        // Check for DEX interactions first
        for program in programs {
            if program.contains(&self.raydium_v4)
                || program.contains(&self.orca_whirlpool)
                || program.contains(&self.jupiter_v6)
            {
                return TransactionType::TokenSwap;
            }

            if program.contains(&self.metaplex) || program.contains(&self.magic_eden) {
                return TransactionType::NFTTrade;
            }
        }

        // Check transfer types
        if !token_transfers.is_empty() {
            return TransactionType::TokenTransfer;
        }

        if !sol_transfers.is_empty() {
            return TransactionType::SystemTransfer;
        }

        TransactionType::Unknown
    }
}

impl Default for EnhancedTransactionParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = EnhancedTransactionParser::new();
        assert_eq!(parser.system_program, "11111111111111111111111111111111");
    }

    #[test]
    fn test_balance_change_calculation() {
        let parser = EnhancedTransactionParser::new();
        let accounts = vec!["addr1".to_string(), "addr2".to_string()];
        let pre = vec![1000, 2000];
        let post = vec![500, 2500];

        let changes = parser.calculate_balance_changes(&accounts, &pre, &post);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].change_lamports, -500);
        assert_eq!(changes[1].change_lamports, 500);
    }
}

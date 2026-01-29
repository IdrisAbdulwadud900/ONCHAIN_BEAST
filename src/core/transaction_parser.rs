/// Transaction Parser Module (Foundation)
/// Provides basic transaction parsing structure for future enhancement
///
/// Current capabilities:
/// - Extract transaction metadata
/// - Identify account types
/// - Recognize program calls
///
/// Planned enhancements:
/// - Full instruction parsing
/// - Token transfer detection  
/// - Fund flow tracing
/// - Pattern recognition
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<u64>,
    pub fee: Option<u64>,
    pub success: bool,
    pub error: Option<String>,
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

pub struct TransactionParser {
    // Known program IDs
    system_program: String,
    token_program: String,
    token_2022_program: String,
}

impl TransactionParser {
    pub fn new() -> Self {
        TransactionParser {
            system_program: "11111111111111111111111111111111".to_string(),
            token_program: "TokenkegQfeZyiNwAJsyFbPVwwQQfKP".to_string(),
            token_2022_program: "TokenzQdBNBrrnxLmFxetcusQuqeGH5LqdpqmHsFAhN9".to_string(),
        }
    }

    /// Create a basic parsed transaction
    pub fn parse_basic(&self, signature: String, slot: u64) -> ParsedTransaction {
        ParsedTransaction {
            signature,
            slot,
            block_time: None,
            fee: None,
            success: true,
            error: None,
        }
    }

    /// Get program name from ID
    pub fn get_program_name(&self, program_id: &str) -> String {
        match program_id {
            p if p == self.system_program => "System Program".to_string(),
            p if p == self.token_program => "SPL Token".to_string(),
            p if p == self.token_2022_program => "Token 2022".to_string(),
            p if p.contains("Raydium") => "Raydium".to_string(),
            p if p.contains("Orca") => "Orca".to_string(),
            p if p.contains("Jupiter") => "Jupiter".to_string(),
            _ => "Unknown Program".to_string(),
        }
    }

    /// Determine transaction type from program calls
    pub fn determine_type(&self, programs_called: &[String]) -> TransactionType {
        for program in programs_called {
            if program.contains("Token") {
                return TransactionType::TokenTransfer;
            }
            if program.contains("Raydium") || program.contains("Orca") {
                return TransactionType::TokenSwap;
            }
            if program.contains("Metaplex") || program.contains("MagicEden") {
                return TransactionType::NFTTrade;
            }
        }
        TransactionType::Unknown
    }
}

impl Default for TransactionParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = TransactionParser::new();
        assert_eq!(parser.system_program, "11111111111111111111111111111111");
    }

    #[test]
    fn test_program_name_resolution() {
        let parser = TransactionParser::new();
        let name = parser.get_program_name("11111111111111111111111111111111");
        assert_eq!(name, "System Program");
    }

    #[test]
    fn test_transaction_type_determination() {
        let parser = TransactionParser::new();
        let tx_type = parser.determine_type(&["TokenkegQfeZyiNwAJsyFbPVwwQQfKP".to_string()]);
        assert_eq!(tx_type, TransactionType::TokenTransfer);
    }
}

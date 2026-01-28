use thiserror::Error;

#[derive(Error, Debug)]
pub enum BeastError {
    #[error("RPC Error: {0}")]
    RpcError(String),

    #[error("Database Error: {0}")]
    DatabaseError(String),

    #[error("Invalid wallet address: {0}")]
    InvalidAddress(String),

    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, BeastError>;
pub type BeastResult<T> = std::result::Result<T, BeastError>;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletAnalysisResponse {
    pub wallet_address: String,
    pub connected_wallets: Vec<String>,
    pub risk_score: f64,
    pub is_suspicious: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceResponse {
    pub source: String,
    pub destination: String,
    pub routes: Vec<TraceRoute>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceRoute {
    pub path: Vec<String>,
    pub confidence: f64,
}

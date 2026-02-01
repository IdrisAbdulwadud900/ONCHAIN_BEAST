/// PnL Calculation Engine
/// Advanced profit/loss analytics and position tracking
use crate::core::errors::BeastResult;
use crate::price::JupiterPriceOracle;
use crate::storage::DatabaseManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// PnL calculation service
pub struct PnLEngine {
    db: Arc<DatabaseManager>,
    oracle: Arc<JupiterPriceOracle>,
}

/// Position tracking for a wallet-token pair
#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub wallet: String,
    pub token_mint: String,
    pub total_bought: f64,
    pub total_sold: f64,
    pub current_balance: f64,
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: Option<f64>,
    pub current_price: Option<f64>,
    pub roi_percentage: Option<f64>,
}

/// Claim verification request
#[derive(Debug, Deserialize)]
pub struct ClaimVerificationRequest {
    pub wallet: String,
    pub token_mint: Option<String>,
    pub claimed_amount: f64,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

/// Claim verification result
#[derive(Debug, Serialize)]
pub struct ClaimVerificationResult {
    pub verified: bool,
    pub actual_pnl: f64,
    pub claimed_amount: f64,
    pub difference: f64,
    pub confidence: String,
    pub proof: ClaimProof,
}

/// Proof of claim with transaction evidence
#[derive(Debug, Serialize)]
pub struct ClaimProof {
    pub total_swaps: i64,
    pub winning_swaps: i64,
    pub losing_swaps: i64,
    pub biggest_win: f64,
    pub biggest_loss: f64,
    pub time_range: TimeRange,
    pub transaction_signatures: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TimeRange {
    pub start: i64,
    pub end: i64,
    pub duration_days: i64,
}

/// Performance metrics for a wallet
#[derive(Debug, Serialize)]
pub struct PerformanceMetrics {
    pub wallet: String,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
    pub total_volume: f64,
    pub roi_percentage: f64,
}

impl PnLEngine {
    pub fn new(db: Arc<DatabaseManager>, oracle: Arc<JupiterPriceOracle>) -> Self {
        Self { db, oracle }
    }

    /// Calculate position for a wallet-token pair
    pub async fn calculate_position(
        &self,
        wallet: &str,
        token_mint: &str,
    ) -> BeastResult<Position> {
        // Get all swaps involving this token
        let swaps = self
            .db
            .get_wallet_swaps_with_usd(wallet, None)
            .await?;

        let mut total_bought = 0.0;
        let mut total_sold = 0.0;
        let mut total_buy_value = 0.0;
        let mut total_sell_value = 0.0;
        let mut realized_pnl = 0.0;

        for swap in swaps.iter() {
            let token_in = swap["token_in"].as_str().unwrap_or("");
            let token_out = swap["token_out"].as_str().unwrap_or("");
            let amount_in = swap["amount_in"].as_f64().unwrap_or(0.0);
            let amount_out = swap["amount_out"].as_f64().unwrap_or(0.0);
            let value_in = swap["value_usd_in"].as_f64().unwrap_or(0.0);
            let value_out = swap["value_usd_out"].as_f64().unwrap_or(0.0);

            // Buying the token (token is output)
            if token_out == token_mint {
                total_bought += amount_out;
                total_buy_value += value_in;
            }

            // Selling the token (token is input)
            if token_in == token_mint {
                total_sold += amount_in;
                total_sell_value += value_out;
            }

            // Track realized PnL
            if let Some(pnl) = swap["pnl_usd"].as_f64() {
                if token_in == token_mint || token_out == token_mint {
                    realized_pnl += pnl;
                }
            }
        }

        let current_balance = total_bought - total_sold;
        let avg_buy_price = if total_bought > 0.0 {
            total_buy_value / total_bought
        } else {
            0.0
        };
        let avg_sell_price = if total_sold > 0.0 {
            total_sell_value / total_sold
        } else {
            0.0
        };

        // Calculate unrealized PnL (current holdings)
        let (unrealized_pnl, current_price, roi) = if current_balance > 0.0 {
            match self.oracle.get_price(token_mint).await {
                Ok(price_quote) => {
                    let current_value = current_balance * price_quote.price_usd;
                    let cost_basis = current_balance * avg_buy_price;
                    let unrealized = current_value - cost_basis;
                    let roi_pct = if cost_basis > 0.0 {
                        (unrealized / cost_basis) * 100.0
                    } else {
                        0.0
                    };
                    (Some(unrealized), Some(price_quote.price_usd), Some(roi_pct))
                }
                Err(_) => (None, None, None),
            }
        } else {
            (None, None, None)
        };

        Ok(Position {
            wallet: wallet.to_string(),
            token_mint: token_mint.to_string(),
            total_bought,
            total_sold,
            current_balance,
            avg_buy_price,
            avg_sell_price,
            realized_pnl,
            unrealized_pnl,
            current_price,
            roi_percentage: roi,
        })
    }

    /// Verify a claim about PnL
    pub async fn verify_claim(
        &self,
        request: &ClaimVerificationRequest,
    ) -> BeastResult<ClaimVerificationResult> {
        let actual_pnl = if let Some(ref token) = request.token_mint {
            // Token-specific PnL
            self.db.get_wallet_token_pnl(&request.wallet, token).await?
        } else {
            // Total wallet PnL
            self.db.get_wallet_pnl(&request.wallet).await?
        };

        let verified = actual_pnl >= request.claimed_amount;
        let difference = actual_pnl - request.claimed_amount;

        // Generate proof
        let proof = self.generate_proof(&request).await?;

        let confidence = if verified {
            if difference < request.claimed_amount * 0.1 {
                "HIGH".to_string()
            } else {
                "MEDIUM".to_string()
            }
        } else {
            "FALSE".to_string()
        };

        Ok(ClaimVerificationResult {
            verified,
            actual_pnl,
            claimed_amount: request.claimed_amount,
            difference,
            confidence,
            proof,
        })
    }

    /// Generate proof for a claim
    async fn generate_proof(
        &self,
        request: &ClaimVerificationRequest,
    ) -> BeastResult<ClaimProof> {
        // Get all swaps for the wallet
        let swaps = self
            .db
            .get_wallet_swaps_with_usd(&request.wallet, None)
            .await?;

        let mut total_swaps = 0i64;
        let mut winning_swaps = 0i64;
        let mut losing_swaps = 0i64;
        let mut biggest_win = 0.0f64;
        let mut biggest_loss = 0.0f64;
        let mut signatures = Vec::new();

        let start_time = request.start_time.unwrap_or(0);
        let end_time = request.end_time.unwrap_or(i64::MAX);

        for swap in swaps.iter() {
            let block_time = swap["block_time"].as_i64().unwrap_or(0);
            
            // Filter by time range
            if block_time < start_time || block_time > end_time {
                continue;
            }

            // Filter by token if specified
            if let Some(ref token) = request.token_mint {
                let token_in = swap["token_in"].as_str().unwrap_or("");
                let token_out = swap["token_out"].as_str().unwrap_or("");
                if token_in != token && token_out != token {
                    continue;
                }
            }

            total_swaps += 1;

            if let Some(pnl) = swap["pnl_usd"].as_f64() {
                if pnl > 0.0 {
                    winning_swaps += 1;
                    if pnl > biggest_win {
                        biggest_win = pnl;
                    }
                } else if pnl < 0.0 {
                    losing_swaps += 1;
                    if pnl < biggest_loss {
                        biggest_loss = pnl;
                    }
                }
            }

            if let Some(sig) = swap["signature"].as_str() {
                if signatures.len() < 10 {
                    // Limit to first 10 for brevity
                    signatures.push(sig.to_string());
                }
            }
        }

        let duration_days = ((end_time - start_time) / 86400).max(1);

        Ok(ClaimProof {
            total_swaps,
            winning_swaps,
            losing_swaps,
            biggest_win,
            biggest_loss,
            time_range: TimeRange {
                start: start_time,
                end: end_time,
                duration_days,
            },
            transaction_signatures: signatures,
        })
    }

    /// Calculate performance metrics for a wallet
    pub async fn calculate_performance(
        &self,
        wallet: &str,
    ) -> BeastResult<PerformanceMetrics> {
        let swaps = self.db.get_wallet_swaps_with_usd(wallet, None).await?;

        let mut total_pnl = 0.0;
        let mut total_volume = 0.0;
        let mut wins = Vec::new();
        let mut losses = Vec::new();

        for swap in swaps.iter() {
            if let Some(pnl) = swap["pnl_usd"].as_f64() {
                total_pnl += pnl;
                if pnl > 0.0 {
                    wins.push(pnl);
                } else if pnl < 0.0 {
                    losses.push(pnl);
                }
            }

            if let Some(value_in) = swap["value_usd_in"].as_f64() {
                total_volume += value_in;
            }
        }

        let win_rate = if !wins.is_empty() || !losses.is_empty() {
            (wins.len() as f64 / (wins.len() + losses.len()) as f64) * 100.0
        } else {
            0.0
        };

        let avg_win = if !wins.is_empty() {
            wins.iter().sum::<f64>() / wins.len() as f64
        } else {
            0.0
        };

        let avg_loss = if !losses.is_empty() {
            losses.iter().sum::<f64>() / losses.len() as f64
        } else {
            0.0
        };

        let best_trade = wins.iter().cloned().fold(0.0f64, f64::max);
        let worst_trade = losses.iter().cloned().fold(0.0f64, f64::min);

        let roi_percentage = if total_volume > 0.0 {
            (total_pnl / total_volume) * 100.0
        } else {
            0.0
        };

        Ok(PerformanceMetrics {
            wallet: wallet.to_string(),
            total_pnl,
            win_rate,
            avg_win,
            avg_loss,
            best_trade,
            worst_trade,
            total_volume,
            roi_percentage,
        })
    }
}

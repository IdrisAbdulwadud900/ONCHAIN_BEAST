use crate::modules::{PatternDetector, TransactionGraphBuilder, TransactionHandler};
/// Analysis API Routes
/// Advanced transaction analysis and pattern detection endpoints
use actix_web::{get, post, web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeWalletRequest {
    pub wallet: String,
    #[serde(default = "default_tx_limit")]
    pub transaction_limit: usize,
}

fn default_tx_limit() -> usize {
    50
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeWalletResponse {
    pub success: bool,
    pub wallet: String,
    pub fund_flow_graph: Option<serde_json::Value>,
    pub pattern_analysis: Option<serde_json::Value>,
    pub summary: Option<AnalysisSummary>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub total_transactions: usize,
    pub unique_connections: usize,
    pub total_sol_volume: f64,
    pub total_token_volume: u64,
    pub risk_level: String,
    pub confidence_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchAnalyzeRequest {
    pub transactions: Vec<String>, // Signatures
}

/// Analyze a wallet's transaction history for patterns
#[post("/analysis/wallet")]
async fn analyze_wallet(
    req: web::Json<AnalyzeWalletRequest>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    // Fetch wallet transactions
    let transactions = match handler
        .process_wallet_transactions(&req.wallet, req.transaction_limit)
        .await
    {
        Ok(txs) => txs,
        Err(e) => {
            return HttpResponse::BadRequest().json(AnalyzeWalletResponse {
                success: false,
                wallet: req.wallet.clone(),
                fund_flow_graph: None,
                pattern_analysis: None,
                summary: None,
                error: Some(format!("Failed to fetch transactions: {}", e)),
            });
        }
    };

    if transactions.is_empty() {
        return HttpResponse::Ok().json(AnalyzeWalletResponse {
            success: true,
            wallet: req.wallet.clone(),
            fund_flow_graph: None,
            pattern_analysis: None,
            summary: Some(AnalysisSummary {
                total_transactions: 0,
                unique_connections: 0,
                total_sol_volume: 0.0,
                total_token_volume: 0,
                risk_level: "Low".to_string(),
                confidence_score: 0.0,
            }),
            error: None,
        });
    }

    // Build fund flow graph
    let mut graph_builder = TransactionGraphBuilder::new();
    graph_builder.add_transactions(&transactions);
    let fund_flow_graph = graph_builder.build();

    // Run pattern detection
    let pattern_detector = PatternDetector::new();
    let pattern_analysis = pattern_detector.analyze_patterns(&fund_flow_graph);

    // Build summary
    let summary = AnalysisSummary {
        total_transactions: transactions.len(),
        unique_connections: fund_flow_graph.unique_wallets,
        total_sol_volume: fund_flow_graph.total_volume_sol,
        total_token_volume: fund_flow_graph.total_volume_tokens,
        risk_level: format!("{:?}", pattern_analysis.overall_risk_level),
        confidence_score: pattern_analysis.confidence_score,
    };

    HttpResponse::Ok().json(AnalyzeWalletResponse {
        success: true,
        wallet: req.wallet.clone(),
        fund_flow_graph: serde_json::to_value(&fund_flow_graph).ok(),
        pattern_analysis: serde_json::to_value(&pattern_analysis).ok(),
        summary: Some(summary),
        error: None,
    })
}

/// Build fund flow graph from transaction batch
#[post("/analysis/fund-flow")]
async fn analyze_fund_flow(
    req: web::Json<BatchAnalyzeRequest>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    if req.transactions.len() > 100 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Maximum 100 transactions allowed per request"
        }));
    }

    // Fetch all transactions
    let transactions = match handler
        .process_transactions_batch(req.transactions.clone())
        .await
    {
        Ok(txs) => txs,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to fetch transactions: {}", e)
            }));
        }
    };

    // Build graph
    let mut graph_builder = TransactionGraphBuilder::new();
    graph_builder.add_transactions(&transactions);
    let fund_flow_graph = graph_builder.build();

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "transaction_count": transactions.len(),
        "fund_flow_graph": fund_flow_graph,
    }))
}

/// Detect suspicious patterns in transaction batch
#[post("/analysis/patterns")]
async fn detect_patterns(
    req: web::Json<BatchAnalyzeRequest>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    if req.transactions.len() > 100 {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": "Maximum 100 transactions allowed per request"
        }));
    }

    // Fetch transactions
    let transactions = match handler
        .process_transactions_batch(req.transactions.clone())
        .await
    {
        Ok(txs) => txs,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to fetch transactions: {}", e)
            }));
        }
    };

    // Build graph and detect patterns
    let mut graph_builder = TransactionGraphBuilder::new();
    graph_builder.add_transactions(&transactions);
    let fund_flow_graph = graph_builder.build();

    let pattern_detector = PatternDetector::new();
    let pattern_analysis = pattern_detector.analyze_patterns(&fund_flow_graph);

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "transaction_count": transactions.len(),
        "pattern_analysis": pattern_analysis,
    }))
}

/// Get wallet relationship graph
#[get("/analysis/wallet/{address}/relationships")]
async fn get_wallet_relationships(
    address: web::Path<String>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    // Fetch wallet transactions (limit 50 for performance)
    let transactions = match handler.process_wallet_transactions(&address, 50).await {
        Ok(txs) => txs,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to fetch transactions: {}", e)
            }));
        }
    };

    // Build graph
    let mut graph_builder = TransactionGraphBuilder::new();
    graph_builder.add_transactions(&transactions);
    let fund_flow_graph = graph_builder.build();

    // Extract relationships for this wallet
    let relationships: Vec<_> = fund_flow_graph
        .flows
        .iter()
        .filter(|flow| flow.from == *address || flow.to == *address)
        .collect();

    let connected_wallets: std::collections::HashSet<String> = relationships
        .iter()
        .flat_map(|flow| vec![flow.from.clone(), flow.to.clone()])
        .filter(|w| w != &*address)
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "wallet": address.to_string(),
        "connected_wallets": connected_wallets.len(),
        "relationships": relationships,
        "total_flows": fund_flow_graph.flows.len(),
    }))
}

/// Get wash trading patterns for a wallet
#[get("/analysis/wallet/{address}/wash-trading")]
async fn detect_wash_trading(
    address: web::Path<String>,
    handler: web::Data<Arc<RwLock<TransactionHandler>>>,
) -> HttpResponse {
    let handler = handler.read().await;

    let transactions = match handler.process_wallet_transactions(&address, 100).await {
        Ok(txs) => txs,
        Err(e) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to fetch transactions: {}", e)
            }));
        }
    };

    let mut graph_builder = TransactionGraphBuilder::new();
    graph_builder.add_transactions(&transactions);
    let fund_flow_graph = graph_builder.build();

    let pattern_detector = PatternDetector::new();
    let patterns = pattern_detector.analyze_patterns(&fund_flow_graph);

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "wallet": address.to_string(),
        "wash_trading_patterns": patterns.wash_trading_patterns,
        "circular_flows": patterns.circular_flows,
        "risk_level": patterns.overall_risk_level,
    }))
}

/// Configure analysis routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(analyze_wallet)
        .service(analyze_fund_flow)
        .service(detect_patterns)
        .service(get_wallet_relationships)
        .service(detect_wash_trading);
}

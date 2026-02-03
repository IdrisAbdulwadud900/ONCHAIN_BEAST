/// Minimal REST API server for side-wallet tracing (including CEX-hop heuristics).
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use crate::auth::ApiKey;
use crate::core::rpc_client::SolanaRpcClient;
use crate::modules::{TransactionHandler, TransferAnalytics};
use crate::storage::{BehavioralProfile, DatabaseManager, TransferEvent};

/// Shared server state.
pub struct ApiState {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub tx_handler: Arc<TransactionHandler>,
    pub transfer_analytics: Arc<TransferAnalytics>,
    pub db_manager: Arc<DatabaseManager>,
}

pub async fn start_server(
    rpc_client: Arc<SolanaRpcClient>,
    db_manager: Arc<DatabaseManager>,
    host: &str,
    port: u16,
) -> std::io::Result<()> {
    let tx_handler = Arc::new(TransactionHandler::new(Arc::clone(&rpc_client)));
    let transfer_analytics = Arc::new(TransferAnalytics::new(Arc::clone(&db_manager)));

    let state = web::Data::new(ApiState {
        rpc_client,
        tx_handler,
        transfer_analytics,
        db_manager,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health_check))
            .route(
                "/api/v1/wallet/{address}/side-wallets",
                web::get().to(find_side_wallets),
            )
    })
    .bind((host, port))?
    .run()
    .await
}

async fn index() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "service": "onchain_beast",
        "feature": "side-wallet tracing",
        "endpoints": {
            "health": "/health",
            "side_wallets": "/api/v1/wallet/{address}/side-wallets"
        }
    }))
}

async fn health_check(state: web::Data<ApiState>) -> HttpResponse {
    match state.rpc_client.health_check().await {
        Ok(true) => HttpResponse::Ok().json(json!({
            "status": "healthy",
            "rpc": "connected"
        })),
        Ok(false) => HttpResponse::ServiceUnavailable().json(json!({
            "status": "unhealthy",
            "rpc": "disconnected"
        })),
        Err(e) => HttpResponse::ServiceUnavailable().json(json!({
            "status": "error",
            "error": e.to_string()
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct SideWalletQuery {
    /// Graph expansion depth (1-3 recommended)
    pub depth: Option<usize>,
    /// Minimum score (0.0-1.0)
    pub threshold: Option<f64>,
    /// Max results returned
    pub limit: Option<usize>,
    /// If true, fetch & ingest recent transactions first
    pub bootstrap: Option<bool>,
    /// Number of recent signatures to ingest when bootstrap=true
    pub bootstrap_limit: Option<u64>,
    /// How many days back to consider event-level evidence (transfer_events)
    pub lookback_days: Option<u32>,
    /// If true, attempt to trace through centralized exchanges (heuristic).
    pub cex_hops: Option<bool>,
    /// Extra signatures to ingest for intermediary wallets during cex_hops.
    pub cex_bootstrap_limit: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SideWalletCandidate {
    address: String,
    score: f64,
    depth: usize,
    reasons: Vec<String>,
    tx_count: u32,
    total_sol: f64,
    total_token: u64,
    first_seen_epoch: u64,
    last_seen_epoch: u64,
    direction: String,
    shared_funders_count: u32,
    shared_counterparties_count: u32,
    shared_funders: Vec<String>,
    shared_counterparties: Vec<String>,
    behavioral_similarity: f64,
    temporal_overlap_ratio: f64,
    same_block_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
struct BootstrapStats {
    wallet: String,
    signatures: usize,
    parsed_ok: usize,
    parsed_failed: usize,
    persisted_failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AssetKey {
    Sol,
    Token(String), // mint
}

#[derive(Debug, Clone, serde::Serialize)]
struct CexHopPath {
    deposit_wallet: String,
    hot_wallet: String,
    deposit_signature: String,
    deposit_block_time: i64,
    sweep_signature: Option<String>,
    sweep_block_time: Option<i64>,
    withdrawal_signature: String,
    withdrawal_block_time: i64,
    kind: String,
    mint: Option<String>,
    deposit_amount_ui: f64,
    withdrawal_amount_ui: f64,
    delta_seconds: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CexHopCandidate {
    wallet: String,
    score: f64,
    paths: Vec<CexHopPath>,
    reasons: Vec<String>,
}

fn clamp01(x: f64) -> f64 {
    if x.is_nan() {
        return 0.0;
    }
    x.max(0.0).min(1.0)
}

fn edge_score(tx_count: u32, total_sol: f64, total_token: u64) -> f64 {
    // Lightweight heuristic score based on activity and volume.
    // Output is [0,1].
    let tx = (tx_count as f64 + 1.0).ln();
    let sol = (total_sol.abs() + 1.0).ln();
    let token = ((total_token as f64) / 1_000_000.0 + 1.0).ln();
    let raw = tx * 0.65 + sol * 0.30 + token * 0.05;
    clamp01(1.0 - (-raw / 3.0).exp())
}

fn recency_score(last_seen_epoch: u64) -> f64 {
    // Favor recent relationships (age decay ~30 days).
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if last_seen_epoch == 0 || last_seen_epoch > now {
        return 0.5;
    }

    let age_days = (now - last_seen_epoch) as f64 / 86_400.0;
    let decay = (-age_days / 30.0).exp();
    clamp01(0.15 + 0.85 * decay)
}

fn direction_label(current: &str, conn_from: &str, conn_to: &str) -> String {
    if current == conn_from {
        "outbound".to_string()
    } else if current == conn_to {
        "inbound".to_string()
    } else {
        "unknown".to_string()
    }
}

fn since_epoch_from_days(lookback_days: u32) -> u64 {
    let days = lookback_days.clamp(1, 365) as u64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(days.saturating_mul(86_400))
}

fn format_signal(wallet: &str, count: u64, last_seen_epoch: u64) -> String {
    if last_seen_epoch > 0 {
        format!("{} ({} events; last_seen={})", wallet, count, last_seen_epoch)
    } else {
        format!("{} ({} events)", wallet, count)
    }
}

fn build_reason(from: &str, to: &str, tx_count: u32, total_sol: f64, total_token: u64) -> String {
    if total_sol > 0.0 {
        format!("Link: {} <-> {} ({} tx, {:.4} SOL)", from, to, tx_count, total_sol)
    } else if total_token > 0 {
        format!("Link: {} <-> {} ({} tx, {} token units)", from, to, tx_count, total_token)
    } else {
        format!("Link: {} <-> {} ({} tx)", from, to, tx_count)
    }
}

fn compute_behavioral_similarity(profile_a: &BehavioralProfile, profile_b: &BehavioralProfile) -> f64 {
    // Similarity based on transaction patterns (0.0 - 1.0)

    // 1. Average SOL amount similarity (normalize by log scale)
    let avg_sol_sim = if profile_a.avg_sol_per_tx > 0.0 && profile_b.avg_sol_per_tx > 0.0 {
        let ratio = (profile_a.avg_sol_per_tx / profile_b.avg_sol_per_tx)
            .max(profile_b.avg_sol_per_tx / profile_a.avg_sol_per_tx);
        let log_ratio = ratio.ln().abs();
        (-log_ratio / 2.0).exp()
    } else {
        0.5
    };

    // 2. Transaction frequency similarity (tx per day)
    let freq_sim = if profile_a.avg_tx_per_day > 0.0 && profile_b.avg_tx_per_day > 0.0 {
        let ratio = (profile_a.avg_tx_per_day / profile_b.avg_tx_per_day)
            .max(profile_b.avg_tx_per_day / profile_a.avg_tx_per_day);
        let log_ratio = ratio.ln().abs();
        (-log_ratio / 1.5).exp()
    } else {
        0.5
    };

    // 3. Most active hour similarity (time-of-day clustering)
    let hour_sim = match (profile_a.most_active_hour_utc, profile_b.most_active_hour_utc) {
        (Some(h_a), Some(h_b)) => {
            let diff = (h_a as i32 - h_b as i32).abs();
            let circular_diff = diff.min(24 - diff);
            if circular_diff <= 2 {
                1.0
            } else if circular_diff <= 4 {
                0.7
            } else if circular_diff <= 8 {
                0.4
            } else {
                0.1
            }
        }
        _ => 0.3,
    };

    let combined = (avg_sol_sim * 0.40) + (freq_sim * 0.35) + (hour_sim * 0.25);
    clamp01(combined)
}

fn event_asset_key(ev: &TransferEvent) -> Option<AssetKey> {
    match ev.kind.as_str() {
        "sol" => Some(AssetKey::Sol),
        "token" => ev.mint.clone().map(AssetKey::Token),
        _ => None,
    }
}

fn event_amount_ui(ev: &TransferEvent) -> Option<f64> {
    match ev.kind.as_str() {
        "sol" => ev.amount_sol,
        "token" => ev.token_amount_ui,
        _ => None,
    }
}

fn time_score(delta_seconds: i64) -> f64 {
    if delta_seconds <= 0 {
        return 1.0;
    }
    // Decay half-life ~72 hours.
    let hours = (delta_seconds as f64) / 3600.0;
    clamp01((-hours / 72.0).exp())
}

fn amount_score(ratio: f64) -> f64 {
    if !ratio.is_finite() || ratio <= 0.0 {
        return 0.0;
    }
    // Penalize differences on a log scale.
    let log_ratio = ratio.ln().abs();
    clamp01((-log_ratio / 0.7).exp())
}

async fn bootstrap_ingest_wallet(state: &ApiState, wallet: &str, limit: u64) -> BootstrapStats {
    let mut stats = BootstrapStats {
        wallet: wallet.to_string(),
        signatures: 0,
        parsed_ok: 0,
        parsed_failed: 0,
        persisted_failed: 0,
    };

    if limit == 0 {
        return stats;
    }

    let sigs = match state.rpc_client.get_signatures(wallet, limit.min(100)).await {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("bootstrap get_signatures failed for {}: {}", wallet, e);
            return stats;
        }
    };

    stats.signatures = sigs.len();
    if sigs.is_empty() {
        return stats;
    }

    for s in sigs {
        match state.tx_handler.process_transaction(&s.signature, None).await {
            Ok(tx) => {
                stats.parsed_ok += 1;
                if let Err(e) = state.transfer_analytics.analyze_transaction(&tx).await {
                    stats.persisted_failed += 1;
                    tracing::debug!("bootstrap persist failed {}: {}", s.signature, e);
                }
            }
            Err(e) => {
                stats.parsed_failed += 1;
                tracing::debug!("bootstrap parse failed {}: {}", s.signature, e);
            }
        }
    }

    stats
}

async fn compute_cex_hops(
    state: &ApiState,
    main_wallet: &str,
    lookback_days: u32,
    cex_bootstrap_limit: u64,
    limit: usize,
) -> (Vec<CexHopCandidate>, Vec<BootstrapStats>) {
    // Note: On-chain "CEX hop" attribution is heuristic. Exchanges pool funds, so we
    // surface candidates with confidence scores + evidence, not a definitive proof.
    const MAX_DEPOSIT_WALLETS: usize = 8;
    const MAX_DEPOSIT_EVENTS_PER_WALLET: usize = 3;
    const MAX_HOT_WALLETS_PER_DEPOSIT: usize = 2;
    const MAX_WITHDRAWALS_PER_HOT: usize = 60;
    const SWEEP_WINDOW_SECS: u64 = 48 * 3600;
    const WITHDRAW_WINDOW_SECS: u64 = 7 * 24 * 3600;

    let since_epoch = since_epoch_from_days(lookback_days);
    let limit = limit.clamp(1, 50);
    let mut bootstrapped_wallets: HashSet<String> = HashSet::new();
    let mut bootstrap_stats: Vec<BootstrapStats> = Vec::new();

    let top_recipients = match state
        .db_manager
        .get_top_outbound_recipients(main_wallet, Some(since_epoch), 30)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!("cex hop unavailable (top outbound recipients): {}", e);
            return (Vec::new(), bootstrap_stats);
        }
    };

    let mut agg: HashMap<String, CexHopCandidate> = HashMap::new();

    for recip in top_recipients.into_iter().take(MAX_DEPOSIT_WALLETS) {
        let deposit_wallet = recip.wallet;
        if deposit_wallet == main_wallet {
            continue;
        }

        // Fetch recent deposit transfer events main -> deposit.
        let deposits = match state
            .db_manager
            .get_transfers_between(main_wallet, &deposit_wallet, Some(since_epoch), 25)
            .await
        {
            Ok(v) => v,
            Err(_) => continue,
        };

        let deposit_events: Vec<TransferEvent> = deposits
            .into_iter()
            .filter(|e| e.block_time.is_some())
            .take(MAX_DEPOSIT_EVENTS_PER_WALLET)
            .collect();

        if deposit_events.is_empty() {
            continue;
        }

        // Bootstrap deposit wallet so we can see sweeps in transfer_events.
        if bootstrapped_wallets.insert(deposit_wallet.clone()) {
            bootstrap_stats.push(
                bootstrap_ingest_wallet(state, &deposit_wallet, cex_bootstrap_limit).await,
            );
        }

        for deposit_ev in deposit_events {
            let deposit_time = deposit_ev.block_time.unwrap() as u64;
            let asset = match event_asset_key(&deposit_ev) {
                Some(a) => a,
                None => continue,
            };
            let deposit_amount = match event_amount_ui(&deposit_ev) {
                Some(a) if a > 0.0 => a,
                _ => continue,
            };

            // Find sweeps from deposit wallet shortly after the deposit.
            let sweep_window_end = deposit_time.saturating_add(SWEEP_WINDOW_SECS);
            let sweeps = match state
                .db_manager
                .get_outbound_transfers_in_window(&deposit_wallet, deposit_time, sweep_window_end, 400)
                .await
            {
                Ok(v) => v,
                Err(_) => continue,
            };

            let mut hot_wallets: Vec<(String, Option<String>, Option<i64>, f64)> = Vec::new();
            for s in sweeps.iter() {
                let to_wallet = match &s.to_wallet {
                    Some(w) => w,
                    None => continue,
                };
                if to_wallet == main_wallet || to_wallet == &deposit_wallet {
                    continue;
                }
                if event_asset_key(s) != Some(asset.clone()) {
                    continue;
                }
                let sweep_amount = match event_amount_ui(s) {
                    Some(a) => a,
                    None => continue,
                };
                if sweep_amount < deposit_amount * 0.5 {
                    continue;
                }

                hot_wallets.push((
                    to_wallet.clone(),
                    Some(s.signature.clone()),
                    s.block_time,
                    sweep_amount,
                ));
                if hot_wallets.len() >= MAX_HOT_WALLETS_PER_DEPOSIT {
                    break;
                }
            }

            if hot_wallets.is_empty() {
                continue;
            }

            for (hot_wallet, sweep_sig, sweep_time, _sweep_amount) in hot_wallets {
                if bootstrapped_wallets.insert(hot_wallet.clone()) {
                    bootstrap_stats.push(
                        bootstrap_ingest_wallet(state, &hot_wallet, cex_bootstrap_limit).await,
                    );
                }

                let withdraw_window_end = deposit_time.saturating_add(WITHDRAW_WINDOW_SECS);
                let outflows = match state
                    .db_manager
                    .get_outbound_transfers_in_window(
                        &hot_wallet,
                        deposit_time,
                        withdraw_window_end,
                        MAX_WITHDRAWALS_PER_HOT,
                    )
                    .await
                {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                for w in outflows {
                    let recipient = match &w.to_wallet {
                        Some(v) => v.clone(),
                        None => continue,
                    };
                    if recipient == main_wallet
                        || recipient == deposit_wallet
                        || recipient == hot_wallet
                    {
                        continue;
                    }
                    if event_asset_key(&w) != Some(asset.clone()) {
                        continue;
                    }

                    let withdraw_time = match w.block_time {
                        Some(t) if t > 0 => t as u64,
                        _ => continue,
                    };
                    let withdraw_amount = match event_amount_ui(&w) {
                        Some(a) if a > 0.0 => a,
                        _ => continue,
                    };

                    let ratio = withdraw_amount / deposit_amount;
                    if ratio < 0.05 || ratio > 1.10 {
                        continue;
                    }

                    let delta_seconds = (withdraw_time as i64) - (deposit_time as i64);
                    let s_time = time_score(delta_seconds);
                    let s_amt = amount_score(ratio);
                    let score = clamp01(0.60 * s_time + 0.40 * s_amt);
                    if score < 0.10 {
                        continue;
                    }

                    let path = CexHopPath {
                        deposit_wallet: deposit_wallet.clone(),
                        hot_wallet: hot_wallet.clone(),
                        deposit_signature: deposit_ev.signature.clone(),
                        deposit_block_time: deposit_time as i64,
                        sweep_signature: sweep_sig.clone(),
                        sweep_block_time: sweep_time,
                        withdrawal_signature: w.signature.clone(),
                        withdrawal_block_time: withdraw_time as i64,
                        kind: w.kind.clone(),
                        mint: w.mint.clone(),
                        deposit_amount_ui: deposit_amount,
                        withdrawal_amount_ui: withdraw_amount,
                        delta_seconds,
                    };

                    let entry =
                        agg.entry(recipient.clone()).or_insert_with(|| CexHopCandidate {
                            wallet: recipient.clone(),
                            score,
                            paths: Vec::new(),
                            reasons: Vec::new(),
                        });

                    if score > entry.score {
                        entry.score = score;
                    }
                    if entry.paths.len() < 5 {
                        entry.paths.push(path);
                    }
                    if entry.reasons.len() < 5 {
                        entry.reasons.push(format!(
                            "CEX hop: {} -> {} -> {} -> {} (dt={}s, score={:.2})",
                            main_wallet, deposit_wallet, hot_wallet, recipient, delta_seconds, score
                        ));
                    }
                }
            }
        }
    }

    let mut out: Vec<CexHopCandidate> = agg.into_values().collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(limit);

    (out, bootstrap_stats)
}

async fn enrich_candidates_with_event_signals(
    state: &ApiState,
    main_wallet: &str,
    candidates: &mut [SideWalletCandidate],
    lookback_days: u32,
) {
    let since_epoch = since_epoch_from_days(lookback_days);

    // Precompute main wallet counterparties once.
    let main_counterparties = match state
        .db_manager
        .get_top_counterparties(main_wallet, Some(since_epoch), 80)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Event evidence unavailable (get_top_counterparties failed): {}", e);
            return;
        }
    };
    let main_set: HashSet<String> = main_counterparties.iter().map(|s| s.wallet.clone()).collect();

    for c in candidates.iter_mut() {
        match state
            .db_manager
            .get_shared_inbound_senders(main_wallet, &c.address, Some(since_epoch), 3)
            .await
        {
            Ok(shared) => {
                c.shared_funders_count = shared.len() as u32;
                c.shared_funders = shared
                    .iter()
                    .map(|s| format_signal(&s.wallet, s.count, s.last_seen_epoch))
                    .collect();
                for s in shared.iter().take(3) {
                    if c.reasons.len() < 8 {
                        c.reasons.push(format!(
                            "Shared inbound funder: {}",
                            format_signal(&s.wallet, s.count, s.last_seen_epoch)
                        ));
                    }
                }
            }
            Err(e) => tracing::debug!("shared funders query failed: {}", e),
        }

        match state
            .db_manager
            .get_top_counterparties(&c.address, Some(since_epoch), 80)
            .await
        {
            Ok(other) => {
                let mut shared_wallets: Vec<String> = other
                    .iter()
                    .map(|s| s.wallet.clone())
                    .filter(|w| main_set.contains(w))
                    .collect();
                shared_wallets.sort();
                shared_wallets.dedup();

                c.shared_counterparties_count = shared_wallets.len() as u32;
                c.shared_counterparties = shared_wallets.iter().take(3).cloned().collect();

                for w in shared_wallets.iter().take(3) {
                    if c.reasons.len() < 8 {
                        c.reasons.push(format!("Shared counterparty: {}", w));
                    }
                }
            }
            Err(e) => tracing::debug!("counterparty query failed: {}", e),
        }

        let bump = 0.06 * (c.shared_funders_count.min(3) as f64)
            + 0.03 * (c.shared_counterparties_count.min(5) as f64);
        c.score = clamp01(c.score + bump);
    }

    // Behavioral correlation: compare transaction patterns
    let main_profile = state
        .db_manager
        .get_behavioral_profile(main_wallet, Some(since_epoch))
        .await
        .ok()
        .flatten();

    if let Some(main_prof) = main_profile {
        for c in candidates.iter_mut() {
            match state
                .db_manager
                .get_behavioral_profile(&c.address, Some(since_epoch))
                .await
            {
                Ok(Some(cand_prof)) => {
                    let similarity = compute_behavioral_similarity(&main_prof, &cand_prof);
                    c.behavioral_similarity = similarity;
                    if similarity > 0.65 {
                        if c.reasons.len() < 8 {
                            c.reasons.push(format!(
                                "Behavioral pattern match (similarity: {:.2})",
                                similarity
                            ));
                        }
                        c.score = clamp01(c.score + similarity * 0.12);
                    }
                }
                Ok(None) => c.behavioral_similarity = 0.0,
                Err(e) => {
                    tracing::debug!("behavioral profile query failed for {}: {}", c.address, e);
                    c.behavioral_similarity = 0.0;
                }
            }
        }
    }

    // Temporal alignment: detect synchronized activity windows
    for c in candidates.iter_mut() {
        match state
            .db_manager
            .get_temporal_overlap(main_wallet, &c.address, Some(since_epoch), 5)
            .await
        {
            Ok(overlap) => {
                c.temporal_overlap_ratio = overlap.overlap_ratio;
                c.same_block_count = overlap.same_block_count;

                if overlap.same_block_count > 0 {
                    if c.reasons.len() < 8 {
                        c.reasons
                            .push(format!("Same-block activity ({} shared blocks)", overlap.same_block_count));
                    }
                    c.score = clamp01(c.score + 0.08 * (overlap.same_block_count.min(5) as f64 * 0.2));
                } else if overlap.overlap_ratio > 0.15 {
                    if c.reasons.len() < 8 {
                        c.reasons.push(format!(
                            "Synchronized activity windows ({:.1}% overlap)",
                            overlap.overlap_ratio * 100.0
                        ));
                    }
                    c.score = clamp01(c.score + overlap.overlap_ratio * 0.10);
                }
            }
            Err(e) => {
                tracing::debug!("temporal overlap query failed for {}: {}", c.address, e);
                c.temporal_overlap_ratio = 0.0;
                c.same_block_count = 0;
            }
        }
    }
}

async fn compute_side_wallets(
    state: &ApiState,
    main_wallet: &str,
    max_depth: usize,
    threshold: f64,
    limit: usize,
    lookback_days: u32,
) -> Result<Vec<SideWalletCandidate>, String> {
    let max_depth = max_depth.clamp(1, 5);
    let threshold = clamp01(threshold);
    let limit = limit.clamp(1, 100);

    // BFS over wallet_relationships graph.
    let mut queue: VecDeque<(String, usize, f64)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut best: HashMap<String, SideWalletCandidate> = HashMap::new();

    queue.push_back((main_wallet.to_string(), 0, 1.0));
    visited.insert(main_wallet.to_string());

    while let Some((current, depth, parent_score)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let connections = state
            .db_manager
            .get_wallet_connections(&current)
            .await
            .map_err(|e| format!("Failed to get connections: {}", e))?;

        for conn in connections {
            let (from, to) = (&conn.from_wallet, &conn.to_wallet);
            let other = if from == &current { to } else { from };
            if other == main_wallet {
                continue;
            }

            let mut s = edge_score(conn.transaction_count, conn.total_sol_transferred, conn.total_token_transferred);

            // Penalize very weak, single-touch relationships.
            if conn.transaction_count <= 1
                && conn.total_sol_transferred.abs() < 0.01
                && conn.total_token_transferred == 0
            {
                s *= 0.35;
            }

            // Favor recent relationships.
            s *= recency_score(conn.last_seen_epoch);

            let combined = clamp01(parent_score * s * (0.85_f64).powi((depth as i32) + 1));
            if combined < threshold {
                continue;
            }

            let dir = direction_label(&current, from, to);
            let reason = build_reason(from, to, conn.transaction_count, conn.total_sol_transferred, conn.total_token_transferred);

            let entry = best.entry(other.to_string()).or_insert_with(|| SideWalletCandidate {
                address: other.to_string(),
                score: combined,
                depth: depth + 1,
                reasons: Vec::new(),
                tx_count: conn.transaction_count,
                total_sol: conn.total_sol_transferred,
                total_token: conn.total_token_transferred,
                first_seen_epoch: conn.first_seen_epoch,
                last_seen_epoch: conn.last_seen_epoch,
                direction: dir.clone(),
                shared_funders_count: 0,
                shared_counterparties_count: 0,
                shared_funders: Vec::new(),
                shared_counterparties: Vec::new(),
                behavioral_similarity: 0.0,
                temporal_overlap_ratio: 0.0,
                same_block_count: 0,
            });

            if combined > entry.score {
                entry.score = combined;
                entry.depth = depth + 1;
                entry.tx_count = conn.transaction_count;
                entry.total_sol = conn.total_sol_transferred;
                entry.total_token = conn.total_token_transferred;
                entry.first_seen_epoch = conn.first_seen_epoch;
                entry.last_seen_epoch = conn.last_seen_epoch;
                entry.direction = dir.clone();
            }

            if entry.reasons.len() < 5 {
                if conn.last_seen_epoch > 0 {
                    entry.reasons.push(format!("{} ({}; last_seen={})", reason, dir, conn.last_seen_epoch));
                } else {
                    entry.reasons.push(format!("{} ({})", reason, dir));
                }
            }

            if !visited.contains(other) {
                visited.insert(other.to_string());
                queue.push_back((other.to_string(), depth + 1, combined));
            }
        }
    }

    let mut results: Vec<SideWalletCandidate> = best.into_values().filter(|c| c.score >= threshold).collect();
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    enrich_candidates_with_event_signals(state, main_wallet, &mut results, lookback_days).await;

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    Ok(results)
}

/// Find side wallets (direct graph + event signals) and CEX-hop funded wallets (heuristic).
async fn find_side_wallets(
    _auth: ApiKey,
    state: web::Data<ApiState>,
    address: web::Path<String>,
    query: web::Query<SideWalletQuery>,
) -> HttpResponse {
    let wallet = address.into_inner();

    let depth = query.depth.unwrap_or(2);
    let threshold = query.threshold.unwrap_or(0.10);
    let limit = query.limit.unwrap_or(15);
    let bootstrap = query.bootstrap.unwrap_or(true);
    let bootstrap_limit = query.bootstrap_limit.unwrap_or(25).min(100);
    let lookback_days = query.lookback_days.unwrap_or(30).clamp(1, 365);
    let cex_hops = query.cex_hops.unwrap_or(true);
    let cex_bootstrap_limit = query.cex_bootstrap_limit.unwrap_or(15).min(100);

    // Bootstrap main wallet: fetch recent signatures, parse transactions, and persist events/relationships.
    let mut bootstrap_stats = BootstrapStats {
        wallet: wallet.clone(),
        signatures: 0,
        parsed_ok: 0,
        parsed_failed: 0,
        persisted_failed: 0,
    };
    let mut bootstrap_errors: Vec<String> = Vec::new();

    if bootstrap {
        match state.rpc_client.get_signatures(&wallet, bootstrap_limit).await {
            Ok(sigs) => {
                bootstrap_stats.signatures = sigs.len();
                for s in sigs {
                    match state.tx_handler.process_transaction(&s.signature, None).await {
                        Ok(tx) => {
                            bootstrap_stats.parsed_ok += 1;
                            if let Err(e) = state.transfer_analytics.analyze_transaction(&tx).await {
                                bootstrap_stats.persisted_failed += 1;
                                if bootstrap_errors.len() < 3 {
                                    bootstrap_errors.push(format!("persist {}: {}", s.signature, e));
                                }
                            }
                        }
                        Err(e) => {
                            bootstrap_stats.parsed_failed += 1;
                            if bootstrap_errors.len() < 3 {
                                bootstrap_errors.push(format!("parse {}: {}", s.signature, e));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Side-wallet bootstrap failed for {}: {}", wallet, e);
                bootstrap_errors.push(format!("get_signatures: {}", e));
            }
        }
    }

    let candidates = match compute_side_wallets(&state, &wallet, depth, threshold, limit, lookback_days).await {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError().json(json!({
                "error": e
            }));
        }
    };

    let (cex_candidates, cex_bootstrap) = if cex_hops {
        compute_cex_hops(&state, &wallet, lookback_days, cex_bootstrap_limit, 10).await
    } else {
        (Vec::new(), Vec::new())
    };

    HttpResponse::Ok().json(json!({
        "main_wallet": wallet,
        "side_wallets": candidates,
        "cex_hops_enabled": cex_hops,
        "cex_funded_wallets": cex_candidates,
        "cex_bootstrap_limit": cex_bootstrap_limit,
        "cex_bootstrap_stats": cex_bootstrap,
        "confidence_threshold": threshold,
        "analysis_depth": depth,
        "lookback_days": lookback_days,
        "bootstrap": bootstrap,
        "bootstrap_stats": bootstrap_stats,
        "bootstrap_errors": bootstrap_errors,
        "message": if candidates.is_empty() && cex_candidates.is_empty() {
            "No candidates yet. Try increasing bootstrap_limit/cex_bootstrap_limit or lookback_days."
        } else {
            "OK"
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::enhanced_parser::{EnhancedTransaction, SolTransfer, TransactionType};
    use std::collections::HashSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_epoch() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn sol_transfer(from: &str, to: &str, amount_sol: f64, instruction_index: usize) -> SolTransfer {
        SolTransfer {
            from: from.to_string(),
            to: to.to_string(),
            amount_lamports: (amount_sol * 1_000_000_000.0) as u64,
            amount_sol,
            instruction_index,
            transfer_type: "system".to_string(),
        }
    }

    fn tx(signature: &str, slot: u64, block_time: u64, sol_transfers: Vec<SolTransfer>) -> EnhancedTransaction {
        EnhancedTransaction {
            signature: signature.to_string(),
            slot,
            block_time: Some(block_time),
            fee: 0,
            success: true,
            error: None,
            accounts: Vec::new(),
            signers: Vec::new(),
            writable_accounts: Vec::new(),
            sol_transfers,
            token_transfers: Vec::new(),
            balance_changes: Vec::new(),
            programs_called: Vec::new(),
            program_names: Vec::new(),
            tx_type: TransactionType::Unknown,
            is_versioned: false,
        }
    }

    async fn test_state() -> ApiState {
        let db_manager = Arc::new(DatabaseManager::new("memory").await.unwrap());
        db_manager.init_schema().await.unwrap();

        let rpc_client = Arc::new(SolanaRpcClient::new("http://localhost".to_string()));
        let tx_handler = Arc::new(TransactionHandler::new(Arc::clone(&rpc_client)));
        let transfer_analytics = Arc::new(TransferAnalytics::new(Arc::clone(&db_manager)));

        ApiState {
            rpc_client,
            tx_handler,
            transfer_analytics,
            db_manager,
        }
    }

    #[tokio::test]
    async fn detects_side_wallets_from_graph_edges() {
        let state = test_state().await;
        let now = now_epoch();

        let main = "MAIN";
        let side1 = "SIDE1";
        let side2 = "SIDE2";

        let mut transfers_a = Vec::new();
        for i in 0..10 {
            transfers_a.push(sol_transfer(main, side1, 1.0, i));
        }
        let mut transfers_b = Vec::new();
        for i in 0..10 {
            transfers_b.push(sol_transfer(side1, side2, 1.0, i));
        }

        let tx_a = tx("sig_main_side1", 1, now - 3600, transfers_a);
        let tx_b = tx("sig_side1_side2", 2, now - 3500, transfers_b);

        state.transfer_analytics.analyze_transaction(&tx_a).await.unwrap();
        state.transfer_analytics.analyze_transaction(&tx_b).await.unwrap();

        let candidates = compute_side_wallets(&state, main, 2, 0.10, 25, 30)
            .await
            .unwrap();
        let addrs: HashSet<String> = candidates.iter().map(|c| c.address.clone()).collect();

        assert!(addrs.contains(side1), "expected {} in {:?}", side1, addrs);
        assert!(addrs.contains(side2), "expected {} in {:?}", side2, addrs);
    }

    #[tokio::test]
    async fn detects_cex_hop_withdrawal_recipient() {
        let state = test_state().await;
        let now = now_epoch();

        let main = "MAIN";
        let deposit = "CEX_DEPOSIT";
        let hot = "CEX_HOT";
        let recipient = "RECIPIENT";

        let deposit_time = now - 2 * 86_400;
        let sweep_time = deposit_time + 3600;
        let withdraw_time = deposit_time + 86_400;

        let tx_deposit = tx(
            "sig_deposit",
            10,
            deposit_time,
            vec![sol_transfer(main, deposit, 50.0, 0)],
        );
        let tx_sweep = tx(
            "sig_sweep",
            11,
            sweep_time,
            vec![sol_transfer(deposit, hot, 49.0, 0)],
        );
        let tx_withdraw = tx(
            "sig_withdraw",
            12,
            withdraw_time,
            vec![sol_transfer(hot, recipient, 45.0, 0)],
        );

        state
            .transfer_analytics
            .analyze_transaction(&tx_deposit)
            .await
            .unwrap();
        state.transfer_analytics.analyze_transaction(&tx_sweep).await.unwrap();
        state
            .transfer_analytics
            .analyze_transaction(&tx_withdraw)
            .await
            .unwrap();

        let (cands, _bootstrap) = compute_cex_hops(&state, main, 30, 0, 10).await;
        let cand = cands
            .into_iter()
            .find(|c| c.wallet == recipient)
            .expect("expected cex-funded recipient");

        assert!(cand.score > 0.1);
        assert!(
            cand.paths.iter().any(|p| p.deposit_wallet == deposit && p.hot_wallet == hot),
            "expected path via deposit/hot; got {:?}",
            cand.paths
        );
    }
}

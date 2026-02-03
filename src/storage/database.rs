use crate::core::enhanced_parser::{EnhancedTransaction, SolTransfer, TokenTransfer};
use crate::core::errors::{BeastError, BeastResult};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_postgres::{Client, NoTls, Row};

/// Storage used by side-wallet tracing.
pub struct DatabaseManager {
    inner: DatabaseInner,
}

enum DatabaseInner {
    Postgres { client: Client },
    Memory { state: RwLock<MemoryState> },
}

#[derive(Default)]
struct MemoryState {
    transactions: HashMap<String, EnhancedTransaction>,
    relationships: HashMap<(String, String), MemoryRelationship>,
    transfer_events: HashMap<(String, i32), TransferEvent>,
}

struct MemoryRelationship {
    from_wallet: String,
    to_wallet: String,
    sol_amount: f64,
    token_amount: u64,
    transaction_count: u32,
    first_seen_epoch: u64,
    last_seen_epoch: u64,
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn passes_since(block_time: Option<i64>, since_epoch: i64) -> bool {
    match block_time {
        None => true,
        Some(t) => t >= since_epoch,
    }
}

impl DatabaseManager {
    pub async fn new(database_url: &str) -> BeastResult<Self> {
        let database_url = database_url.trim();

        if database_url.eq_ignore_ascii_case("memory") || database_url.starts_with("memory:") {
            return Ok(Self {
                inner: DatabaseInner::Memory {
                    state: RwLock::new(MemoryState::default()),
                },
            });
        }

        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to connect: {}", e)))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Database connection error: {}", e);
            }
        });

        Ok(Self {
            inner: DatabaseInner::Postgres { client },
        })
    }

    /// Initialize minimal schema required for tracing.
    pub async fn init_schema(&self) -> BeastResult<()> {
        let DatabaseInner::Postgres { client } = &self.inner else {
            // In-memory backend requires no schema.
            return Ok(());
        };

        client
            .execute(
                "CREATE TABLE IF NOT EXISTS transactions (
                    signature TEXT PRIMARY KEY,
                    slot BIGINT NOT NULL,
                    block_time BIGINT,
                    success BOOLEAN NOT NULL,
                    fee BIGINT NOT NULL,
                    sol_transfers_count INTEGER NOT NULL DEFAULT 0,
                    token_transfers_count INTEGER NOT NULL DEFAULT 0,
                    data JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )",
                &[],
            )
            .await
            .map_err(|e| {
                BeastError::DatabaseError(format!("Failed to create transactions table: {}", e))
            })?;

        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot)",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time)",
                &[],
            )
            .await
            .ok();

        // Graph edges derived from transfers (wallet <-> wallet).
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS wallet_relationships (
                    id SERIAL PRIMARY KEY,
                    from_wallet TEXT NOT NULL,
                    to_wallet TEXT NOT NULL,
                    sol_amount DOUBLE PRECISION NOT NULL DEFAULT 0,
                    token_amount BIGINT NOT NULL DEFAULT 0,
                    transaction_count INTEGER NOT NULL DEFAULT 1,
                    first_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    UNIQUE(from_wallet, to_wallet)
                )",
                &[],
            )
            .await
            .map_err(|e| {
                BeastError::DatabaseError(format!(
                    "Failed to create wallet_relationships table: {}",
                    e
                ))
            })?;

        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_wallet_relationships_from ON wallet_relationships(from_wallet)",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_wallet_relationships_to ON wallet_relationships(to_wallet)",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_wallet_relationships_last_seen ON wallet_relationships(last_seen DESC)",
                &[],
            )
            .await
            .ok();

        // Event-level transfers used for evidence + higher-signal scoring.
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS transfer_events (
                    id SERIAL PRIMARY KEY,
                    signature TEXT NOT NULL,
                    event_index INTEGER NOT NULL,
                    slot BIGINT NOT NULL,
                    block_time BIGINT,
                    kind TEXT NOT NULL,
                    instruction_index INTEGER NOT NULL,
                    transfer_type TEXT NOT NULL,
                    from_wallet TEXT,
                    to_wallet TEXT,
                    mint TEXT,
                    amount_lamports BIGINT,
                    amount_sol DOUBLE PRECISION,
                    token_amount BIGINT,
                    token_decimals INTEGER,
                    token_amount_ui DOUBLE PRECISION,
                    from_token_account TEXT,
                    to_token_account TEXT,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    UNIQUE(signature, event_index)
                )",
                &[],
            )
            .await
            .map_err(|e| {
                BeastError::DatabaseError(format!("Failed to create transfer_events table: {}", e))
            })?;

        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_signature ON transfer_events(signature)",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_from_wallet ON transfer_events(from_wallet)",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_to_wallet ON transfer_events(to_wallet)",
                &[],
            )
            .await
            .ok();
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_block_time ON transfer_events(block_time)",
                &[],
            )
            .await
            .ok();

        Ok(())
    }

    pub async fn store_transaction(&self, tx: &EnhancedTransaction) -> BeastResult<()> {
        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let data = serde_json::to_value(tx).map_err(|e| {
                    BeastError::DatabaseError(format!("Failed to serialize transaction: {}", e))
                })?;

                client
                    .execute(
                        "INSERT INTO transactions (signature, slot, block_time, success, fee, sol_transfers_count, token_transfers_count, data)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, ($8::text)::jsonb)
                         ON CONFLICT (signature) DO UPDATE SET
                            data = EXCLUDED.data,
                            sol_transfers_count = EXCLUDED.sol_transfers_count,
                            token_transfers_count = EXCLUDED.token_transfers_count",
                        &[
                            &tx.signature,
                            &(tx.slot as i64),
                            &tx.block_time.map(|t| t as i64),
                            &tx.success,
                            &(tx.fee as i64),
                            &(tx.sol_transfers.len() as i32),
                            &(tx.token_transfers.len() as i32),
                            &data.to_string(),
                        ],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!("Failed to store transaction: {}", e))
                    })?;

                Ok(())
            }
            DatabaseInner::Memory { state } => {
                let mut mem = state.write().await;
                mem.transactions
                    .insert(tx.signature.clone(), tx.clone());
                Ok(())
            }
        }
    }

    /// Upsert a wallet-relationship edge.
    pub async fn store_wallet_relationship(
        &self,
        from_wallet: &str,
        to_wallet: &str,
        sol_amount: f64,
        token_amount: u64,
    ) -> BeastResult<()> {
        match &self.inner {
            DatabaseInner::Postgres { client } => {
                client
                    .execute(
                        "INSERT INTO wallet_relationships (from_wallet, to_wallet, sol_amount, token_amount, transaction_count)
                         VALUES ($1, $2, $3, $4, 1)
                         ON CONFLICT (from_wallet, to_wallet) DO UPDATE SET
                            sol_amount = wallet_relationships.sol_amount + EXCLUDED.sol_amount,
                            token_amount = wallet_relationships.token_amount + EXCLUDED.token_amount,
                            transaction_count = wallet_relationships.transaction_count + 1,
                            last_seen = NOW()",
                        &[&from_wallet, &to_wallet, &sol_amount, &(token_amount as i64)],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!("Failed to store relationship: {}", e))
                    })?;

                Ok(())
            }
            DatabaseInner::Memory { state } => {
                let now = now_epoch();
                let mut mem = state.write().await;
                let key = (from_wallet.to_string(), to_wallet.to_string());
                let entry = mem.relationships.entry(key).or_insert_with(|| MemoryRelationship {
                    from_wallet: from_wallet.to_string(),
                    to_wallet: to_wallet.to_string(),
                    sol_amount: 0.0,
                    token_amount: 0,
                    transaction_count: 0,
                    first_seen_epoch: now,
                    last_seen_epoch: now,
                });

                entry.sol_amount += sol_amount;
                entry.token_amount = entry.token_amount.saturating_add(token_amount);
                entry.transaction_count = entry.transaction_count.saturating_add(1);
                entry.last_seen_epoch = now;
                Ok(())
            }
        }
    }

    /// Store a SOL transfer as an event (idempotent per signature+event_index).
    pub async fn store_sol_transfer_event(
        &self,
        tx: &EnhancedTransaction,
        transfer: &SolTransfer,
        event_index: i32,
    ) -> BeastResult<()> {
        match &self.inner {
            DatabaseInner::Postgres { client } => {
                client
                    .execute(
                        "INSERT INTO transfer_events (
                            signature,
                            event_index,
                            slot,
                            block_time,
                            kind,
                            instruction_index,
                            transfer_type,
                            from_wallet,
                            to_wallet,
                            amount_lamports,
                            amount_sol
                         ) VALUES ($1,$2,$3,$4,'sol',$5,$6,$7,$8,$9,$10)
                         ON CONFLICT (signature, event_index) DO NOTHING",
                        &[
                            &tx.signature,
                            &event_index,
                            &(tx.slot as i64),
                            &tx.block_time.map(|t| t as i64),
                            &(transfer.instruction_index as i32),
                            &transfer.transfer_type,
                            &transfer.from,
                            &transfer.to,
                            &(transfer.amount_lamports as i64),
                            &transfer.amount_sol,
                        ],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to store SOL transfer event: {}",
                            e
                        ))
                    })?;

                Ok(())
            }
            DatabaseInner::Memory { state } => {
                let mut mem = state.write().await;
                let key = (tx.signature.clone(), event_index);
                if mem.transfer_events.contains_key(&key) {
                    return Ok(());
                }

                mem.transfer_events.insert(
                    key,
                    TransferEvent {
                        signature: tx.signature.clone(),
                        event_index,
                        slot: tx.slot as i64,
                        block_time: tx.block_time.map(|t| t as i64),
                        kind: "sol".to_string(),
                        transfer_type: transfer.transfer_type.clone(),
                        from_wallet: Some(transfer.from.clone()),
                        to_wallet: Some(transfer.to.clone()),
                        mint: None,
                        amount_sol: Some(transfer.amount_sol),
                        token_amount_ui: None,
                        token_amount: None,
                        token_decimals: None,
                    },
                );
                Ok(())
            }
        }
    }

    /// Store a token transfer as an event (idempotent per signature+event_index).
    pub async fn store_token_transfer_event(
        &self,
        tx: &EnhancedTransaction,
        transfer: &TokenTransfer,
        event_index: i32,
    ) -> BeastResult<()> {
        let from_wallet = transfer.from_owner.as_deref();
        let to_wallet = transfer.to_owner.as_deref();

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                client
                    .execute(
                        "INSERT INTO transfer_events (
                            signature,
                            event_index,
                            slot,
                            block_time,
                            kind,
                            instruction_index,
                            transfer_type,
                            from_wallet,
                            to_wallet,
                            mint,
                            token_amount,
                            token_decimals,
                            token_amount_ui,
                            from_token_account,
                            to_token_account
                         ) VALUES ($1,$2,$3,$4,'token',$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                         ON CONFLICT (signature, event_index) DO NOTHING",
                        &[
                            &tx.signature,
                            &event_index,
                            &(tx.slot as i64),
                            &tx.block_time.map(|t| t as i64),
                            &(transfer.instruction_index as i32),
                            &transfer.transfer_type,
                            &from_wallet,
                            &to_wallet,
                            &transfer.mint,
                            &(transfer.amount as i64),
                            &(transfer.decimals as i32),
                            &transfer.amount_ui,
                            &transfer.from_token_account,
                            &transfer.to_token_account,
                        ],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to store token transfer event: {}",
                            e
                        ))
                    })?;

                Ok(())
            }
            DatabaseInner::Memory { state } => {
                let mut mem = state.write().await;
                let key = (tx.signature.clone(), event_index);
                if mem.transfer_events.contains_key(&key) {
                    return Ok(());
                }

                mem.transfer_events.insert(
                    key,
                    TransferEvent {
                        signature: tx.signature.clone(),
                        event_index,
                        slot: tx.slot as i64,
                        block_time: tx.block_time.map(|t| t as i64),
                        kind: "token".to_string(),
                        transfer_type: transfer.transfer_type.clone(),
                        from_wallet: transfer.from_owner.clone(),
                        to_wallet: transfer.to_owner.clone(),
                        mint: Some(transfer.mint.clone()),
                        amount_sol: None,
                        token_amount_ui: Some(transfer.amount_ui),
                        token_amount: Some(transfer.amount as i64),
                        token_decimals: Some(transfer.decimals as i32),
                    },
                );
                Ok(())
            }
        }
    }

    /// Find shared inbound funders (wallets that sent to both A and B).
    pub async fn get_shared_inbound_senders(
        &self,
        wallet_a: &str,
        wallet_b: &str,
        since_epoch: Option<u64>,
        limit: usize,
    ) -> BeastResult<Vec<SharedWalletSignal>> {
        let since_epoch = since_epoch.unwrap_or(0) as i64;
        let limit = (limit as i64).clamp(1, 50);

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let rows = client
                    .query(
                        "WITH a AS (
                            SELECT from_wallet,
                                   COUNT(*)::BIGINT AS cnt,
                                   MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                            FROM transfer_events
                            WHERE to_wallet = $1
                              AND from_wallet IS NOT NULL
                              AND (block_time IS NULL OR block_time >= $3)
                            GROUP BY from_wallet
                         ),
                         b AS (
                            SELECT from_wallet,
                                   COUNT(*)::BIGINT AS cnt,
                                   MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                            FROM transfer_events
                            WHERE to_wallet = $2
                              AND from_wallet IS NOT NULL
                              AND (block_time IS NULL OR block_time >= $3)
                            GROUP BY from_wallet
                         )
                         SELECT a.from_wallet,
                                (a.cnt + b.cnt)::BIGINT AS total_cnt,
                                GREATEST(a.last_seen, b.last_seen)::BIGINT AS last_seen
                         FROM a
                         INNER JOIN b ON a.from_wallet = b.from_wallet
                         ORDER BY total_cnt DESC
                         LIMIT $4",
                        &[&wallet_a, &wallet_b, &since_epoch, &limit],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to get shared inbound senders: {}",
                            e
                        ))
                    })?;

                Ok(rows
                    .iter()
                    .map(|row| SharedWalletSignal {
                        wallet: row.get::<_, String>(0),
                        count: row.get::<_, i64>(1) as u64,
                        last_seen_epoch: row.get::<_, i64>(2) as u64,
                    })
                    .collect())
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;

                let mut a: HashMap<String, (u64, u64)> = HashMap::new();
                let mut b: HashMap<String, (u64, u64)> = HashMap::new();

                for ev in mem.transfer_events.values() {
                    if !passes_since(ev.block_time, since_epoch) {
                        continue;
                    }

                    let last_seen = ev.block_time.unwrap_or(0).max(0) as u64;
                    if ev.to_wallet.as_deref() == Some(wallet_a) {
                        let Some(from) = ev.from_wallet.as_ref() else {
                            continue;
                        };
                        let entry = a.entry(from.clone()).or_insert((0, 0));
                        entry.0 += 1;
                        entry.1 = entry.1.max(last_seen);
                    }
                    if ev.to_wallet.as_deref() == Some(wallet_b) {
                        let Some(from) = ev.from_wallet.as_ref() else {
                            continue;
                        };
                        let entry = b.entry(from.clone()).or_insert((0, 0));
                        entry.0 += 1;
                        entry.1 = entry.1.max(last_seen);
                    }
                }

                let mut out: Vec<SharedWalletSignal> = Vec::new();
                for (wallet, (cnt_a, last_a)) in a {
                    let Some((cnt_b, last_b)) = b.get(&wallet) else {
                        continue;
                    };
                    out.push(SharedWalletSignal {
                        wallet,
                        count: cnt_a + *cnt_b,
                        last_seen_epoch: last_a.max(*last_b),
                    });
                }

                out.sort_by(|x, y| y.count.cmp(&x.count));
                out.truncate(limit as usize);
                Ok(out)
            }
        }
    }

    /// Get top counterparties for a wallet from transfer_events.
    pub async fn get_top_counterparties(
        &self,
        wallet: &str,
        since_epoch: Option<u64>,
        limit: usize,
    ) -> BeastResult<Vec<SharedWalletSignal>> {
        let since_epoch = since_epoch.unwrap_or(0) as i64;
        let limit = (limit as i64).clamp(1, 200);

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let rows = client
                    .query(
                        "SELECT
                            CASE WHEN from_wallet = $1 THEN to_wallet ELSE from_wallet END AS counterparty,
                            COUNT(*)::BIGINT AS cnt,
                            MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                         FROM transfer_events
                         WHERE (from_wallet = $1 OR to_wallet = $1)
                           AND from_wallet IS NOT NULL
                           AND to_wallet IS NOT NULL
                           AND (block_time IS NULL OR block_time >= $2)
                         GROUP BY counterparty
                         ORDER BY cnt DESC
                         LIMIT $3",
                        &[&wallet, &since_epoch, &limit],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!("Failed to get top counterparties: {}", e))
                    })?;

                Ok(rows
                    .iter()
                    .filter_map(|row| {
                        let counterparty: Option<String> = row.get(0);
                        counterparty.map(|w| SharedWalletSignal {
                            wallet: w,
                            count: row.get::<_, i64>(1) as u64,
                            last_seen_epoch: row.get::<_, i64>(2) as u64,
                        })
                    })
                    .collect())
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;

                let mut agg: HashMap<String, (u64, u64)> = HashMap::new();
                for ev in mem.transfer_events.values() {
                    if !passes_since(ev.block_time, since_epoch) {
                        continue;
                    }

                    let (Some(from), Some(to)) = (&ev.from_wallet, &ev.to_wallet) else {
                        continue;
                    };
                    if from != wallet && to != wallet {
                        continue;
                    }

                    let counterparty = if from == wallet { to.clone() } else { from.clone() };
                    let last_seen = ev.block_time.unwrap_or(0).max(0) as u64;
                    let entry = agg.entry(counterparty).or_insert((0, 0));
                    entry.0 += 1;
                    entry.1 = entry.1.max(last_seen);
                }

                let mut out: Vec<SharedWalletSignal> = agg
                    .into_iter()
                    .map(|(wallet, (count, last_seen_epoch))| SharedWalletSignal {
                        wallet,
                        count,
                        last_seen_epoch,
                    })
                    .collect();
                out.sort_by(|a, b| {
                    b.count
                        .cmp(&a.count)
                        .then_with(|| b.last_seen_epoch.cmp(&a.last_seen_epoch))
                });
                out.truncate(limit as usize);
                Ok(out)
            }
        }
    }

    /// Get top outbound recipients for a wallet from transfer_events, with volume signals.
    pub async fn get_top_outbound_recipients(
        &self,
        wallet: &str,
        since_epoch: Option<u64>,
        limit: usize,
    ) -> BeastResult<Vec<WalletVolumeSignal>> {
        let since_epoch = since_epoch.unwrap_or(0) as i64;
        let limit = (limit as i64).clamp(1, 200);

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let rows = client
                    .query(
                        "SELECT
                            to_wallet AS counterparty,
                            COUNT(*)::BIGINT AS cnt,
                            SUM(CASE WHEN kind = 'sol' THEN COALESCE(amount_sol, 0.0) ELSE 0.0 END)::DOUBLE PRECISION AS total_sol,
                            SUM(CASE WHEN kind = 'token' THEN COALESCE(token_amount_ui, 0.0) ELSE 0.0 END)::DOUBLE PRECISION AS total_token_ui,
                            MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                         FROM transfer_events
                         WHERE from_wallet = $1
                           AND to_wallet IS NOT NULL
                           AND (block_time IS NULL OR block_time >= $2)
                         GROUP BY to_wallet
                         ORDER BY total_sol DESC, total_token_ui DESC, cnt DESC
                         LIMIT $3",
                        &[&wallet, &since_epoch, &limit],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to get top outbound recipients: {}",
                            e
                        ))
                    })?;

                Ok(rows
                    .iter()
                    .filter_map(|row| {
                        let counterparty: Option<String> = row.get(0);
                        counterparty.map(|w| WalletVolumeSignal {
                            wallet: w,
                            count: row.get::<_, i64>(1) as u64,
                            total_sol: row.get::<_, f64>(2),
                            total_token_ui: row.get::<_, f64>(3),
                            last_seen_epoch: row.get::<_, i64>(4) as u64,
                        })
                    })
                    .collect())
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;

                let mut agg: HashMap<String, (u64, f64, f64, u64)> = HashMap::new();
                for ev in mem.transfer_events.values() {
                    if !passes_since(ev.block_time, since_epoch) {
                        continue;
                    }
                    if ev.from_wallet.as_deref() != Some(wallet) {
                        continue;
                    }
                    let Some(to) = ev.to_wallet.as_ref() else {
                        continue;
                    };

                    let last_seen = ev.block_time.unwrap_or(0).max(0) as u64;
                    let entry = agg.entry(to.clone()).or_insert((0, 0.0, 0.0, 0));
                    entry.0 += 1;
                    entry.3 = entry.3.max(last_seen);

                    match ev.kind.as_str() {
                        "sol" => {
                            if let Some(a) = ev.amount_sol {
                                entry.1 += a.max(0.0);
                            }
                        }
                        "token" => {
                            if let Some(a) = ev.token_amount_ui {
                                entry.2 += a.max(0.0);
                            }
                        }
                        _ => {}
                    }
                }

                let mut out: Vec<WalletVolumeSignal> = agg
                    .into_iter()
                    .map(|(wallet, (count, total_sol, total_token_ui, last_seen_epoch))| {
                        WalletVolumeSignal {
                            wallet,
                            count,
                            total_sol,
                            total_token_ui,
                            last_seen_epoch,
                        }
                    })
                    .collect();

                out.sort_by(|a, b| {
                    b.total_sol
                        .partial_cmp(&a.total_sol)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| {
                            b.total_token_ui
                                .partial_cmp(&a.total_token_ui)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .then_with(|| b.count.cmp(&a.count))
                });
                out.truncate(limit as usize);
                Ok(out)
            }
        }
    }

    /// Get transfer events from one wallet to another (newest first).
    pub async fn get_transfers_between(
        &self,
        from_wallet: &str,
        to_wallet: &str,
        since_epoch: Option<u64>,
        limit: usize,
    ) -> BeastResult<Vec<TransferEvent>> {
        let since_epoch = since_epoch.unwrap_or(0) as i64;
        let limit = (limit as i64).clamp(1, 500);

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let rows = client
                    .query(
                        "SELECT
                            signature,
                            event_index,
                            slot,
                            block_time,
                            kind,
                            transfer_type,
                            from_wallet,
                            to_wallet,
                            mint,
                            amount_sol,
                            token_amount_ui,
                            token_amount,
                            token_decimals
                         FROM transfer_events
                         WHERE from_wallet = $1
                           AND to_wallet = $2
                           AND (block_time IS NULL OR block_time >= $3)
                         ORDER BY COALESCE(block_time, 0) DESC
                         LIMIT $4",
                        &[&from_wallet, &to_wallet, &since_epoch, &limit],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!("Failed to get transfer events: {}", e))
                    })?;

                Ok(rows.iter().map(TransferEvent::from_row).collect())
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;
                let mut out: Vec<TransferEvent> = mem
                    .transfer_events
                    .values()
                    .filter(|ev| passes_since(ev.block_time, since_epoch))
                    .filter(|ev| ev.from_wallet.as_deref() == Some(from_wallet))
                    .filter(|ev| ev.to_wallet.as_deref() == Some(to_wallet))
                    .cloned()
                    .collect();

                out.sort_by(|a, b| {
                    let ta = a.block_time.unwrap_or(0);
                    let tb = b.block_time.unwrap_or(0);
                    tb.cmp(&ta)
                        .then_with(|| b.signature.cmp(&a.signature))
                        .then_with(|| b.event_index.cmp(&a.event_index))
                });
                out.truncate(limit as usize);
                Ok(out)
            }
        }
    }

    /// Get outbound transfer events for a wallet within a time window (oldest first).
    pub async fn get_outbound_transfers_in_window(
        &self,
        from_wallet: &str,
        start_epoch: u64,
        end_epoch: u64,
        limit: usize,
    ) -> BeastResult<Vec<TransferEvent>> {
        let start_epoch = start_epoch as i64;
        let end_epoch = end_epoch as i64;
        let limit = (limit as i64).clamp(1, 2000);

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let rows = client
                    .query(
                        "SELECT
                            signature,
                            event_index,
                            slot,
                            block_time,
                            kind,
                            transfer_type,
                            from_wallet,
                            to_wallet,
                            mint,
                            amount_sol,
                            token_amount_ui,
                            token_amount,
                            token_decimals
                         FROM transfer_events
                         WHERE from_wallet = $1
                           AND to_wallet IS NOT NULL
                           AND block_time IS NOT NULL
                           AND block_time >= $2
                           AND block_time <= $3
                         ORDER BY block_time ASC
                         LIMIT $4",
                        &[&from_wallet, &start_epoch, &end_epoch, &limit],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to get outbound transfer events: {}",
                            e
                        ))
                    })?;

                Ok(rows.iter().map(TransferEvent::from_row).collect())
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;

                let mut out: Vec<TransferEvent> = mem
                    .transfer_events
                    .values()
                    .filter(|ev| ev.from_wallet.as_deref() == Some(from_wallet))
                    .filter(|ev| ev.to_wallet.is_some())
                    .filter(|ev| {
                        let Some(bt) = ev.block_time else {
                            return false;
                        };
                        bt >= start_epoch && bt <= end_epoch
                    })
                    .cloned()
                    .collect();

                out.sort_by(|a, b| {
                    let ta = a.block_time.unwrap_or(0);
                    let tb = b.block_time.unwrap_or(0);
                    ta.cmp(&tb)
                        .then_with(|| a.signature.cmp(&b.signature))
                        .then_with(|| a.event_index.cmp(&b.event_index))
                });
                out.truncate(limit as usize);
                Ok(out)
            }
        }
    }

    pub async fn get_wallet_connections(&self, wallet_address: &str) -> BeastResult<Vec<WalletConnection>> {
        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let rows = client
                    .query(
                        "SELECT
                            from_wallet,
                            to_wallet,
                            sol_amount,
                            token_amount,
                            transaction_count,
                            EXTRACT(EPOCH FROM first_seen)::BIGINT AS first_seen_epoch,
                            EXTRACT(EPOCH FROM last_seen)::BIGINT AS last_seen_epoch
                         FROM wallet_relationships
                         WHERE from_wallet = $1 OR to_wallet = $1
                         ORDER BY transaction_count DESC
                         LIMIT 100",
                        &[&wallet_address],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!("Failed to get connections: {}", e))
                    })?;

                Ok(rows
                    .iter()
                    .map(|row| WalletConnection {
                        from_wallet: row.get(0),
                        to_wallet: row.get(1),
                        total_sol_transferred: row.get(2),
                        total_token_transferred: row.get::<_, i64>(3) as u64,
                        transaction_count: row.get::<_, i32>(4) as u32,
                        first_seen_epoch: row.get::<_, i64>(5) as u64,
                        last_seen_epoch: row.get::<_, i64>(6) as u64,
                    })
                    .collect())
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;
                let mut out: Vec<WalletConnection> = mem
                    .relationships
                    .values()
                    .filter(|rel| rel.from_wallet == wallet_address || rel.to_wallet == wallet_address)
                    .map(|rel| WalletConnection {
                        from_wallet: rel.from_wallet.clone(),
                        to_wallet: rel.to_wallet.clone(),
                        total_sol_transferred: rel.sol_amount,
                        total_token_transferred: rel.token_amount,
                        transaction_count: rel.transaction_count,
                        first_seen_epoch: rel.first_seen_epoch,
                        last_seen_epoch: rel.last_seen_epoch,
                    })
                    .collect();

                out.sort_by(|a, b| b.transaction_count.cmp(&a.transaction_count));
                out.truncate(100);
                Ok(out)
            }
        }
    }

    /// Get behavioral profile for a wallet from transfer_events.
    pub async fn get_behavioral_profile(
        &self,
        wallet: &str,
        since_epoch: Option<u64>,
    ) -> BeastResult<Option<BehavioralProfile>> {
        let since = since_epoch.unwrap_or(0) as i64;
        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let row = client
                    .query_opt(
                        "WITH wallet_transfers AS (
                            SELECT
                                COALESCE(amount_sol, 0.0) AS sol,
                                block_time,
                                EXTRACT(HOUR FROM TO_TIMESTAMP(block_time)) AS hour_utc
                            FROM transfer_events
                            WHERE (from_wallet = $1 OR to_wallet = $1)
                              AND kind = 'sol'
                              AND amount_sol > 0.0
                              AND (block_time IS NULL OR block_time >= $2)
                        ),
                        time_bounds AS (
                            SELECT
                                MIN(block_time)::BIGINT AS first_tx,
                                MAX(block_time)::BIGINT AS last_tx
                            FROM wallet_transfers
                        ),
                        stats AS (
                            SELECT
                                COUNT(*)::BIGINT AS total_transfers,
                                AVG(sol)::DOUBLE PRECISION AS avg_sol,
                                PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY sol)::DOUBLE PRECISION AS median_sol,
                                MODE() WITHIN GROUP (ORDER BY hour_utc)::INTEGER AS most_active_hour
                            FROM wallet_transfers
                        )
                        SELECT
                            COALESCE(s.total_transfers, 0)::BIGINT AS total_transfers,
                            COALESCE(s.avg_sol, 0.0)::DOUBLE PRECISION AS avg_sol,
                            COALESCE(s.median_sol, 0.0)::DOUBLE PRECISION AS median_sol,
                            COALESCE(s.most_active_hour, -1)::INTEGER AS most_active_hour,
                            COALESCE(t.first_tx, 0)::BIGINT AS first_tx,
                            COALESCE(t.last_tx, 0)::BIGINT AS last_tx
                        FROM stats s
                        CROSS JOIN time_bounds t
                        WHERE s.total_transfers > 0",
                        &[&wallet, &since],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to get behavioral profile: {}",
                            e
                        ))
                    })?;

                let Some(row) = row else {
                    return Ok(None);
                };

                let total_transfers: i64 = row.get(0);
                if total_transfers <= 0 {
                    return Ok(None);
                }

                let avg_sol: f64 = row.get(1);
                let median_sol: f64 = row.get(2);
                let most_active_hour: i32 = row.get(3);
                let first_tx: i64 = row.get(4);
                let last_tx: i64 = row.get(5);

                let days_active = if last_tx > first_tx && first_tx > 0 {
                    ((last_tx - first_tx) / 86_400).max(1) as u32
                } else {
                    1
                };
                let avg_tx_per_day = total_transfers as f64 / days_active as f64;

                Ok(Some(BehavioralProfile {
                    wallet: wallet.to_string(),
                    total_transfers: total_transfers as u64,
                    avg_sol_per_tx: avg_sol,
                    median_sol_per_tx: median_sol,
                    total_days_active: days_active,
                    avg_tx_per_day,
                    most_active_hour_utc: if most_active_hour >= 0 {
                        Some(most_active_hour)
                    } else {
                        None
                    },
                    first_tx_epoch: first_tx as u64,
                    last_tx_epoch: last_tx as u64,
                }))
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;

                let mut sol_amounts: Vec<f64> = Vec::new();
                let mut hour_counts: HashMap<i32, u64> = HashMap::new();
                let mut first_tx: i64 = i64::MAX;
                let mut last_tx: i64 = 0;

                for ev in mem.transfer_events.values() {
                    if ev.kind != "sol" {
                        continue;
                    }
                    if !passes_since(ev.block_time, since) {
                        continue;
                    }
                    if ev.from_wallet.as_deref() != Some(wallet)
                        && ev.to_wallet.as_deref() != Some(wallet)
                    {
                        continue;
                    }
                    let Some(sol) = ev.amount_sol else {
                        continue;
                    };
                    if sol <= 0.0 {
                        continue;
                    }

                    sol_amounts.push(sol);

                    if let Some(bt) = ev.block_time {
                        first_tx = first_tx.min(bt);
                        last_tx = last_tx.max(bt);
                        // bt is in UTC epoch seconds.
                        let hour = ((bt.rem_euclid(86_400)) / 3600) as i32;
                        *hour_counts.entry(hour).or_insert(0) += 1;
                    }
                }

                if sol_amounts.is_empty() {
                    return Ok(None);
                }

                let total_transfers = sol_amounts.len() as u64;
                let sum: f64 = sol_amounts.iter().sum();
                let avg_sol = sum / total_transfers as f64;

                sol_amounts.sort_by(|a, b| {
                    a.partial_cmp(b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let median_sol = if sol_amounts.len() % 2 == 1 {
                    sol_amounts[sol_amounts.len() / 2]
                } else {
                    let hi = sol_amounts.len() / 2;
                    let lo = hi - 1;
                    (sol_amounts[lo] + sol_amounts[hi]) / 2.0
                };

                let most_active_hour_utc = {
                    let mut best: Option<(i32, u64)> = None;
                    for (hour, cnt) in hour_counts {
                        match best {
                            None => best = Some((hour, cnt)),
                            Some((best_hour, best_cnt)) => {
                                if cnt > best_cnt || (cnt == best_cnt && hour < best_hour) {
                                    best = Some((hour, cnt));
                                }
                            }
                        }
                    }
                    best.map(|(h, _)| h)
                };

                let first_tx_epoch = if first_tx == i64::MAX {
                    0
                } else {
                    first_tx.max(0) as u64
                };
                let last_tx_epoch = last_tx.max(0) as u64;

                let days_active = if last_tx > first_tx && first_tx > 0 {
                    ((last_tx - first_tx) / 86_400).max(1) as u32
                } else {
                    1
                };
                let avg_tx_per_day = total_transfers as f64 / days_active as f64;

                Ok(Some(BehavioralProfile {
                    wallet: wallet.to_string(),
                    total_transfers,
                    avg_sol_per_tx: avg_sol,
                    median_sol_per_tx: median_sol,
                    total_days_active: days_active,
                    avg_tx_per_day,
                    most_active_hour_utc,
                    first_tx_epoch,
                    last_tx_epoch,
                }))
            }
        }
    }

    /// Detect temporal overlap between two wallets (synchronized activity).
    pub async fn get_temporal_overlap(
        &self,
        wallet_a: &str,
        wallet_b: &str,
        since_epoch: Option<u64>,
        time_window_minutes: u32,
    ) -> BeastResult<TemporalOverlap> {
        let since = since_epoch.unwrap_or(0) as i64;
        let window_secs = (time_window_minutes.clamp(1, 60) * 60) as i64;

        match &self.inner {
            DatabaseInner::Postgres { client } => {
                let row = client
                    .query_one(
                        "WITH a_times AS (
                            SELECT DISTINCT (block_time / $4)::BIGINT AS time_bucket
                            FROM transfer_events
                            WHERE (from_wallet = $1 OR to_wallet = $1)
                              AND block_time IS NOT NULL
                              AND block_time >= $3
                        ),
                        b_times AS (
                            SELECT DISTINCT (block_time / $4)::BIGINT AS time_bucket
                            FROM transfer_events
                            WHERE (from_wallet = $2 OR to_wallet = $2)
                              AND block_time IS NOT NULL
                              AND block_time >= $3
                        ),
                        overlap AS (
                            SELECT COUNT(*)::INTEGER AS overlap_count
                            FROM a_times
                            INNER JOIN b_times ON a_times.time_bucket = b_times.time_bucket
                        ),
                        same_block AS (
                            SELECT COUNT(DISTINCT a.signature)::INTEGER AS same_block_count
                            FROM transfer_events a
                            INNER JOIN transfer_events b
                                ON a.slot = b.slot
                                AND a.signature != b.signature
                            WHERE (a.from_wallet = $1 OR a.to_wallet = $1)
                              AND (b.from_wallet = $2 OR b.to_wallet = $2)
                              AND a.block_time >= $3
                              AND b.block_time >= $3
                        )
                        SELECT
                            COALESCE(o.overlap_count, 0)::INTEGER AS overlapping_minutes,
                            (SELECT COUNT(*)::INTEGER FROM a_times) + (SELECT COUNT(*)::INTEGER FROM b_times) AS total_minutes,
                            COALESCE(s.same_block_count, 0)::INTEGER AS same_block_count
                        FROM overlap o
                        CROSS JOIN same_block s",
                        &[&wallet_a, &wallet_b, &since, &window_secs],
                    )
                    .await
                    .map_err(|e| {
                        BeastError::DatabaseError(format!(
                            "Failed to get temporal overlap: {}",
                            e
                        ))
                    })?;

                let overlapping_minutes: i32 = row.get(0);
                let total_minutes: i32 = row.get(1);
                let same_block_count: i32 = row.get(2);

                let overlap_ratio = if total_minutes > 0 {
                    (overlapping_minutes as f64) / (total_minutes as f64)
                } else {
                    0.0
                };

                Ok(TemporalOverlap {
                    overlapping_minutes: overlapping_minutes as u32,
                    total_minutes_checked: total_minutes as u32,
                    overlap_ratio,
                    same_block_count: same_block_count as u32,
                })
            }
            DatabaseInner::Memory { state } => {
                let mem = state.read().await;

                let mut a_times: HashSet<i64> = HashSet::new();
                let mut b_times: HashSet<i64> = HashSet::new();

                let mut a_events: Vec<&TransferEvent> = Vec::new();
                let mut b_events: Vec<&TransferEvent> = Vec::new();

                for ev in mem.transfer_events.values() {
                    let Some(bt) = ev.block_time else {
                        continue;
                    };
                    if bt < since {
                        continue;
                    }

                    let involved_a = ev.from_wallet.as_deref() == Some(wallet_a)
                        || ev.to_wallet.as_deref() == Some(wallet_a);
                    let involved_b = ev.from_wallet.as_deref() == Some(wallet_b)
                        || ev.to_wallet.as_deref() == Some(wallet_b);

                    if involved_a {
                        a_times.insert(bt / window_secs);
                        a_events.push(ev);
                    }
                    if involved_b {
                        b_times.insert(bt / window_secs);
                        b_events.push(ev);
                    }
                }

                let overlap_count = a_times.intersection(&b_times).count() as u32;
                let total_minutes = (a_times.len() + b_times.len()) as u32;
                let overlap_ratio = if total_minutes > 0 {
                    overlap_count as f64 / total_minutes as f64
                } else {
                    0.0
                };

                let mut slot_to_b_sigs: HashMap<i64, HashSet<String>> = HashMap::new();
                for ev in b_events.iter() {
                    slot_to_b_sigs
                        .entry(ev.slot)
                        .or_default()
                        .insert(ev.signature.clone());
                }

                let mut same_sigs: HashSet<String> = HashSet::new();
                for ev in a_events.iter() {
                    let Some(b_sigs) = slot_to_b_sigs.get(&ev.slot) else {
                        continue;
                    };
                    // Any signature in that slot that isn't this one?
                    if b_sigs.iter().any(|sig| sig != &ev.signature) {
                        same_sigs.insert(ev.signature.clone());
                    }
                }

                Ok(TemporalOverlap {
                    overlapping_minutes: overlap_count,
                    total_minutes_checked: total_minutes,
                    overlap_ratio,
                    same_block_count: same_sigs.len() as u32,
                })
            }
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct WalletConnection {
    pub from_wallet: String,
    pub to_wallet: String,
    pub total_sol_transferred: f64,
    pub total_token_transferred: u64,
    pub transaction_count: u32,
    pub first_seen_epoch: u64,
    pub last_seen_epoch: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SharedWalletSignal {
    pub wallet: String,
    pub count: u64,
    pub last_seen_epoch: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletVolumeSignal {
    pub wallet: String,
    pub count: u64,
    pub total_sol: f64,
    pub total_token_ui: f64,
    pub last_seen_epoch: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BehavioralProfile {
    pub wallet: String,
    pub total_transfers: u64,
    pub avg_sol_per_tx: f64,
    pub median_sol_per_tx: f64,
    pub total_days_active: u32,
    pub avg_tx_per_day: f64,
    pub most_active_hour_utc: Option<i32>,
    pub first_tx_epoch: u64,
    pub last_tx_epoch: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TemporalOverlap {
    pub overlapping_minutes: u32,
    pub total_minutes_checked: u32,
    pub overlap_ratio: f64,
    pub same_block_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TransferEvent {
    pub signature: String,
    pub event_index: i32,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub kind: String, // "sol" | "token"
    pub transfer_type: String,
    pub from_wallet: Option<String>,
    pub to_wallet: Option<String>,
    pub mint: Option<String>,
    pub amount_sol: Option<f64>,
    pub token_amount_ui: Option<f64>,
    pub token_amount: Option<i64>,
    pub token_decimals: Option<i32>,
}

impl TransferEvent {
    fn from_row(row: &Row) -> Self {
        TransferEvent {
            signature: row.get::<_, String>(0),
            event_index: row.get::<_, i32>(1),
            slot: row.get::<_, i64>(2),
            block_time: row.get::<_, Option<i64>>(3),
            kind: row.get::<_, String>(4),
            transfer_type: row.get::<_, String>(5),
            from_wallet: row.get::<_, Option<String>>(6),
            to_wallet: row.get::<_, Option<String>>(7),
            mint: row.get::<_, Option<String>>(8),
            amount_sol: row.get::<_, Option<f64>>(9),
            token_amount_ui: row.get::<_, Option<f64>>(10),
            token_amount: row.get::<_, Option<i64>>(11),
            token_decimals: row.get::<_, Option<i32>>(12),
        }
    }
}

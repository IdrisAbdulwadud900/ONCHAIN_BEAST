use crate::core::enhanced_parser::{EnhancedTransaction, SolTransfer, TokenTransfer};
use crate::core::errors::{BeastError, BeastResult};
use crate::modules::pattern_detector::PatternAnalysisResult;
use crate::modules::transaction_graph_builder::FundFlowGraph;
use serde_json;
/// PostgreSQL Database Layer
/// Persistent storage for transactions, analyses, and relationships
use tokio_postgres::{Client, NoTls};

/// Database manager
pub struct DatabaseManager {
    client: Client,
}

impl DatabaseManager {
    /// Create new database manager
    pub async fn new(database_url: &str) -> BeastResult<Self> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to connect: {}", e)))?;

        // Spawn connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Database connection error: {}", e);
            }
        });

        Ok(Self { client })
    }

    /// Initialize database schema
    pub async fn init_schema(&self) -> BeastResult<()> {
        // Create transactions table
        self.client
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

        // Create indexes on transactions
        self.client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot)",
                &[],
            )
            .await
            .ok();
        self.client
            .execute("CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time)", &[])
            .await
            .ok();

        // Create wallet_analyses table
        self.client
            .execute(
                "CREATE TABLE IF NOT EXISTS wallet_analyses (
                    id SERIAL PRIMARY KEY,
                    wallet_address TEXT NOT NULL,
                    transaction_count INTEGER NOT NULL,
                    total_sol_in DOUBLE PRECISION NOT NULL DEFAULT 0,
                    total_sol_out DOUBLE PRECISION NOT NULL DEFAULT 0,
                    total_token_transferred BIGINT NOT NULL DEFAULT 0,
                    risk_level TEXT NOT NULL,
                    confidence_score DOUBLE PRECISION NOT NULL,
                    fund_flow_graph JSONB,
                    pattern_analysis JSONB,
                    analyzed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )",
                &[],
            )
            .await
            .map_err(|e| {
                BeastError::DatabaseError(format!("Failed to create wallet_analyses table: {}", e))
            })?;

        // Create indexes on wallet_analyses
        self.client
            .execute("CREATE INDEX IF NOT EXISTS idx_wallet_analyses_address ON wallet_analyses(wallet_address)", &[])
            .await
            .ok();
        self.client
            .execute("CREATE INDEX IF NOT EXISTS idx_wallet_analyses_risk ON wallet_analyses(risk_level)", &[])
            .await
            .ok();
        self.client
            .execute("CREATE INDEX IF NOT EXISTS idx_wallet_analyses_time ON wallet_analyses(analyzed_at DESC)", &[])
            .await
            .ok();

        // Create wallet_relationships table
        // Note: Some existing deployments use `sol_amount`/`token_amount` column names.
        // We keep that schema to remain compatible with already-initialized databases.
        self.client
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

        // Create indexes on wallet_relationships
        self.client
            .execute("CREATE INDEX IF NOT EXISTS idx_wallet_relationships_from ON wallet_relationships(from_wallet)", &[])
            .await
            .ok();
        self.client
            .execute("CREATE INDEX IF NOT EXISTS idx_wallet_relationships_to ON wallet_relationships(to_wallet)", &[])
            .await
            .ok();

        // Create transfer_events table (event-level transfers)
        self.client
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
                BeastError::DatabaseError(format!(
                    "Failed to create transfer_events table: {}",
                    e
                ))
            })?;

        // Create indexes on transfer_events
        self.client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_signature ON transfer_events(signature)",
                &[],
            )
            .await
            .ok();
        self.client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_from_wallet ON transfer_events(from_wallet)",
                &[],
            )
            .await
            .ok();
        self.client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_to_wallet ON transfer_events(to_wallet)",
                &[],
            )
            .await
            .ok();
        self.client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_transfer_events_block_time ON transfer_events(block_time)",
                &[],
            )
            .await
            .ok();

        Ok(())
    }

    /// Store transaction
    pub async fn store_transaction(&self, tx: &EnhancedTransaction) -> BeastResult<()> {
        let data = serde_json::to_value(tx).map_err(|e| {
            BeastError::DatabaseError(format!("Failed to serialize transaction: {}", e))
        })?;

        self.client
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
            .map_err(|e| BeastError::DatabaseError(format!("Failed to store transaction: {}", e)))?;

        Ok(())
    }

    /// Get transaction by signature
    pub async fn get_transaction(
        &self,
        signature: &str,
    ) -> BeastResult<Option<EnhancedTransaction>> {
        let row = self
            .client
            .query_opt(
                "SELECT data::text FROM transactions WHERE signature = $1",
                &[&signature],
            )
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to get transaction: {}", e)))?;

        if let Some(row) = row {
            let data_str: String = row.get(0);
            let data: serde_json::Value = serde_json::from_str(&data_str)
                .map_err(|e| BeastError::DatabaseError(format!("Failed to parse JSON: {}", e)))?;
            let tx: EnhancedTransaction = serde_json::from_value(data).map_err(|e| {
                BeastError::DatabaseError(format!("Failed to deserialize transaction: {}", e))
            })?;
            Ok(Some(tx))
        } else {
            Ok(None)
        }
    }

    /// Store wallet analysis
    pub async fn store_wallet_analysis(
        &self,
        wallet_address: &str,
        graph: &FundFlowGraph,
        patterns: &PatternAnalysisResult,
    ) -> BeastResult<()> {
        let graph_json = serde_json::to_value(graph)
            .map_err(|e| BeastError::DatabaseError(format!("Failed to serialize graph: {}", e)))?;
        let patterns_json = serde_json::to_value(patterns).map_err(|e| {
            BeastError::DatabaseError(format!("Failed to serialize patterns: {}", e))
        })?;

        let total_sol_in: f64 = graph
            .wallets
            .iter()
            .filter(|w| w.address == wallet_address)
            .map(|w| w.total_received_sol)
            .sum();

        let total_sol_out: f64 = graph
            .wallets
            .iter()
            .filter(|w| w.address == wallet_address)
            .map(|w| w.total_sent_sol)
            .sum();

        let risk_level = format!("{:?}", patterns.overall_risk_level);

        self.client
            .execute(
                "INSERT INTO wallet_analyses (wallet_address, transaction_count, total_sol_in, total_sol_out, 
                 total_token_transferred, risk_level, confidence_score, fund_flow_graph, pattern_analysis)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::jsonb)",
                &[
                    &wallet_address,
                    &(graph.transaction_count as i32),
                    &total_sol_in,
                    &total_sol_out,
                    &(graph.total_volume_tokens as i64),
                    &risk_level,
                    &patterns.confidence_score,
                    &graph_json.to_string(),
                    &patterns_json.to_string(),
                ],
            )
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to store analysis: {}", e)))?;

        Ok(())
    }

    /// Get latest wallet analysis
    pub async fn get_latest_wallet_analysis(
        &self,
        wallet_address: &str,
    ) -> BeastResult<Option<(FundFlowGraph, PatternAnalysisResult)>> {
        let row = self
            .client
            .query_opt(
                "SELECT fund_flow_graph, pattern_analysis FROM wallet_analyses 
                 WHERE wallet_address = $1 
                 ORDER BY analyzed_at DESC LIMIT 1",
                &[&wallet_address],
            )
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to get analysis: {}", e)))?;

        if let Some(row) = row {
            let graph_str: String = row.get(0);
            let patterns_str: String = row.get(1);

            let graph_json: serde_json::Value = serde_json::from_str(&graph_str).map_err(|e| {
                BeastError::DatabaseError(format!("Failed to parse graph JSON: {}", e))
            })?;
            let patterns_json: serde_json::Value =
                serde_json::from_str(&patterns_str).map_err(|e| {
                    BeastError::DatabaseError(format!("Failed to parse patterns JSON: {}", e))
                })?;

            let graph: FundFlowGraph = serde_json::from_value(graph_json).map_err(|e| {
                BeastError::DatabaseError(format!("Failed to deserialize graph: {}", e))
            })?;
            let patterns: PatternAnalysisResult =
                serde_json::from_value(patterns_json).map_err(|e| {
                    BeastError::DatabaseError(format!("Failed to deserialize patterns: {}", e))
                })?;

            Ok(Some((graph, patterns)))
        } else {
            Ok(None)
        }
    }

    /// Store wallet relationship
    pub async fn store_wallet_relationship(
        &self,
        from_wallet: &str,
        to_wallet: &str,
        sol_amount: f64,
        token_amount: u64,
    ) -> BeastResult<()> {
        self.client
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
            .map_err(|e| BeastError::DatabaseError(format!("Failed to store relationship: {}", e)))?;

        Ok(())
    }

    /// Store a SOL transfer as an event (idempotent per signature+event_index)
    pub async fn store_sol_transfer_event(
        &self,
        tx: &EnhancedTransaction,
        transfer: &SolTransfer,
        event_index: i32,
    ) -> BeastResult<()> {
        self.client
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
            .map_err(|e| BeastError::DatabaseError(format!("Failed to store SOL transfer event: {}", e)))?;

        Ok(())
    }

    /// Store a token transfer as an event (idempotent per signature+event_index)
    pub async fn store_token_transfer_event(
        &self,
        tx: &EnhancedTransaction,
        transfer: &TokenTransfer,
        event_index: i32,
    ) -> BeastResult<()> {
        let from_wallet = transfer.from_owner.as_deref();
        let to_wallet = transfer.to_owner.as_deref();

        self.client
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
                BeastError::DatabaseError(format!("Failed to store token transfer event: {}", e))
            })?;

        Ok(())
    }

    /// Find shared inbound funders (wallets that sent to both A and B)
    pub async fn get_shared_inbound_senders(
        &self,
        wallet_a: &str,
        wallet_b: &str,
        since_epoch: Option<u64>,
        limit: usize,
    ) -> BeastResult<Vec<SharedWalletSignal>> {
        let since_epoch = since_epoch.unwrap_or(0) as i64;
        let limit = (limit as i64).clamp(1, 50);

        let rows = self
            .client
            .query(
                "WITH a AS (
                    SELECT from_wallet,
                           COUNT(*)::BIGINT AS cnt,
                           MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                    FROM transfer_events
                    WHERE to_wallet = $1
                      AND from_wallet IS NOT NULL
                     AND COALESCE(block_time, 0) >= $3
                    GROUP BY from_wallet
                 ),
                 b AS (
                    SELECT from_wallet,
                           COUNT(*)::BIGINT AS cnt,
                           MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                    FROM transfer_events
                    WHERE to_wallet = $2
                      AND from_wallet IS NOT NULL
                     AND COALESCE(block_time, 0) >= $3
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
                BeastError::DatabaseError(format!("Failed to get shared inbound senders: {}", e))
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

    /// Get top counterparties for a wallet from transfer_events
    pub async fn get_top_counterparties(
        &self,
        wallet: &str,
        since_epoch: Option<u64>,
        limit: usize,
    ) -> BeastResult<Vec<SharedWalletSignal>> {
        let since_epoch = since_epoch.unwrap_or(0) as i64;
        let limit = (limit as i64).clamp(1, 200);

        let rows = self
            .client
            .query(
                "SELECT
                    CASE WHEN from_wallet = $1 THEN to_wallet ELSE from_wallet END AS counterparty,
                    COUNT(*)::BIGINT AS cnt,
                    MAX(COALESCE(block_time, 0))::BIGINT AS last_seen
                 FROM transfer_events
                 WHERE (from_wallet = $1 OR to_wallet = $1)
                   AND from_wallet IS NOT NULL
                   AND to_wallet IS NOT NULL
                                     AND COALESCE(block_time, 0) >= $2
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
                counterparty.map(|wallet| SharedWalletSignal {
                    wallet,
                    count: row.get::<_, i64>(1) as u64,
                    last_seen_epoch: row.get::<_, i64>(2) as u64,
                })
            })
            .collect())
    }

    /// Get wallet connections
    pub async fn get_wallet_connections(
        &self,
        wallet_address: &str,
    ) -> BeastResult<Vec<WalletConnection>> {
        let rows = self
            .client
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
            .map_err(|e| BeastError::DatabaseError(format!("Failed to get connections: {}", e)))?;

        let connections = rows
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
            .collect();

        Ok(connections)
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> BeastResult<DatabaseStats> {
        let tx_count = self
            .client
            .query_one("SELECT COUNT(*) FROM transactions", &[])
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to count transactions: {}", e)))?
            .get::<_, i64>(0) as usize;

        let analysis_count = self
            .client
            .query_one("SELECT COUNT(*) FROM wallet_analyses", &[])
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to count analyses: {}", e)))?
            .get::<_, i64>(0) as usize;

        let relationship_count = self
            .client
            .query_one("SELECT COUNT(*) FROM wallet_relationships", &[])
            .await
            .map_err(|e| {
                BeastError::DatabaseError(format!("Failed to count relationships: {}", e))
            })?
            .get::<_, i64>(0) as usize;

        Ok(DatabaseStats {
            transaction_count: tx_count,
            wallet_analysis_count: analysis_count,
            wallet_relationship_count: relationship_count,
        })
    }

    /// Health check
    pub async fn health_check(&self) -> BeastResult<bool> {
        self.client
            .query_one("SELECT 1", &[])
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Health check failed: {}", e)))?;
        Ok(true)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DatabaseStats {
    pub transaction_count: usize,
    pub wallet_analysis_count: usize,
    pub wallet_relationship_count: usize,
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

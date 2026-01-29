use crate::core::enhanced_parser::EnhancedTransaction;
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

    /// Get wallet connections
    pub async fn get_wallet_connections(
        &self,
        wallet_address: &str,
    ) -> BeastResult<Vec<WalletConnection>> {
        let rows = self
            .client
            .query(
                "SELECT from_wallet, to_wallet, sol_amount, token_amount, transaction_count
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
}

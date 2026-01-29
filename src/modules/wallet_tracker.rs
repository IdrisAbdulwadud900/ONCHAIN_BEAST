/// WalletTracker: Core module for tracking and clustering connected wallets
///
/// Responsibilities:
/// - Find wallets connected to a main wallet
/// - Identify side wallets and alternate addresses
/// - Track wallet relationships and fund flows
/// - Detect wallet clustering patterns
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Wallet {
    pub address: String,
    pub balance: u64,
    pub transaction_count: u64,
    pub first_seen: u64,
    pub last_seen: u64,
    pub risk_score: f64,
}

#[derive(Debug)]
pub struct WalletTracker {
    wallets: HashMap<String, Wallet>,
    relationships: HashMap<String, Vec<String>>, // address -> connected addresses
}

impl WalletTracker {
    pub fn new() -> Self {
        WalletTracker {
            wallets: HashMap::new(),
            relationships: HashMap::new(),
        }
    }

    /// Add a wallet to tracking
    pub fn add_wallet(&mut self, wallet: Wallet) {
        self.wallets.insert(wallet.address.clone(), wallet);
    }

    /// Find wallets potentially belonging to the same entity
    pub fn find_connected_wallets(&self, wallet_address: &str) -> HashSet<String> {
        let mut connected = HashSet::new();
        let mut to_visit = vec![wallet_address.to_string()];

        while let Some(current) = to_visit.pop() {
            if connected.insert(current.clone()) {
                if let Some(related) = self.relationships.get(&current) {
                    to_visit.extend(related.iter().cloned());
                }
            }
        }

        connected
    }

    /// Analyze wallet clustering based on transaction patterns
    pub fn cluster_wallets(&self) -> Vec<Vec<String>> {
        // Implement clustering using connected components algorithm
        let mut visited = std::collections::HashSet::new();
        let mut clusters = Vec::new();

        for wallet_address in self.wallets.keys() {
            if !visited.contains(wallet_address) {
                let cluster = self.find_connected_wallets(wallet_address);
                if !cluster.is_empty() {
                    visited.extend(cluster.iter().cloned());
                    clusters.push(cluster.into_iter().collect());
                }
            }
        }

        clusters
    }

    /// Get wallet by address
    pub fn get_wallet(&self, address: &str) -> Option<&Wallet> {
        self.wallets.get(address)
    }
}

impl Default for WalletTracker {
    fn default() -> Self {
        Self::new()
    }
}

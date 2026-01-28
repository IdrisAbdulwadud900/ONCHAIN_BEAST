/// Response Caching Layer
///
/// Provides in-memory caching for RPC responses to reduce costs and improve performance

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Cache entry with expiration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub data: T,
    pub expires_at: DateTime<Utc>,
}

impl<T> CacheEntry<T> {
    /// Check if cache entry is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Response cache with TTL support
#[derive(Clone)]
pub struct ResponseCache<T> {
    cache: Arc<DashMap<String, CacheEntry<T>>>,
    default_ttl: Duration,
}

impl<T: Clone> ResponseCache<T> {
    /// Create new cache with default TTL
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            default_ttl: Duration::seconds(ttl_seconds),
        }
    }

    /// Get value from cache
    pub fn get(&self, key: &str) -> Option<T> {
        if let Some(entry) = self.cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            } else {
                // Remove expired entry
                drop(entry);
                self.cache.remove(key);
            }
        }
        None
    }

    /// Set value in cache with default TTL
    pub fn set(&self, key: String, value: T) {
        self.set_with_ttl(key, value, self.default_ttl);
    }

    /// Set value in cache with custom TTL
    pub fn set_with_ttl(&self, key: String, value: T, ttl: Duration) {
        let expires_at = Utc::now() + ttl;
        self.cache.insert(
            key,
            CacheEntry {
                data: value,
                expires_at,
            },
        );
    }

    /// Invalidate cache entry
    pub fn invalidate(&self, key: &str) {
        self.cache.remove(key);
    }

    /// Clear all expired entries
    pub fn cleanup(&self) {
        self.cache.retain(|_, entry| !entry.is_expired());
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        self.cache.len()
    }

    /// Clear entire cache
    pub fn clear(&self) {
        self.cache.clear();
    }
}

/// Cache manager for different data types
pub struct CacheManager {
    /// Account info cache (5 minute TTL)
    pub account_cache: ResponseCache<serde_json::Value>,
    /// Transaction cache (10 minute TTL - transactions don't change)
    pub transaction_cache: ResponseCache<serde_json::Value>,
    /// Cluster info cache (1 minute TTL)
    pub cluster_cache: ResponseCache<serde_json::Value>,
    /// Signature list cache (2 minute TTL)
    pub signature_cache: ResponseCache<serde_json::Value>,
}

impl CacheManager {
    /// Create new cache manager with default TTLs
    pub fn new() -> Self {
        Self {
            account_cache: ResponseCache::new(300),      // 5 minutes
            transaction_cache: ResponseCache::new(600),  // 10 minutes
            cluster_cache: ResponseCache::new(60),       // 1 minute
            signature_cache: ResponseCache::new(120),    // 2 minutes
        }
    }

    /// Cleanup all expired entries
    pub fn cleanup_all(&self) {
        self.account_cache.cleanup();
        self.transaction_cache.cleanup();
        self.cluster_cache.cleanup();
        self.signature_cache.cleanup();
    }

    /// Get total cache size
    pub fn total_size(&self) -> usize {
        self.account_cache.size()
            + self.transaction_cache.size()
            + self.cluster_cache.size()
            + self.signature_cache.size()
    }

    /// Clear all caches
    pub fn clear_all(&self) {
        self.account_cache.clear();
        self.transaction_cache.clear();
        self.cluster_cache.clear();
        self.signature_cache.clear();
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_set_get() {
        let cache = ResponseCache::<String>::new(60);
        cache.set("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
    }

    #[test]
    fn test_cache_expiration() {
        let cache = ResponseCache::<String>::new(1);
        cache.set("key1".to_string(), "value1".to_string());
        
        // Immediate get should work
        assert_eq!(cache.get("key1"), Some("value1".to_string()));
        
        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert_eq!(cache.get("key1"), None);
    }

    #[test]
    fn test_cache_invalidate() {
        let cache = ResponseCache::<String>::new(60);
        cache.set("key1".to_string(), "value1".to_string());
        cache.invalidate("key1");
        assert_eq!(cache.get("key1"), None);
    }
}

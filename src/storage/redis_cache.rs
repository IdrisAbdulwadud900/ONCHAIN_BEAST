use crate::core::errors::{BeastError, BeastResult};
/// Redis Caching Layer
/// High-performance distributed caching for transactions and analysis results
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Redis cache manager
pub struct RedisCache {
    client: ConnectionManager,
    default_ttl: usize, // seconds
}

impl RedisCache {
    /// Create new Redis cache manager
    pub async fn new(redis_url: &str) -> BeastResult<Self> {
        let client = Client::open(redis_url).map_err(|e| {
            BeastError::DatabaseError(format!("Failed to create Redis client: {}", e))
        })?;

        let connection = ConnectionManager::new(client)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self {
            client: connection,
            default_ttl: 3600, // 1 hour default
        })
    }

    /// Set value with default TTL
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> BeastResult<()> {
        self.set_with_ttl(key, value, self.default_ttl).await
    }

    /// Set value with custom TTL
    pub async fn set_with_ttl<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: usize,
    ) -> BeastResult<()> {
        let serialized = serde_json::to_string(value)
            .map_err(|e| BeastError::DatabaseError(format!("Failed to serialize: {}", e)))?;

        let mut conn = self.client.clone();
        conn.set_ex::<_, _, ()>(key, serialized, ttl_seconds as u64)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis SET failed: {}", e)))?;

        Ok(())
    }

    /// Get value
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> BeastResult<Option<T>> {
        let mut conn = self.client.clone();
        let result: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis GET failed: {}", e)))?;

        if let Some(data) = result {
            let value = serde_json::from_str(&data)
                .map_err(|e| BeastError::DatabaseError(format!("Failed to deserialize: {}", e)))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Delete key
    pub async fn delete(&self, key: &str) -> BeastResult<()> {
        let mut conn = self.client.clone();
        conn.del::<_, ()>(key)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis DEL failed: {}", e)))?;
        Ok(())
    }

    /// Delete multiple keys
    pub async fn delete_pattern(&self, pattern: &str) -> BeastResult<usize> {
        let mut conn = self.client.clone();

        // Get all keys matching pattern
        let keys: Vec<String> = conn
            .keys(pattern)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis KEYS failed: {}", e)))?;

        if keys.is_empty() {
            return Ok(0);
        }

        // Delete all matching keys
        let count = keys.len();
        conn.del::<_, ()>(&keys)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis DEL failed: {}", e)))?;

        Ok(count)
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> BeastResult<bool> {
        let mut conn = self.client.clone();
        conn.exists(key)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis EXISTS failed: {}", e)))
    }

    /// Set expiration on existing key
    pub async fn expire(&self, key: &str, ttl_seconds: usize) -> BeastResult<()> {
        let mut conn = self.client.clone();
        conn.expire::<_, ()>(key, ttl_seconds as i64)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis EXPIRE failed: {}", e)))?;
        Ok(())
    }

    /// Get TTL of key
    pub async fn ttl(&self, key: &str) -> BeastResult<i64> {
        let mut conn = self.client.clone();
        conn.ttl(key)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis TTL failed: {}", e)))
    }

    /// Increment counter
    pub async fn incr(&self, key: &str) -> BeastResult<i64> {
        let mut conn = self.client.clone();
        conn.incr(key, 1)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis INCR failed: {}", e)))
    }

    /// Increment counter by amount
    pub async fn incr_by(&self, key: &str, amount: i64) -> BeastResult<i64> {
        let mut conn = self.client.clone();
        conn.incr(key, amount)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis INCRBY failed: {}", e)))
    }

    /// Health check
    pub async fn health_check(&self) -> BeastResult<bool> {
        let mut conn = self.client.clone();
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis PING failed: {}", e)))?;

        Ok(pong == "PONG")
    }

    /// Get info
    pub async fn get_info(&self) -> BeastResult<RedisInfo> {
        // Simple info without dbsize
        Ok(RedisInfo {
            connected: self.health_check().await.unwrap_or(false),
            db_size: 0, // Not available with this Redis client
            used_memory: 0,
        })
    }

    /// Flush all keys (USE WITH CAUTION)
    pub async fn flush_all(&self) -> BeastResult<()> {
        let mut conn = self.client.clone();
        redis::cmd("FLUSHALL")
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| BeastError::DatabaseError(format!("Redis FLUSHALL failed: {}", e)))?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisInfo {
    pub connected: bool,
    pub db_size: usize,
    pub used_memory: usize,
}

impl Clone for RedisCache {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            default_ttl: self.default_ttl,
        }
    }
}

/// Cache key builders
pub mod keys {
    pub fn transaction(signature: &str) -> String {
        format!("tx:{}", signature)
    }

    pub fn wallet_analysis(address: &str) -> String {
        format!("analysis:{}", address)
    }

    pub fn token_metadata(mint: &str) -> String {
        format!("token:{}", mint)
    }

    pub fn fund_flow_graph(wallet: &str) -> String {
        format!("graph:{}", wallet)
    }

    pub fn pattern_analysis(wallet: &str) -> String {
        format!("patterns:{}", wallet)
    }

    pub fn rate_limit(identifier: &str) -> String {
        format!("ratelimit:{}", identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_keys() {
        assert_eq!(keys::transaction("sig123"), "tx:sig123");
        assert_eq!(keys::wallet_analysis("wallet123"), "analysis:wallet123");
        assert_eq!(keys::token_metadata("mint123"), "token:mint123");
    }
}

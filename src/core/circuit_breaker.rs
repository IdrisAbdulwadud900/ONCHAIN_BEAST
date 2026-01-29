use crate::core::errors::{BeastError, BeastResult};
use crate::metrics::{CIRCUIT_BREAKER_STATE, CIRCUIT_BREAKER_TRIPS};
/// Simple Circuit Breaker for RPC calls
/// Prevents cascade failures
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Too many failures, rejecting calls
    HalfOpen, // Testing if service recovered
}

/// Simple circuit breaker implementation
pub struct RpcCircuitBreaker {
    service_name: String,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<AtomicU64>,
    success_count: Arc<AtomicU64>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    failure_threshold: u64,
    timeout_duration: Duration,
}

impl RpcCircuitBreaker {
    /// Create new circuit breaker
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            failure_threshold: 5,
            timeout_duration: Duration::from_secs(30),
        }
    }

    /// Execute function with circuit breaker protection
    pub async fn call<F, Fut, T>(&self, operation: F) -> BeastResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = BeastResult<T>>,
    {
        // Check if circuit is open
        let state = self.check_state().await;

        match state {
            CircuitState::Open => {
                CIRCUIT_BREAKER_TRIPS
                    .with_label_values(&[&self.service_name])
                    .inc();
                return Err(BeastError::RpcError(
                    "Circuit breaker open - too many failures".to_string(),
                ));
            }
            CircuitState::HalfOpen | CircuitState::Closed => {
                // Try the operation
                match operation().await {
                    Ok(result) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Err(err) => {
                        self.on_failure().await;
                        Err(err)
                    }
                }
            }
        }
    }

    /// Check and update circuit state
    async fn check_state(&self) -> CircuitState {
        let mut state = self.state.write().await;
        let failures = self.failure_count.load(Ordering::Relaxed);

        match *state {
            CircuitState::Closed => {
                if failures >= self.failure_threshold {
                    *state = CircuitState::Open;
                    CIRCUIT_BREAKER_STATE
                        .with_label_values(&[&self.service_name])
                        .set(1.0);
                    tracing::warn!("{} circuit breaker opened", self.service_name);
                }
            }
            CircuitState::Open => {
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() > self.timeout_duration {
                        *state = CircuitState::HalfOpen;
                        CIRCUIT_BREAKER_STATE
                            .with_label_values(&[&self.service_name])
                            .set(2.0);
                        tracing::info!("{} circuit breaker half-open", self.service_name);
                    }
                }
            }
            CircuitState::HalfOpen => {
                // State will be updated by on_success/on_failure
            }
        }

        *state
    }

    /// Record successful call
    async fn on_success(&self) {
        let successes = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
        let mut state = self.state.write().await;

        if *state == CircuitState::HalfOpen && successes >= 3 {
            *state = CircuitState::Closed;
            self.failure_count.store(0, Ordering::Relaxed);
            self.success_count.store(0, Ordering::Relaxed);
            CIRCUIT_BREAKER_STATE
                .with_label_values(&[&self.service_name])
                .set(0.0);
            tracing::info!("{} circuit breaker closed", self.service_name);
        }
    }

    /// Record failed call
    async fn on_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        *self.last_failure_time.write().await = Some(Instant::now());

        let mut state = self.state.write().await;
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Open;
            CIRCUIT_BREAKER_STATE
                .with_label_values(&[&self.service_name])
                .set(1.0);
            tracing::warn!("{} circuit breaker re-opened", self.service_name);
        }
    }

    /// Get current state
    pub async fn state(&self) -> String {
        format!("{:?}", *self.state.read().await)
    }

    /// Check if circuit is open
    pub async fn is_open(&self) -> bool {
        matches!(*self.state.read().await, CircuitState::Open)
    }
}

impl Clone for RpcCircuitBreaker {
    fn clone(&self) -> Self {
        Self {
            service_name: self.service_name.clone(),
            state: Arc::clone(&self.state),
            failure_count: Arc::clone(&self.failure_count),
            success_count: Arc::clone(&self.success_count),
            last_failure_time: Arc::clone(&self.last_failure_time),
            failure_threshold: self.failure_threshold,
            timeout_duration: self.timeout_duration,
        }
    }
}

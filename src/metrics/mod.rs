use lazy_static::lazy_static;
/// Prometheus Metrics
/// Application monitoring and observability
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::time::Instant;

lazy_static! {
    /// Global metrics registry
    pub static ref REGISTRY: Registry = Registry::new();

    // === Request Metrics ===

    /// Total HTTP requests
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("http_requests_total", "Total HTTP requests"),
        &["method", "endpoint", "status"]
    ).unwrap();

    /// HTTP request duration
    pub static ref HTTP_REQUEST_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds")
            .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
        &["method", "endpoint"]
    ).unwrap();

    /// Active connections
    pub static ref ACTIVE_CONNECTIONS: Gauge = Gauge::new(
        "active_connections",
        "Number of active HTTP connections"
    ).unwrap();

    // === Transaction Parsing Metrics ===

    /// Transactions parsed
    pub static ref TRANSACTIONS_PARSED: Counter = Counter::new(
        "transactions_parsed_total",
        "Total transactions parsed"
    ).unwrap();

    /// Parsing errors
    pub static ref PARSE_ERRORS: CounterVec = CounterVec::new(
        Opts::new("parse_errors_total", "Total parsing errors"),
        &["error_type"]
    ).unwrap();

    /// Parse duration
    pub static ref PARSE_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("parse_duration_seconds", "Transaction parsing duration")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0])
    ).unwrap();

    // === Transfer Metrics ===

    /// SOL transfers extracted
    pub static ref SOL_TRANSFERS: Counter = Counter::new(
        "sol_transfers_extracted_total",
        "Total SOL transfers extracted"
    ).unwrap();

    /// Token transfers extracted
    pub static ref TOKEN_TRANSFERS: Counter = Counter::new(
        "token_transfers_extracted_total",
        "Total token transfers extracted"
    ).unwrap();

    /// Transfer amounts (SOL)
    pub static ref SOL_TRANSFER_AMOUNT: Histogram = Histogram::with_opts(
        HistogramOpts::new("sol_transfer_amount", "SOL transfer amounts")
            .buckets(vec![0.001, 0.01, 0.1, 1.0, 10.0, 100.0, 1000.0])
    ).unwrap();

    // === Cache Metrics ===

    /// Cache hits
    pub static ref CACHE_HITS: CounterVec = CounterVec::new(
        Opts::new("cache_hits_total", "Total cache hits"),
        &["cache_type"]
    ).unwrap();

    /// Cache misses
    pub static ref CACHE_MISSES: CounterVec = CounterVec::new(
        Opts::new("cache_misses_total", "Total cache misses"),
        &["cache_type"]
    ).unwrap();

    /// Cache size
    pub static ref CACHE_SIZE: GaugeVec = GaugeVec::new(
        Opts::new("cache_size", "Current cache size"),
        &["cache_type"]
    ).unwrap();

    // === Database Metrics ===

    /// Database queries
    pub static ref DB_QUERIES: CounterVec = CounterVec::new(
        Opts::new("db_queries_total", "Total database queries"),
        &["operation", "table"]
    ).unwrap();

    /// Database query duration
    pub static ref DB_QUERY_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("db_query_duration_seconds", "Database query duration")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
        &["operation", "table"]
    ).unwrap();

    /// Database errors
    pub static ref DB_ERRORS: CounterVec = CounterVec::new(
        Opts::new("db_errors_total", "Total database errors"),
        &["operation", "error_type"]
    ).unwrap();

    /// Database connection pool
    pub static ref DB_POOL_ACTIVE: Gauge = Gauge::new(
        "db_pool_active_connections",
        "Active database connections"
    ).unwrap();

    pub static ref DB_POOL_IDLE: Gauge = Gauge::new(
        "db_pool_idle_connections",
        "Idle database connections"
    ).unwrap();

    // === RPC Metrics ===

    /// RPC calls
    pub static ref RPC_CALLS: CounterVec = CounterVec::new(
        Opts::new("rpc_calls_total", "Total RPC calls"),
        &["method", "status"]
    ).unwrap();

    /// RPC call duration
    pub static ref RPC_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new("rpc_duration_seconds", "RPC call duration")
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
        &["method"]
    ).unwrap();

    /// RPC errors
    pub static ref RPC_ERRORS: CounterVec = CounterVec::new(
        Opts::new("rpc_errors_total", "Total RPC errors"),
        &["method", "error_type"]
    ).unwrap();

    // === Token Metadata Metrics ===

    /// Token metadata fetched
    pub static ref TOKEN_METADATA_FETCHED: Counter = Counter::new(
        "token_metadata_fetched_total",
        "Total token metadata fetched"
    ).unwrap();

    /// Token metadata cache hits
    pub static ref TOKEN_METADATA_CACHE_HITS: Counter = Counter::new(
        "token_metadata_cache_hits_total",
        "Token metadata cache hits"
    ).unwrap();

    // === Analysis Metrics ===

    /// Wallet analyses performed
    pub static ref WALLET_ANALYSES: Counter = Counter::new(
        "wallet_analyses_total",
        "Total wallet analyses performed"
    ).unwrap();

    /// Pattern detections
    pub static ref PATTERN_DETECTIONS: CounterVec = CounterVec::new(
        Opts::new("pattern_detections_total", "Total pattern detections"),
        &["pattern_type", "detected"]
    ).unwrap();

    /// Analysis duration
    pub static ref ANALYSIS_DURATION: Histogram = Histogram::with_opts(
        HistogramOpts::new("analysis_duration_seconds", "Wallet analysis duration")
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0])
    ).unwrap();

    /// Graph nodes
    pub static ref GRAPH_NODES: Histogram = Histogram::with_opts(
        HistogramOpts::new("graph_nodes", "Number of nodes in fund flow graph")
            .buckets(vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0])
    ).unwrap();

    /// Graph edges
    pub static ref GRAPH_EDGES: Histogram = Histogram::with_opts(
        HistogramOpts::new("graph_edges", "Number of edges in fund flow graph")
            .buckets(vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0])
    ).unwrap();

    // === Circuit Breaker Metrics ===

    /// Circuit breaker state
    pub static ref CIRCUIT_BREAKER_STATE: GaugeVec = GaugeVec::new(
        Opts::new("circuit_breaker_state", "Circuit breaker state (0=closed, 1=open, 2=half-open)"),
        &["service"]
    ).unwrap();

    /// Circuit breaker trips
    pub static ref CIRCUIT_BREAKER_TRIPS: CounterVec = CounterVec::new(
        Opts::new("circuit_breaker_trips_total", "Total circuit breaker trips"),
        &["service"]
    ).unwrap();
}

/// Initialize metrics registry
pub fn init_metrics() {
    // Register HTTP metrics
    REGISTRY
        .register(Box::new(HTTP_REQUESTS_TOTAL.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(HTTP_REQUEST_DURATION.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(ACTIVE_CONNECTIONS.clone()))
        .unwrap();

    // Register parsing metrics
    REGISTRY
        .register(Box::new(TRANSACTIONS_PARSED.clone()))
        .unwrap();
    REGISTRY.register(Box::new(PARSE_ERRORS.clone())).unwrap();
    REGISTRY.register(Box::new(PARSE_DURATION.clone())).unwrap();

    // Register transfer metrics
    REGISTRY.register(Box::new(SOL_TRANSFERS.clone())).unwrap();
    REGISTRY
        .register(Box::new(TOKEN_TRANSFERS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(SOL_TRANSFER_AMOUNT.clone()))
        .unwrap();

    // Register cache metrics
    REGISTRY.register(Box::new(CACHE_HITS.clone())).unwrap();
    REGISTRY.register(Box::new(CACHE_MISSES.clone())).unwrap();
    REGISTRY.register(Box::new(CACHE_SIZE.clone())).unwrap();

    // Register database metrics
    REGISTRY.register(Box::new(DB_QUERIES.clone())).unwrap();
    REGISTRY
        .register(Box::new(DB_QUERY_DURATION.clone()))
        .unwrap();
    REGISTRY.register(Box::new(DB_ERRORS.clone())).unwrap();
    REGISTRY.register(Box::new(DB_POOL_ACTIVE.clone())).unwrap();
    REGISTRY.register(Box::new(DB_POOL_IDLE.clone())).unwrap();

    // Register RPC metrics
    REGISTRY.register(Box::new(RPC_CALLS.clone())).unwrap();
    REGISTRY.register(Box::new(RPC_DURATION.clone())).unwrap();
    REGISTRY.register(Box::new(RPC_ERRORS.clone())).unwrap();

    // Register token metadata metrics
    REGISTRY
        .register(Box::new(TOKEN_METADATA_FETCHED.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(TOKEN_METADATA_CACHE_HITS.clone()))
        .unwrap();

    // Register analysis metrics
    REGISTRY
        .register(Box::new(WALLET_ANALYSES.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(PATTERN_DETECTIONS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(ANALYSIS_DURATION.clone()))
        .unwrap();
    REGISTRY.register(Box::new(GRAPH_NODES.clone())).unwrap();
    REGISTRY.register(Box::new(GRAPH_EDGES.clone())).unwrap();

    // Register circuit breaker metrics
    REGISTRY
        .register(Box::new(CIRCUIT_BREAKER_STATE.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(CIRCUIT_BREAKER_TRIPS.clone()))
        .unwrap();
}

/// Get metrics in Prometheus format
pub fn gather_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// Timer helper for measuring durations
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    pub fn observe_and_reset(&mut self, histogram: &Histogram) -> f64 {
        let duration = self.elapsed_secs();
        histogram.observe(duration);
        self.start = Instant::now();
        duration
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_secs();
        assert!(elapsed >= 0.01);
    }
}

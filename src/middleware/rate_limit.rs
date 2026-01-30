/// Rate Limiting Middleware
///
/// Implements per-IP and per-API-key rate limiting using token bucket algorithm
use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use dashmap::DashMap;
use futures::future::LocalBoxFuture;
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use std::future::{ready, Ready};
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;

use super::auth::ApiKey;

fn is_rate_limit_exempt_path(path: &str) -> bool {
    matches!(
        path,
        "/" | "/health" | "/status" | "/metrics" | "/metrics/health"
    )
}

/// Rate limiter configuration
#[derive(Clone)]
pub struct RateLimiterConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            burst_size: 10,
        }
    }
}

/// Rate limiter middleware
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimiterConfig,
    // Per-IP rate limiters
    ip_limiters:
        Arc<DashMap<IpAddr, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>,
    // Per-API-key rate limiters
    api_key_limiters:
        Arc<DashMap<String, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>,
}

impl RateLimiter {
    /// Create new rate limiter with default config
    pub fn new() -> Self {
        Self::with_config(RateLimiterConfig::default())
    }

    /// Create new rate limiter with custom config
    pub fn with_config(config: RateLimiterConfig) -> Self {
        Self {
            config,
            ip_limiters: Arc::new(DashMap::new()),
            api_key_limiters: Arc::new(DashMap::new()),
        }
    }

    /// Get or create rate limiter for IP address
    fn get_ip_limiter(
        &self,
        ip: IpAddr,
    ) -> Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        self.ip_limiters
            .entry(ip)
            .or_insert_with(|| {
                let per_minute = NonZeroU32::new(self.config.requests_per_minute.max(1)).unwrap();
                let burst = NonZeroU32::new(self.config.burst_size.max(1)).unwrap();
                let quota = Quota::per_minute(per_minute).allow_burst(burst);
                Arc::new(GovernorRateLimiter::direct(quota))
            })
            .clone()
    }

    /// Get or create rate limiter for API key
    fn get_api_key_limiter(
        &self,
        api_key: &str,
    ) -> Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        self.api_key_limiters
            .entry(api_key.to_string())
            .or_insert_with(|| {
                // API keys get higher rate limits
                let per_minute =
                    NonZeroU32::new((self.config.requests_per_minute * 5).max(1)).unwrap();
                let burst = NonZeroU32::new(self.config.burst_size.max(1)).unwrap();
                let quota = Quota::per_minute(per_minute).allow_burst(burst);
                Arc::new(GovernorRateLimiter::direct(quota))
            })
            .clone()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimiterMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterMiddleware {
            service,
            config: self.config.clone(),
            ip_limiters: self.ip_limiters.clone(),
            api_key_limiters: self.api_key_limiters.clone(),
        }))
    }
}

pub struct RateLimiterMiddleware<S> {
    service: S,
    config: RateLimiterConfig,
    ip_limiters:
        Arc<DashMap<IpAddr, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>,
    api_key_limiters:
        Arc<DashMap<String, Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>>>,
}

impl<S, B> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if is_rate_limit_exempt_path(req.path()) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            });
        }

        // Check if API key is present (higher rate limit)
        let api_key = req.extensions().get::<ApiKey>().map(|k| k.0.clone());

        if let Some(key) = api_key {
            // Use API key rate limiter
            let limiter = self
                .api_key_limiters
                .entry(key)
                .or_insert_with(|| {
                    let per_minute =
                        NonZeroU32::new((self.config.requests_per_minute * 5).max(1)).unwrap();
                    let burst = NonZeroU32::new(self.config.burst_size.max(1)).unwrap();
                    let quota = Quota::per_minute(per_minute).allow_burst(burst);
                    Arc::new(GovernorRateLimiter::direct(quota))
                })
                .clone();

            match limiter.check() {
                Ok(_) => {
                    let fut = self.service.call(req);
                    Box::pin(async move {
                        let res = fut.await?;
                        Ok(res.map_into_boxed_body())
                    })
                }
                Err(_) => {
                    let (req, _pl) = req.into_parts();
                    Box::pin(async move {
                        let response = HttpResponse::TooManyRequests().json(serde_json::json!({
                            "error": "Rate limit exceeded",
                            "message": "Too many requests. Please slow down.",
                            "limit": "300 requests per minute for authenticated users"
                        }));
                        Ok(ServiceResponse::new(req, response.map_into_boxed_body()))
                    })
                }
            }
        } else {
            // Use IP-based rate limiter
            let peer_addr = req.peer_addr().map(|addr| addr.ip());

            if let Some(ip) = peer_addr {
                let limiter = self
                    .ip_limiters
                    .entry(ip)
                    .or_insert_with(|| {
                        let per_minute =
                            NonZeroU32::new(self.config.requests_per_minute.max(1)).unwrap();
                        let burst = NonZeroU32::new(self.config.burst_size.max(1)).unwrap();
                        let quota = Quota::per_minute(per_minute).allow_burst(burst);
                        Arc::new(GovernorRateLimiter::direct(quota))
                    })
                    .clone();

                match limiter.check() {
                    Ok(_) => {
                        let fut = self.service.call(req);
                        Box::pin(async move {
                            let res = fut.await?;
                            Ok(res.map_into_boxed_body())
                        })
                    }
                    Err(_) => {
                        let (req, _pl) = req.into_parts();
                        Box::pin(async move {
                            let response = HttpResponse::TooManyRequests()
                                .json(serde_json::json!({
                                    "error": "Rate limit exceeded",
                                    "message": "Too many requests. Please slow down or authenticate with an API key.",
                                    "limit": "60 requests per minute for unauthenticated users"
                                }));
                            Ok(ServiceResponse::new(req, response.map_into_boxed_body()))
                        })
                    }
                }
            } else {
                // No peer address, allow request
                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_boxed_body())
                })
            }
        }
    }
}

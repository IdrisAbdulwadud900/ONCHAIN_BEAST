/// API Key Authentication Middleware
///
/// Validates API keys from request headers and enforces authentication
use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures::future::LocalBoxFuture;
use std::collections::HashSet;
use std::future::{ready, Ready};
use std::sync::Arc;

/// API Key authentication middleware
#[derive(Clone)]
pub struct ApiKeyAuth {
    valid_keys: Arc<HashSet<String>>,
    public_endpoints: Arc<HashSet<String>>,
}

impl ApiKeyAuth {
    /// Create new API key authentication middleware
    pub fn new(api_keys: Vec<String>) -> Self {
        let mut valid_keys = HashSet::new();
        for key in api_keys {
            valid_keys.insert(key);
        }

        // Public endpoints that don't require authentication
        let mut public_endpoints = HashSet::new();
        public_endpoints.insert("/".to_string());
        public_endpoints.insert("/health".to_string());
        public_endpoints.insert("/status".to_string());

        Self {
            valid_keys: Arc::new(valid_keys),
            public_endpoints: Arc::new(public_endpoints),
        }
    }

    /// Check if endpoint is public
    fn is_public_endpoint(&self, path: &str) -> bool {
        self.public_endpoints.contains(path)
    }

    /// Validate API key from request
    fn validate_api_key(&self, api_key: &str) -> bool {
        self.valid_keys.contains(api_key)
    }
}

impl<S, B> Transform<S, ServiceRequest> for ApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = ApiKeyAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ApiKeyAuthMiddleware {
            service,
            valid_keys: self.valid_keys.clone(),
            public_endpoints: self.public_endpoints.clone(),
        }))
    }
}

pub struct ApiKeyAuthMiddleware<S> {
    service: S,
    valid_keys: Arc<HashSet<String>>,
    public_endpoints: Arc<HashSet<String>>,
}

impl<S, B> Service<ServiceRequest> for ApiKeyAuthMiddleware<S>
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
        let path = req.path().to_string();

        // Check if endpoint is public
        if self.public_endpoints.contains(&path) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            });
        }

        // If no API keys configured, allow all requests (auth disabled)
        if self.valid_keys.is_empty() {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            });
        }

        // Extract API key from header
        let api_key = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let valid_keys = self.valid_keys.clone();

        // Validate API key
        match api_key {
            Some(key) if valid_keys.contains(&key) => {
                // Store API key in request extensions for later use
                req.extensions_mut().insert(ApiKey(key.clone()));

                let fut = self.service.call(req);
                Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_boxed_body())
                })
            }
            Some(_) => {
                // Invalid API key
                let (req, _pl) = req.into_parts();
                Box::pin(async move {
                    let response = HttpResponse::Unauthorized().json(serde_json::json!({
                        "error": "Invalid API key",
                        "message": "The provided API key is not valid"
                    }));
                    Ok(ServiceResponse::new(req, response.map_into_boxed_body()))
                })
            }
            None => {
                // Missing API key
                let (req, _pl) = req.into_parts();
                Box::pin(async move {
                    let response = HttpResponse::Unauthorized().json(serde_json::json!({
                        "error": "Missing API key",
                        "message": "API key required. Include 'X-API-Key' header in your request"
                    }));
                    Ok(ServiceResponse::new(req, response.map_into_boxed_body()))
                })
            }
        }
    }
}

/// API Key stored in request extensions
#[derive(Clone)]
pub struct ApiKey(pub String);

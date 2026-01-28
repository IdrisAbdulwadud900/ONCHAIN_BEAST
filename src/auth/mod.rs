/// Simple API Key Authentication using Extractors
///
/// This approach uses Actix-web extractors instead of middleware,
/// avoiding complex type system issues while providing clean auth.

use actix_web::{
    dev::Payload,
    error::ErrorUnauthorized,
    Error, FromRequest, HttpRequest,
};
use futures::future::{ready, Ready};
use std::sync::OnceLock;

/// Global API keys configuration
static API_KEYS: OnceLock<Vec<String>> = OnceLock::new();

/// Initialize API keys from configuration
pub fn init_api_keys(keys: Vec<String>) {
    API_KEYS.get_or_init(|| keys);
}

/// Get configured API keys
fn get_api_keys() -> &'static [String] {
    match API_KEYS.get() {
        Some(keys) => keys.as_slice(),
        None => &[],
    }
}

/// Check if authentication is enabled
pub fn is_auth_enabled() -> bool {
    !get_api_keys().is_empty()
}

/// Authenticated request - requires valid API key
///
/// Usage in handlers:
/// ```
/// async fn protected_handler(_auth: ApiKey) -> Result<HttpResponse> {
///     // This handler requires authentication
///     Ok(HttpResponse::Ok().json("Protected data"))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ApiKey(pub String);

impl FromRequest for ApiKey {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        // If auth is disabled, allow all requests
        if !is_auth_enabled() {
            return ready(Ok(ApiKey("auth-disabled".to_string())));
        }

        // Extract API key from header
        let api_key = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        match api_key {
            Some(key) if get_api_keys().contains(&key) => {
                // Valid API key
                ready(Ok(ApiKey(key)))
            }
            Some(_) => {
                // Invalid API key
                ready(Err(ErrorUnauthorized(serde_json::json!({
                    "error": "Invalid API key",
                    "message": "The provided API key is not valid"
                }))))
            }
            None => {
                // Missing API key
                ready(Err(ErrorUnauthorized(serde_json::json!({
                    "error": "Missing API key",
                    "message": "API key required. Include 'X-API-Key' header in your request"
                }))))
            }
        }
    }
}

/// Optional authentication - allows both authenticated and public access
///
/// Usage in handlers:
/// ```
/// async fn maybe_protected_handler(auth: MaybeApiKey) -> Result<HttpResponse> {
///     match auth.0 {
///         Some(api_key) => {
///             // Authenticated user - provide enhanced response
///             Ok(HttpResponse::Ok().json("Enhanced data"))
///         }
///         None => {
///             // Public access - provide basic response
///             Ok(HttpResponse::Ok().json("Basic data"))
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MaybeApiKey(pub Option<ApiKey>);

impl FromRequest for MaybeApiKey {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        // If auth is disabled, always return None (public access)
        if !is_auth_enabled() {
            return ready(Ok(MaybeApiKey(None)));
        }

        // Extract API key from header
        let api_key = req
            .headers()
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        match api_key {
            Some(key) if get_api_keys().contains(&key) => {
                // Valid API key - authenticated
                ready(Ok(MaybeApiKey(Some(ApiKey(key)))))
            }
            _ => {
                // No key or invalid key - public access
                ready(Ok(MaybeApiKey(None)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_keys_initialization() {
        // Note: This test may fail if keys are already initialized
        let keys = vec!["test-key-1".to_string(), "test-key-2".to_string()];
        init_api_keys(keys.clone());
        
        assert!(is_auth_enabled());
        assert_eq!(get_api_keys().len(), 2);
    }
}

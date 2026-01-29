/// Token Metadata API Routes
/// Provides REST endpoints for token metadata queries and statistics
use crate::api::ApiState;
use crate::metrics::{Timer, HTTP_REQUESTS_TOTAL, HTTP_REQUEST_DURATION};
use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct TokenMetadataResponse {
    pub mint: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub verified: bool,
}

#[derive(Serialize)]
pub struct MetadataStatsResponse {
    pub total_tokens: usize,
    pub cached_count: usize,
    pub fetch_success_rate: f64,
    pub avg_fetch_time_ms: f64,
    pub common_symbols: Vec<String>,
}

#[derive(Serialize)]
pub struct SearchResultsResponse {
    pub query: String,
    pub results: Vec<TokenMetadataResponse>,
}

#[derive(Serialize)]
pub struct TopTokensResponse {
    pub tokens: Vec<(String, usize)>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Deserialize)]
pub struct TopTokensQuery {
    pub limit: Option<i32>,
}

/// GET /metadata/token/{mint}
/// Get token metadata by mint address
pub async fn get_token_metadata(
    state: web::Data<ApiState>,
    mint: web::Path<String>,
) -> impl Responder {
    let timer = Timer::new();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "metadata_token"])
        .inc();

    match state.token_metadata_service.get_token_metadata(&mint).await {
        Ok(metadata) => {
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_token", "200"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(TokenMetadataResponse {
                mint: metadata.mint,
                symbol: metadata.symbol,
                name: metadata.name,
                decimals: metadata.decimals,
                verified: metadata.verified,
            })
        }
        Err(e) => {
            tracing::warn!("Failed to get token metadata for {}: {:?}", mint, e);
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_token", "404"])
                .observe(timer.elapsed_secs());

            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Token metadata not found"
            }))
        }
    }
}

/// POST /metadata/batch
/// Get metadata for multiple tokens
pub async fn get_tokens_batch(
    state: web::Data<ApiState>,
    mints: web::Json<Vec<String>>,
) -> impl Responder {
    let timer = Timer::new();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["POST", "metadata_batch"])
        .inc();

    match state
        .token_metadata_service
        .get_token_metadata_batch(&mints)
        .await
    {
        Ok(metadata_map) => {
            let results: Vec<TokenMetadataResponse> = metadata_map
                .values()
                .map(|m| TokenMetadataResponse {
                    mint: m.mint.clone(),
                    symbol: m.symbol.clone(),
                    name: m.name.clone(),
                    decimals: m.decimals,
                    verified: m.verified,
                })
                .collect();

            HTTP_REQUEST_DURATION
                .with_label_values(&["POST", "metadata_batch", "200"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(serde_json::json!({
                "count": results.len(),
                "tokens": results
            }))
        }
        Err(e) => {
            tracing::error!("Failed to batch fetch metadata: {:?}", e);
            HTTP_REQUEST_DURATION
                .with_label_values(&["POST", "metadata_batch", "500"])
                .observe(timer.elapsed_secs());

            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch metadata"
            }))
        }
    }
}

/// GET /metadata/stats
/// Get token metadata statistics
pub async fn get_metadata_stats(state: web::Data<ApiState>) -> impl Responder {
    let timer = Timer::new();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "metadata_stats"])
        .inc();

    match state.token_metadata_service.get_metadata_stats().await {
        Ok(stats) => {
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_stats", "200"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(MetadataStatsResponse {
                total_tokens: stats.total_tokens,
                cached_count: stats.cached_count,
                fetch_success_rate: stats.fetch_success_rate,
                avg_fetch_time_ms: stats.avg_fetch_time_ms,
                common_symbols: stats.common_symbols,
            })
        }
        Err(e) => {
            tracing::error!("Failed to get metadata stats: {:?}", e);
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_stats", "500"])
                .observe(timer.elapsed_secs());

            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get statistics"
            }))
        }
    }
}

/// GET /metadata/search
/// Search tokens by symbol or name
pub async fn search_tokens(
    state: web::Data<ApiState>,
    query: web::Query<SearchQuery>,
) -> impl Responder {
    let timer = Timer::new();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "metadata_search"])
        .inc();

    if query.q.is_empty() {
        HTTP_REQUEST_DURATION
            .with_label_values(&["GET", "metadata_search", "400"])
            .observe(timer.elapsed_secs());

        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Query parameter 'q' is required"
        }));
    }

    match state.token_metadata_service.search_tokens(&query.q).await {
        Ok(tokens) => {
            let results: Vec<TokenMetadataResponse> = tokens
                .iter()
                .map(|t| TokenMetadataResponse {
                    mint: t.mint.clone(),
                    symbol: t.symbol.clone(),
                    name: t.name.clone(),
                    decimals: t.decimals,
                    verified: t.verified,
                })
                .collect();

            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_search", "200"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(SearchResultsResponse {
                query: query.q.clone(),
                results,
            })
        }
        Err(e) => {
            tracing::error!("Failed to search tokens: {:?}", e);
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_search", "500"])
                .observe(timer.elapsed_secs());

            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Search failed"
            }))
        }
    }
}

/// GET /metadata/top-tokens
/// Get most used tokens
pub async fn get_top_tokens(
    state: web::Data<ApiState>,
    query: web::Query<TopTokensQuery>,
) -> impl Responder {
    let timer = Timer::new();
    HTTP_REQUESTS_TOTAL
        .with_label_values(&["GET", "metadata_top"])
        .inc();

    let limit = query.limit.unwrap_or(10);
    if limit <= 0 || limit > 100 {
        HTTP_REQUEST_DURATION
            .with_label_values(&["GET", "metadata_top", "400"])
            .observe(timer.elapsed_secs());

        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Limit must be between 1 and 100"
        }));
    }

    match state.token_metadata_service.get_top_tokens(limit).await {
        Ok(tokens) => {
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_top", "200"])
                .observe(timer.elapsed_secs());

            HttpResponse::Ok().json(TopTokensResponse { tokens })
        }
        Err(e) => {
            tracing::error!("Failed to get top tokens: {:?}", e);
            HTTP_REQUEST_DURATION
                .with_label_values(&["GET", "metadata_top", "500"])
                .observe(timer.elapsed_secs());

            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get top tokens"
            }))
        }
    }
}

/// Configure metadata routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metadata")
            .route("/token/{mint}", web::get().to(get_token_metadata))
            .route("/batch", web::post().to(get_tokens_batch))
            .route("/stats", web::get().to(get_metadata_stats))
            .route("/search", web::get().to(search_tokens))
            .route("/top-tokens", web::get().to(get_top_tokens)),
    );
}

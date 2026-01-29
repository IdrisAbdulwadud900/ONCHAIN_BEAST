use crate::metrics;
/// Metrics API routes
/// Exposes Prometheus metrics endpoint
use actix_web::{web, HttpResponse};

/// GET /metrics - Prometheus metrics endpoint
pub async fn get_metrics() -> HttpResponse {
    let metrics_output = metrics::gather_metrics();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(metrics_output)
}

/// Health check with Redis and database status
pub async fn health_check(
    db_manager: web::Data<std::sync::Arc<crate::storage::DatabaseManager>>,
    redis_cache: web::Data<std::sync::Arc<crate::storage::RedisCache>>,
) -> HttpResponse {
    let mut health_status = serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    // Check database
    match db_manager.health_check().await {
        Ok(healthy) => {
            health_status["database"] = serde_json::json!({
                "status": if healthy { "healthy" } else { "unhealthy" },
                "type": "postgresql"
            });
        }
        Err(e) => {
            health_status["database"] = serde_json::json!({
                "status": "error",
                "error": e.to_string()
            });
            health_status["status"] = serde_json::json!("degraded");
        }
    }

    // Check Redis
    match redis_cache.health_check().await {
        Ok(healthy) => {
            health_status["redis"] = serde_json::json!({
                "status": if healthy { "healthy" } else { "unhealthy" }
            });
        }
        Err(e) => {
            health_status["redis"] = serde_json::json!({
                "status": "error",
                "error": e.to_string()
            });
            health_status["status"] = serde_json::json!("degraded");
        }
    }

    // Get database stats
    if let Ok(stats) = db_manager.get_stats().await {
        health_status["database_stats"] = serde_json::json!({
            "transactions": stats.transaction_count,
            "wallet_analyses": stats.wallet_analysis_count,
            "relationships": stats.wallet_relationship_count,
        });
    }

    // Get Redis info
    if let Ok(info) = redis_cache.get_info().await {
        health_status["redis_info"] = serde_json::json!({
            "db_size": info.db_size,
            "used_memory": info.used_memory,
        });
    }

    HttpResponse::Ok().json(health_status)
}

pub fn configure_metrics_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .route("", web::get().to(get_metrics))
            .route("/health", web::get().to(health_check)),
    );
}

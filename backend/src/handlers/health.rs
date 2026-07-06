use axum::Json;

use crate::dto::health::HealthResponse;

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::ok())
}

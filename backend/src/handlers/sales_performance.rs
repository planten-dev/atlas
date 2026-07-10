use axum::{
    Extension, Json,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    dto::{
        auth::ErrorResponse,
        sales_performance::{
            CreatePerformanceBatchRequest, PerformanceBatchesQuery, PerformanceEntriesQuery,
            PerformanceMonthQuery, PerformanceSummaryQuery,
        },
    },
    repositories::RepositoryError,
    services::{auth::CurrentSession, sales_performance::SalesPerformanceError},
    state::AppState,
};

pub async fn pending(
    State(state): State<AppState>,
    query: Result<Query<PerformanceMonthQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(error) => return rejection("invalid query parameters", error),
    };
    match state.sales_performance.pending(query).await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn create_batch(
    State(state): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    request: Result<Json<CreatePerformanceBatchRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => return rejection("invalid request body", error),
    };
    match state
        .sales_performance
        .post_batch(session.user.id, request)
        .await
    {
        Ok(value) => (StatusCode::CREATED, Json(value)).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn batches(
    State(state): State<AppState>,
    query: Result<Query<PerformanceBatchesQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(error) => return rejection("invalid query parameters", error),
    };
    match state.sales_performance.batches(query).await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn summary(
    State(state): State<AppState>,
    query: Result<Query<PerformanceSummaryQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(error) => return rejection("invalid query parameters", error),
    };
    match state.sales_performance.summary(query).await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn entries(
    State(state): State<AppState>,
    query: Result<Query<PerformanceEntriesQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(error) => return rejection("invalid query parameters", error),
    };
    match state.sales_performance.entries(query).await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => error_response(error),
    }
}

fn error_response(error: SalesPerformanceError) -> Response {
    let status = match &error {
        SalesPerformanceError::Repository(RepositoryError::Database(_)) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        SalesPerformanceError::PaymentNotFound | SalesPerformanceError::SalesRecordNotFound => {
            StatusCode::NOT_FOUND
        }
        SalesPerformanceError::PaymentNotPending
        | SalesPerformanceError::PaymentOutsideMonth
        | SalesPerformanceError::SalesRecordInactive
        | SalesPerformanceError::AllocationTotalInvalid
        | SalesPerformanceError::PostedEntriesMissing => StatusCode::CONFLICT,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    (
        status,
        Json(ErrorResponse {
            error: error.code().to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn rejection<E: std::fmt::Display>(message: &str, error: E) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: format!("{message}: {error}"),
        }),
    )
        .into_response()
}

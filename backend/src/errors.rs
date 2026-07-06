use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("database error")]
    Database(#[from] sea_orm::DbErr),
    #[error("password hash error")]
    PasswordHash(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.log_internal_error();

        (
            self.status_code(),
            Json(ErrorResponse {
                code: self.code(),
                error: self.public_message(),
            }),
        )
            .into_response()
    }
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Database(_) | Self::PasswordHash(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::Database(_) => "database_error",
            Self::PasswordHash(_) => "password_hash_error",
        }
    }

    fn public_message(&self) -> String {
        match self {
            Self::BadRequest(message)
            | Self::Unauthorized(message)
            | Self::Forbidden(message)
            | Self::NotFound(message)
            | Self::Conflict(message) => message.clone(),
            Self::Database(_) => "database error".to_string(),
            Self::PasswordHash(_) => "password hash error".to_string(),
        }
    }

    fn log_internal_error(&self) {
        match self {
            Self::Database(err) => tracing::error!(error = %err, "database error"),
            Self::PasswordHash(message) => tracing::error!(error = %message, "password hash error"),
            _ => {}
        }
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(err: argon2::password_hash::Error) -> Self {
        Self::PasswordHash(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::Value;

    #[tokio::test]
    async fn client_error_response_contains_status_code_and_message() {
        let response = AppError::BadRequest("username is required".to_string()).into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response_body(response).await;
        assert_eq!(body["code"], "bad_request");
        assert_eq!(body["error"], "username is required");
    }

    #[tokio::test]
    async fn internal_error_response_hides_source_details() {
        let response = AppError::Database(sea_orm::DbErr::Custom(
            "database password leaked".to_string(),
        ))
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = response_body(response).await;
        assert_eq!(body["code"], "database_error");
        assert_eq!(body["error"], "database error");
    }

    async fn response_body(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");

        serde_json::from_slice(&bytes).expect("body should be json")
    }
}

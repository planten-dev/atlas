use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::{error, warn};

use crate::{
    dto::auth::ErrorResponse, integrations::dingtalk::DingTalkError, repositories::RepositoryError,
    services::auth::AuthError,
};

pub(crate) fn auth_error_response(error: AuthError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "auth request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "auth request rejected"
        );
    }

    (
        status,
        Json(ErrorResponse {
            error: code.to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &AuthError) -> StatusCode {
    match error {
        AuthError::DingTalk(error) => match error {
            DingTalkError::MissingConfig(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DingTalkError::ProviderHttp { .. } | DingTalkError::Http(_) => StatusCode::BAD_GATEWAY,
            DingTalkError::MissingRequiredField { .. } | DingTalkError::MissingIdentityField(_) => {
                StatusCode::BAD_REQUEST
            }
        },
        AuthError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        AuthError::MissingCallbackField(_)
        | AuthError::ProviderRejected { .. }
        | AuthError::StateMismatch => StatusCode::BAD_REQUEST,
        AuthError::MissingSession | AuthError::InvalidSession => StatusCode::UNAUTHORIZED,
        AuthError::InvalidSessionTtl => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

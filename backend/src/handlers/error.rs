use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::{error, warn};

use crate::{
    dto::auth::ErrorResponse,
    integrations::dingtalk::DingTalkError,
    repositories::RepositoryError,
    services::{auth::AuthError, authz::AuthzError},
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

pub(crate) fn authz_error_response(error: AuthzError) -> Response {
    let status = authz_status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "authorization request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "authorization request rejected"
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

pub(crate) fn permission_denied_response(object: &str, action: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "permission_denied".to_string(),
            message: format!("missing permission `{object}:{action}`"),
        }),
    )
        .into_response()
}

fn authz_status_code(error: &AuthzError) -> StatusCode {
    match error {
        AuthzError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        AuthzError::Casbin(_) | AuthzError::PolicyLoadConflict => StatusCode::INTERNAL_SERVER_ERROR,
        AuthzError::RoleNotFound | AuthzError::UserNotFound | AuthzError::PolicyNotFound => {
            StatusCode::NOT_FOUND
        }
        AuthzError::DuplicateRoleCode | AuthzError::DuplicatePolicy => StatusCode::CONFLICT,
        AuthzError::InvalidRoleKind
        | AuthzError::InvalidSubjectKind
        | AuthzError::InvalidEffect
        | AuthzError::InvalidInput(_)
        | AuthzError::InheritanceCycle
        | AuthzError::InheritanceTooDeep => StatusCode::UNPROCESSABLE_ENTITY,
        AuthzError::MissingSession => StatusCode::UNAUTHORIZED,
    }
}

fn status_code(error: &AuthError) -> StatusCode {
    match error {
        AuthError::DingTalk(error) => match error {
            DingTalkError::MissingConfig(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DingTalkError::ProviderHttp { .. }
            | DingTalkError::ProviderApi { .. }
            | DingTalkError::MissingResponseField { .. }
            | DingTalkError::Http(_) => StatusCode::BAD_GATEWAY,
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
        AuthError::UserNotFound => StatusCode::NOT_FOUND,
        AuthError::MissingSession | AuthError::InvalidSession => StatusCode::UNAUTHORIZED,
        AuthError::InvalidSessionTtl => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

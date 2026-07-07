use axum::{
    Router,
    routing::{get, post},
};

use crate::{handlers, state::AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route(
            "/api/v1/auth/login/dingtalk",
            get(handlers::auth::dingtalk_login),
        )
        .route(
            "/api/v1/auth/callback/dingtalk",
            get(handlers::auth::dingtalk_callback),
        )
        .route("/api/v1/auth/me", get(handlers::auth::me))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .with_state(state)
}

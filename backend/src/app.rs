use axum::{
    Router,
    middleware::from_fn_with_state,
    routing::{get, post},
};

use crate::{handlers, middleware::auth::require_auth, state::AppState};

pub fn router(state: AppState) -> Router {
    let authenticated_routes = Router::new()
        .route("/api/v1/auth/me", get(handlers::auth::me))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .route("/api/v1/users/list", get(handlers::users::list_users))
        .route(
            "/api/v1/users/detail/{user_id}",
            get(handlers::users::user_detail),
        )
        .route(
            "/api/v1/users/update-status/{user_id}",
            post(handlers::users::update_user_status),
        )
        .route(
            "/api/v1/users/delete/{user_id}",
            post(handlers::users::delete_user),
        )
        .route_layer(from_fn_with_state(state.clone(), require_auth));

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
        .merge(authenticated_routes)
        .with_state(state)
}

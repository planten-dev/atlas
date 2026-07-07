use axum::{
    Router,
    handler::Handler,
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
};

use crate::{
    handlers,
    middleware::{auth::require_auth, authz::require_permission},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    let authenticated_routes = Router::new()
        .route("/api/v1/auth/me", get(handlers::auth::me))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .route(
            "/api/v1/permissions/roles",
            get(handlers::permissions::list_roles
                .layer(require_permission(&state, "system:permissions:read")))
            .post(
                handlers::permissions::create_role
                    .layer(require_permission(&state, "system:permissions:write")),
            ),
        )
        .route(
            "/api/v1/permissions/roles/{roleId}",
            get(handlers::permissions::get_role
                .layer(require_permission(&state, "system:permissions:read")))
            .patch(
                handlers::permissions::update_role
                    .layer(require_permission(&state, "system:permissions:write")),
            )
            .delete(
                handlers::permissions::delete_role
                    .layer(require_permission(&state, "system:permissions:write")),
            ),
        )
        .route(
            "/api/v1/permissions/roles/{roleId}/parents",
            put(handlers::permissions::set_role_parents
                .layer(require_permission(&state, "system:permissions:write"))),
        )
        .route(
            "/api/v1/permissions/users/{userId}/roles",
            get(handlers::permissions::list_user_roles
                .layer(require_permission(&state, "system:permissions:read")))
            .put(
                handlers::permissions::set_user_roles
                    .layer(require_permission(&state, "system:permissions:write")),
            ),
        )
        .route(
            "/api/v1/permissions/policies",
            get(handlers::permissions::list_policies
                .layer(require_permission(&state, "system:permissions:read")))
            .post(
                handlers::permissions::create_policy
                    .layer(require_permission(&state, "system:permissions:write")),
            ),
        )
        .route(
            "/api/v1/permissions/policies/{policyId}",
            delete(
                handlers::permissions::delete_policy
                    .layer(require_permission(&state, "system:permissions:write")),
            ),
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

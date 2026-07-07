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
        .route(
            "/api/v1/products/list",
            get(handlers::products::list_products
                .layer(require_permission(&state, "products:read"))),
        )
        .route(
            "/api/v1/products/detail/{product_id}",
            get(handlers::products::product_detail
                .layer(require_permission(&state, "products:read"))),
        )
        .route(
            "/api/v1/products/create",
            post(
                handlers::products::create_product
                    .layer(require_permission(&state, "products:write")),
            ),
        )
        .route(
            "/api/v1/products/update/{product_id}",
            post(
                handlers::products::update_product
                    .layer(require_permission(&state, "products:write")),
            ),
        )
        .route(
            "/api/v1/products/disable/{product_id}",
            post(
                handlers::products::disable_product
                    .layer(require_permission(&state, "products:write")),
            ),
        )
        .route(
            "/api/v1/products/delete/{product_id}",
            post(
                handlers::products::delete_product
                    .layer(require_permission(&state, "products:write")),
            ),
        )
        .route(
            "/api/v1/systems/list",
            get(handlers::systems::list_systems.layer(require_permission(&state, "systems:read"))),
        )
        .route(
            "/api/v1/systems/detail/{system_id}",
            get(handlers::systems::system_detail.layer(require_permission(&state, "systems:read"))),
        )
        .route(
            "/api/v1/systems/create",
            post(
                handlers::systems::create_system.layer(require_permission(&state, "systems:write")),
            ),
        )
        .route(
            "/api/v1/systems/update/{system_id}",
            post(
                handlers::systems::update_system.layer(require_permission(&state, "systems:write")),
            ),
        )
        .route(
            "/api/v1/systems/disable/{system_id}",
            post(
                handlers::systems::disable_system
                    .layer(require_permission(&state, "systems:write")),
            ),
        )
        .route(
            "/api/v1/systems/delete/{system_id}",
            post(
                handlers::systems::delete_system.layer(require_permission(&state, "systems:write")),
            ),
        )
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

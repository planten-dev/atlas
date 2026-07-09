use axum::{
    Router,
    handler::Handler,
    middleware::{from_fn, from_fn_with_state},
    routing::{delete, get, post, put},
};

use crate::{
    handlers,
    middleware::{auth::require_auth, authz::require_permission, request_log::log_request},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    let authenticated_routes = Router::new()
        .route("/api/v1/auth/me", get(handlers::auth::me))
        // Deliberately carries no permission layer: every authenticated
        // user may read their own effective permissions (frontend menu
        // and button gating depends on this).
        .route(
            "/api/v1/auth/me/permissions",
            get(handlers::permissions::my_permissions),
        )
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .route(
            "/api/v1/users/list",
            get(handlers::users::list_users.layer(require_permission(&state, "users:read"))),
        )
        .route(
            "/api/v1/users/detail/{user_id}",
            get(handlers::users::user_detail.layer(require_permission(&state, "users:read"))),
        )
        .route(
            "/api/v1/users/{user_id}/profile",
            get(handlers::users::user_profile),
        )
        .route(
            "/api/v1/users/{user_id}/profile/sync",
            post(handlers::users::sync_user_profile),
        )
        .route(
            "/api/v1/users/update-status/{user_id}",
            post(
                handlers::users::update_user_status
                    .layer(require_permission(&state, "users:write")),
            ),
        )
        .route(
            "/api/v1/users/delete/{user_id}",
            post(handlers::users::delete_user.layer(require_permission(&state, "users:write"))),
        )
        .route(
            "/api/v1/product-categories/list",
            get(handlers::product_categories::list_categories
                .layer(require_permission(&state, "products:categories:read"))),
        )
        .route(
            "/api/v1/product-categories/detail/{category_id}",
            get(handlers::product_categories::category_detail
                .layer(require_permission(&state, "products:categories:read"))),
        )
        .route(
            "/api/v1/product-categories/create",
            post(
                handlers::product_categories::create_category
                    .layer(require_permission(&state, "products:categories:write")),
            ),
        )
        .route(
            "/api/v1/product-categories/update/{category_id}",
            post(
                handlers::product_categories::update_category
                    .layer(require_permission(&state, "products:categories:write")),
            ),
        )
        .route(
            "/api/v1/product-categories/disable/{category_id}",
            post(
                handlers::product_categories::disable_category
                    .layer(require_permission(&state, "products:categories:write")),
            ),
        )
        .route(
            "/api/v1/product-categories/delete/{category_id}",
            post(
                handlers::product_categories::delete_category
                    .layer(require_permission(&state, "products:categories:write")),
            ),
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
            "/api/v1/products/suggestions",
            get(handlers::products::product_suggestions
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
            "/api/v1/events/list",
            get(handlers::events::list_events.layer(require_permission(&state, "events:read"))),
        )
        .route(
            "/api/v1/events/detail/{event_id}",
            get(handlers::events::event_detail.layer(require_permission(&state, "events:read"))),
        )
        // approve/reject deliberately carry no static permission layer: the
        // permission a reviewer needs is declared per resource type
        // (ReviewableResource::APPROVAL_PERMISSION) and is only known after
        // loading the target event, so EventService::review performs the
        // check dynamically.
        .route(
            "/api/v1/events/approve/{event_id}",
            post(handlers::events::approve_event),
        )
        .route(
            "/api/v1/events/reject/{event_id}",
            post(handlers::events::reject_event),
        )
        .route(
            "/api/v1/departments/list",
            get(handlers::departments::list_departments
                .layer(require_permission(&state, "departments:read"))),
        )
        .route(
            "/api/v1/departments/detail/{department_id}",
            get(handlers::departments::department_detail
                .layer(require_permission(&state, "departments:read"))),
        )
        .route(
            "/api/v1/departments/sync/dingtalk",
            post(
                handlers::departments::sync_dingtalk_departments
                    .layer(require_permission(&state, "departments:write")),
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
            "/api/v1/stores/list",
            get(handlers::stores::list_stores.layer(require_permission(&state, "stores:read"))),
        )
        .route(
            "/api/v1/stores/detail/{store_id}",
            get(handlers::stores::store_detail.layer(require_permission(&state, "stores:read"))),
        )
        .route(
            "/api/v1/stores/create",
            post(handlers::stores::create_store.layer(require_permission(&state, "stores:write"))),
        )
        .route(
            "/api/v1/stores/update/{store_id}",
            post(handlers::stores::update_store.layer(require_permission(&state, "stores:write"))),
        )
        .route(
            "/api/v1/stores/disable/{store_id}",
            post(handlers::stores::disable_store.layer(require_permission(&state, "stores:write"))),
        )
        .route(
            "/api/v1/stores/delete/{store_id}",
            post(handlers::stores::delete_store.layer(require_permission(&state, "stores:write"))),
        )
        .route(
            "/api/v1/customers/list",
            get(handlers::customers::list_customers
                .layer(require_permission(&state, "customers:read"))),
        )
        .route(
            "/api/v1/customers/detail/{customer_id}",
            get(handlers::customers::customer_detail
                .layer(require_permission(&state, "customers:read"))),
        )
        .route(
            "/api/v1/customers/create",
            post(
                handlers::customers::create_customer
                    .layer(require_permission(&state, "customers:write")),
            ),
        )
        .route(
            "/api/v1/customers/update/{customer_id}",
            post(
                handlers::customers::update_customer
                    .layer(require_permission(&state, "customers:write")),
            ),
        )
        .route(
            "/api/v1/customers/disable/{customer_id}",
            post(
                handlers::customers::disable_customer
                    .layer(require_permission(&state, "customers:write")),
            ),
        )
        .route(
            "/api/v1/customers/delete/{customer_id}",
            post(
                handlers::customers::delete_customer
                    .layer(require_permission(&state, "customers:write")),
            ),
        )
        .route(
            "/api/v1/sales-records/list",
            get(handlers::sales_records::list_sales_records
                .layer(require_permission(&state, "sales:records:read"))),
        )
        .route(
            "/api/v1/sales-records/detail/{sales_record_id}",
            get(handlers::sales_records::sales_record_detail
                .layer(require_permission(&state, "sales:records:read"))),
        )
        .route(
            "/api/v1/sales-records/create-sale",
            post(
                handlers::sales_records::create_sale_record
                    .layer(require_permission(&state, "sales:records:write")),
            ),
        )
        .route(
            "/api/v1/sales-records/create-service",
            post(
                handlers::sales_records::create_service_record
                    .layer(require_permission(&state, "sales:records:write")),
            ),
        )
        .route(
            "/api/v1/sales-records/void/{sales_record_id}",
            post(
                handlers::sales_records::void_sales_record
                    .layer(require_permission(&state, "sales:records:write")),
            ),
        )
        .route(
            "/api/v1/sales-payments/list",
            get(handlers::sales_records::list_sales_payments
                .layer(require_permission(&state, "sales:records:read"))),
        )
        .route(
            "/api/v1/sales-payments/detail/{payment_id}",
            get(handlers::sales_records::sales_payment_detail
                .layer(require_permission(&state, "sales:records:read"))),
        )
        .route(
            "/api/v1/sales-payments/collect",
            post(
                handlers::sales_records::create_collection_payment
                    .layer(require_permission(&state, "sales:records:write")),
            ),
        )
        .route(
            "/api/v1/sales-payments/void/{payment_id}",
            post(
                handlers::sales_records::void_sales_payment
                    .layer(require_permission(&state, "sales:records:write")),
            ),
        )
        .route(
            "/api/v1/sales-record-operation-counts/list",
            get(handlers::sales_records::list_operation_counts
                .layer(require_permission(&state, "sales:operation-counts:read"))),
        )
        .route(
            "/api/v1/sales-record-operation-counts/detail/{sales_record_line_id}",
            get(handlers::sales_records::operation_count_detail
                .layer(require_permission(&state, "sales:operation-counts:read"))),
        )
        .route(
            "/api/v1/sales-record-operation-counts/update/{sales_record_line_id}",
            post(
                handlers::sales_records::update_operation_count
                    .layer(require_permission(&state, "sales:operation-counts:write")),
            ),
        )
        .route(
            "/api/v1/sales-record-operation-usages/list",
            get(handlers::sales_records::list_operation_usages
                .layer(require_permission(&state, "sales:operation-usages:read"))),
        )
        .route(
            "/api/v1/sales-record-operation-usages/detail/{usage_id}",
            get(handlers::sales_records::operation_usage_detail
                .layer(require_permission(&state, "sales:operation-usages:read"))),
        )
        .route(
            "/api/v1/sales-record-operation-usages/create",
            post(
                handlers::sales_records::create_operation_usage
                    .layer(require_permission(&state, "sales:operation-usages:write")),
            ),
        )
        .route(
            "/api/v1/sales-record-operation-usages/update/{usage_id}",
            post(
                handlers::sales_records::update_operation_usage
                    .layer(require_permission(&state, "sales:operation-usages:write")),
            ),
        )
        .route(
            "/api/v1/sales-record-operation-usages/void/{usage_id}",
            post(
                handlers::sales_records::void_operation_usage
                    .layer(require_permission(&state, "sales:operation-usages:write")),
            ),
        )
        .route(
            "/api/v1/sales-record-operation-usages/delete/{usage_id}",
            post(
                handlers::sales_records::delete_operation_usage
                    .layer(require_permission(&state, "sales:operation-usages:write")),
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
        .route(
            "/api/v1/permissions/catalog",
            get(handlers::permissions::permission_catalog
                .layer(require_permission(&state, "system:permissions:read"))),
        )
        .route(
            "/api/v1/permissions/subjects/{subjectKind}/{subjectId}/policies",
            put(handlers::permissions::replace_subject_policies
                .layer(require_permission(&state, "system:permissions:write"))),
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
        .layer(from_fn(log_request))
}

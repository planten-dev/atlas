use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    dto::{
        auth::ErrorResponse,
        products::{
            CreateProductRequest, ListProductsQuery, ProductSuggestionsQuery, UpdateProductRequest,
        },
    },
    repositories::RepositoryError,
    services::products::ProductError,
    state::AppState,
};

pub async fn list_products(
    State(state): State<AppState>,
    query: Result<Query<ListProductsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.products.list_products(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => product_error_response(error),
    }
}

pub async fn product_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let product_id = match path {
        Ok(Path(product_id)) => product_id,
        Err(error) => return validation_error_response("invalid product_id path parameter", error),
    };

    match state.products.product_detail(product_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => product_error_response(error),
    }
}

pub async fn product_suggestions(
    State(state): State<AppState>,
    query: Result<Query<ProductSuggestionsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.products.product_suggestions(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => product_error_response(error),
    }
}

pub async fn create_product(
    State(state): State<AppState>,
    request: Result<Json<CreateProductRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state.products.create_product(request).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => product_error_response(error),
    }
}

pub async fn update_product(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateProductRequest>, JsonRejection>,
) -> Response {
    let product_id = match path {
        Ok(Path(product_id)) => product_id,
        Err(error) => return validation_error_response("invalid product_id path parameter", error),
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state.products.update_product(product_id, request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => product_error_response(error),
    }
}

pub async fn disable_product(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let product_id = match path {
        Ok(Path(product_id)) => product_id,
        Err(error) => return validation_error_response("invalid product_id path parameter", error),
    };

    match state.products.disable_product(product_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => product_error_response(error),
    }
}

pub async fn delete_product(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let product_id = match path {
        Ok(Path(product_id)) => product_id,
        Err(error) => return validation_error_response("invalid product_id path parameter", error),
    };

    match state.products.delete_product(product_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => product_error_response(error),
    }
}

fn product_error_response(error: ProductError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "product request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "product request rejected"
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

fn validation_error_response(error: &'static str, detail: impl std::fmt::Display) -> Response {
    warn!(error, detail = %detail, "product request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &ProductError) -> StatusCode {
    match error {
        ProductError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        ProductError::ProductNotFound | ProductError::ProductCategoryNotFound => StatusCode::NOT_FOUND,
        ProductError::ProductHasReferences => StatusCode::CONFLICT,
        ProductError::ProductCategoryDisabled
        | ProductError::MissingRequiredField { .. }
        | ProductError::FieldTooLong { .. }
        | ProductError::InvalidStatus { .. }
        | ProductError::InvalidUnitPrice { .. }
        | ProductError::NegativeUnitPrice { .. }
        | ProductError::UnitPriceTooLarge { .. }
        | ProductError::InvalidPaginationMinimum { .. }
        | ProductError::InvalidPaginationMaximum { .. } => StatusCode::BAD_REQUEST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app,
        config::{AuthConfig, DatabaseConfig, DatabaseKind, DingTalkConfig, SessionConfig},
        db,
        repositories::{
            authz::AuthzRepository,
            customers::{CustomerRepository, NewCustomer},
            departments::DepartmentRepository,
            events::EventRepository,
            product_categories::ProductCategoryRepository,
            products::ProductRepository,
            sales_records::{NewSalesRecord, NewSalesRecordLine, SalesRecordRepository},
            sessions::SessionRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            user_profiles::UserProfileRepository, users::UserRepository,
        },
        services::{
            auth::AuthService, authz::AuthzService, customers::CustomerService,
            departments::DepartmentService, events::EventService,
            product_categories::ProductCategoryService, products::ProductService,
            review::ApplierRegistry, sales_records::SalesRecordService, stores::StoreService,
            systems::SystemService, users::UserService,
        },
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, header},
        routing::{get, post},
    };
    use sea_orm::entity::prelude::Decimal;
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    struct TestContext {
        app: Router,
        users: UserRepository,
        authz: AuthzService,
        product_categories: ProductCategoryRepository,
        products: ProductRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
        sales_records: SalesRecordRepository,
    }

    #[tokio::test]
    async fn products_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .clone()
            .oneshot(request(Method::GET, "/api/v1/products/list", None, None))
            .await
            .expect("products list request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/delete/{}", Uuid::new_v4()),
                None,
                None,
            ))
            .await
            .expect("product delete request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_without_product_permission_is_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/products/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("products list request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_denied")
        );
    }

    #[tokio::test]
    async fn read_permission_allows_read_but_not_write() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "products", "read").await;

        let read_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/products/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("products list request should be handled");
        assert_eq!(read_response.status(), StatusCode::OK);

        let write_response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/products/create",
                Some(&cookie),
                Some(json!({"name": "product-a", "unit_price": "12.30"})),
            ))
            .await
            .expect("product create request should be handled");
        assert_eq!(write_response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn product_crud_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let category_id = default_category_id(&context).await;
        let medical_category_id = medical_category_id(&context).await;
        grant(&context, user_id, "products", "read").await;
        grant(&context, user_id, "products", "write").await;

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/products/create",
                Some(&cookie),
                Some(json!({
                    "name": "product-a",
                    "category_id": category_id,
                    "series": "series-a",
                    "brand_name": "brand-a",
                    "specification": "spec-a",
                    "unit": null,
                    "unit_price": "12.3"
                })),
            ))
            .await
            .expect("product create request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = response_json(create_response).await;
        assert_eq!(
            created.pointer("/name").and_then(Value::as_str),
            Some("product-a")
        );
        assert_eq!(
            created.pointer("/category_id").and_then(Value::as_str),
            Some(category_id.to_string().as_str())
        );
        assert_eq!(
            created.pointer("/category_name").and_then(Value::as_str),
            Some("产品")
        );
        assert_eq!(
            created
                .pointer("/requires_operation_count")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            created.pointer("/unit_price").and_then(Value::as_str),
            Some("12.30")
        );
        assert_eq!(created.pointer("/unit"), Some(&Value::Null));
        let product_id = created
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("product id should be present")
            .to_string();

        let list_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!(
                    "/api/v1/products/list?status_filter=active&category_id={category_id}&page_number=1&page_size=20"
                ),
                Some(&cookie),
                None,
            ))
            .await
            .expect("products list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list = response_json(list_response).await;
        assert_eq!(
            list.pointer("/total_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            list.pointer("/products/0/id").and_then(Value::as_str),
            Some(product_id.as_str())
        );

        let update_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/update/{product_id}"),
                Some(&cookie),
                Some(json!({
                    "name": "product-b",
                    "category_id": medical_category_id,
                    "brand_name": null,
                    "unit": "piece",
                    "unit_price": "25"
                })),
            ))
            .await
            .expect("product update request should be handled");
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated = response_json(update_response).await;
        assert_eq!(
            updated.pointer("/name").and_then(Value::as_str),
            Some("product-b")
        );
        assert_eq!(
            updated.pointer("/category_id").and_then(Value::as_str),
            Some(medical_category_id.to_string().as_str())
        );
        assert_eq!(
            updated.pointer("/category_name").and_then(Value::as_str),
            Some("医疗")
        );
        assert_eq!(
            updated
                .pointer("/requires_operation_count")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(updated.pointer("/brand_name"), Some(&Value::Null));
        assert_eq!(
            updated.pointer("/unit").and_then(Value::as_str),
            Some("piece")
        );
        assert_eq!(
            updated.pointer("/unit_price").and_then(Value::as_str),
            Some("25.00")
        );

        let disable_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/disable/{product_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("product disable request should be handled");
        assert_eq!(disable_response.status(), StatusCode::OK);
        let disabled = response_json(disable_response).await;
        assert_eq!(
            disabled.pointer("/status").and_then(Value::as_str),
            Some("disabled")
        );

        let delete_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/delete/{product_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("product delete request should be handled");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let missing_detail = context
            .app
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/products/detail/{product_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("product detail request should be handled");
        assert_eq!(missing_detail.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn product_search_suggestions_and_reference_delete_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let category_id = default_category_id(&context).await;
        grant(&context, user_id, "products", "read").await;
        grant(&context, user_id, "products", "write").await;

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/products/create",
                Some(&cookie),
                Some(json!({
                    "name": "hydrating serum",
                    "category_id": category_id,
                    "series": "skin line",
                    "brand_name": "atlas lab",
                    "specification": "30ml bottle",
                    "unit": "bottle",
                    "unit_price": "12.30"
                })),
            ))
            .await
            .expect("product create request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = response_json(create_response).await;
        let product_id = created
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("product id should be present")
            .to_string();

        let keyword_list = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/products/list?keyword=30ml",
                Some(&cookie),
                None,
            ))
            .await
            .expect("product keyword list request should be handled");
        assert_eq!(keyword_list.status(), StatusCode::OK);
        assert_eq!(
            response_json(keyword_list)
                .await
                .pointer("/products/0/id")
                .and_then(Value::as_str),
            Some(product_id.as_str())
        );

        let suggestions = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/products/suggestions?keyword=atlas&limit_per_field=10",
                Some(&cookie),
                None,
            ))
            .await
            .expect("product suggestions request should be handled");
        assert_eq!(suggestions.status(), StatusCode::OK);
        let suggestions = response_json(suggestions).await;
        assert_eq!(
            suggestions.pointer("/series/0").and_then(Value::as_str),
            Some("skin line")
        );
        assert_eq!(
            suggestions.pointer("/brand_names/0").and_then(Value::as_str),
            Some("atlas lab")
        );
        assert_eq!(
            suggestions.pointer("/units/0").and_then(Value::as_str),
            Some("bottle")
        );

        let invalid_suggestions = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/products/suggestions?limit_per_field=201",
                Some(&cookie),
                None,
            ))
            .await
            .expect("invalid suggestions request should be handled");
        assert_eq!(invalid_suggestions.status(), StatusCode::BAD_REQUEST);

        create_sales_line_reference(&context, user_id, product_id.parse().unwrap()).await;
        let delete_response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/delete/{product_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("referenced product delete request should be handled");
        assert_eq!(delete_response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(delete_response)
                .await
                .pointer("/error")
                .and_then(Value::as_str),
            Some("product_has_references")
        );
    }

    #[tokio::test]
    async fn product_api_validates_inputs_and_missing_products() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let category_id = default_category_id(&context).await;
        grant(&context, user_id, "products", "read").await;
        grant(&context, user_id, "products", "write").await;

        for body in [
            json!({"name": "", "unit_price": "12.30"}),
            json!({"name": "product-a", "category_id": category_id, "unit_price": "12.345"}),
            json!({"name": "product-a", "category_id": category_id, "unit_price": "-0.01"}),
            json!({"name": "product-a", "category_id": category_id, "unit_price": "12.30", "status": "deleted"}),
        ] {
            let response = context
                .app
                .clone()
                .oneshot(request(
                    Method::POST,
                    "/api/v1/products/create",
                    Some(&cookie),
                    Some(body),
                ))
                .await
                .expect("product create request should be handled");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        let invalid_query = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/products/list?status_filter=deleted",
                Some(&cookie),
                None,
            ))
            .await
            .expect("products list request should be handled");
        assert_eq!(invalid_query.status(), StatusCode::BAD_REQUEST);

        let invalid_category_query = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/products/list?category_id=not-a-uuid",
                Some(&cookie),
                None,
            ))
            .await
            .expect("products list request should be handled");
        assert_eq!(invalid_category_query.status(), StatusCode::BAD_REQUEST);

        let missing_category = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/products/create",
                Some(&cookie),
                Some(json!({
                    "name": "product-a",
                    "category_id": Uuid::new_v4(),
                    "unit_price": "12.30"
                })),
            ))
            .await
            .expect("product create request should be handled");
        assert_eq!(missing_category.status(), StatusCode::NOT_FOUND);

        let disabled_category_id = medical_category_id(&context).await;
        let disabled_category = context
            .product_categories
            .find_by_id(disabled_category_id)
            .await
            .expect("category lookup should succeed")
            .expect("category should exist");
        context
            .product_categories
            .update_status(&disabled_category, "disabled", chrono::Utc::now())
            .await
            .expect("category should disable");
        let disabled_category_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/products/create",
                Some(&cookie),
                Some(json!({
                    "name": "product-a",
                    "category_id": disabled_category_id,
                    "unit_price": "12.30"
                })),
            ))
            .await
            .expect("product create request should be handled");
        assert_eq!(disabled_category_response.status(), StatusCode::BAD_REQUEST);

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/products/create",
                Some(&cookie),
                Some(json!({
                    "name": "product-a",
                    "category_id": category_id,
                    "unit_price": "12.30"
                })),
            ))
            .await
            .expect("product create request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let product = response_json(create_response).await;
        let product_id = product
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("product id should be present");

        let null_category = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/update/{product_id}"),
                Some(&cookie),
                Some(json!({"category_id": null})),
            ))
            .await
            .expect("product update request should be handled");
        assert_eq!(null_category.status(), StatusCode::BAD_REQUEST);

        let invalid_id = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/products/update/not-a-uuid",
                Some(&cookie),
                Some(json!({"name": "product-a"})),
            ))
            .await
            .expect("product update request should be handled");
        assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);

        let missing_delete = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/products/delete/{}", Uuid::new_v4()),
                Some(&cookie),
                None,
            ))
            .await
            .expect("product delete request should be handled");
        assert_eq!(missing_delete.status(), StatusCode::NOT_FOUND);
    }

    async fn test_context(mock_base_url: &str) -> TestContext {
        let database = DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        };
        let db = db::connect_and_migrate(&database)
            .await
            .expect("test database should initialize");
        let users = UserRepository::new(db.clone());
        let profiles = UserProfileRepository::new(db.clone());
        let sessions = SessionRepository::new(db.clone());
        let product_categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db.clone());
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db.clone());
        let customers = CustomerRepository::new(db.clone());
        let sales_records = SalesRecordRepository::new(db.clone());
        let dingtalk_config = DingTalkConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://127.0.0.1:3000/api/v1/auth/callback/dingtalk".to_string(),
            auth_url: "https://login.dingtalk.com/oauth2/auth".to_string(),
            token_url: format!("{mock_base_url}/token"),
            user_info_url: format!("{mock_base_url}/me"),
            corp_token_url: format!("{mock_base_url}/gettoken"),
            department_listsub_url: format!("{mock_base_url}/listsub"),
            user_detail_url: format!("{mock_base_url}/user_detail"),
            getbyunionid_url: format!("{mock_base_url}/getbyunionid"),
            scope: "openid".to_string(),
            corp_id: "".to_string(),
            external_id_fields: vec!["userId".to_string()],
        };
        let auth = AuthService::new(
            dingtalk_config.clone(),
            users.clone(),
            profiles.clone(),
            EventRepository::new(db.clone()),
            sessions.clone(),
            86_400,
        );
        let departments_service = DepartmentService::new(dingtalk_config, departments.clone());
        let authz = AuthzService::new(AuthzRepository::new(db.clone()))
            .await
            .expect("test authz service should initialize");
        let users_service = UserService::new(users.clone(), profiles, sessions);
        let product_categories_service =
            ProductCategoryService::new(product_categories.clone(), products.clone());
        let products_service = ProductService::new(products.clone(), product_categories.clone());
        let stores_service = StoreService::new(stores.clone(), systems.clone());
        let systems_service = SystemService::new(systems.clone(), stores.clone());
        let customers_service =
            CustomerService::new(customers.clone(), systems.clone(), stores.clone());
        let sales_records_service = SalesRecordService::new(
            sales_records.clone(),
            customers.clone(),
            systems.clone(),
            stores.clone(),
            product_categories.clone(),
            users.clone(),
        );
        let events_service = EventService::new(
            EventRepository::new(db),
            authz.clone(),
            std::sync::Arc::new(ApplierRegistry::new()),
            180,
        );
        let state = AppState::new(crate::state::AppStateParts {
            auth,
            authz: authz.clone(),
            users: users_service,
            product_categories: product_categories_service,
            products: products_service,
            systems: systems_service,
            stores: stores_service,
            customers: customers_service,
            sales_records: sales_records_service,
            events: events_service,
            departments: departments_service,
            auth_config: AuthConfig {
                frontend_callback_url: "".to_string(),
            },
            session_config: SessionConfig {
                ttl_seconds: 86_400,
                cookie_secure: false,
            },
        });
        TestContext {
            app: app::router(state),
            users,
            authz,
            product_categories,
            products,
            systems,
            stores,
            customers,
            sales_records,
        }
    }

    async fn create_sales_line_reference(
        context: &TestContext,
        user_id: Uuid,
        product_id: Uuid,
    ) {
        let now = chrono::Utc::now();
        let system = context
            .systems
            .create_system(
                NewSystem {
                    name: "reference system".to_string(),
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("system should be created");
        let store = context
            .stores
            .create_store(
                NewStore {
                    name: "reference store".to_string(),
                    system_id: system.id,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("store should be created");
        let customer = context
            .customers
            .create_customer(
                NewCustomer {
                    name: "reference customer".to_string(),
                    creator_user_id: user_id,
                    system_id: system.id,
                    store_id: store.id,
                    remark: None,
                    status: "active".to_string(),
                    attachments: None,
                },
                now,
            )
            .await
            .expect("customer should be created");
        let record = context
            .sales_records
            .insert_sales_record(
                &context.products.db,
                NewSalesRecord {
                    record_type: "sale".to_string(),
                    customer_id: customer.id,
                    record_date: now.date_naive(),
                    customer_type: Some("new".to_string()),
                    deal_type: Some("non_salon".to_string()),
                    system_id: system.id,
                    store_id: store.id,
                    handler_user_id: user_id,
                    expert_user_id: None,
                    consultant_user_id: None,
                    doctor_user_id: None,
                    remark: None,
                    status: "active".to_string(),
                    created_by_user_id: user_id,
                },
                now,
            )
            .await
            .expect("sales record should be inserted");
        context
            .sales_records
            .insert_sales_record_line(
                &context.products.db,
                NewSalesRecordLine {
                    sales_record_id: record.id,
                    product_id,
                    item_name: "referenced".to_string(),
                    receivable_amount: Decimal::new(10000, 2),
                    operation_total_count: None,
                    remark: None,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("sales line should be inserted");
    }

    async fn start_mock_dingtalk() -> String {
        async fn token() -> Json<Value> {
            Json(json!({
                "accessToken": "provider-token",
                "userId": "ding-user-1"
            }))
        }

        async fn me() -> Json<Value> {
            Json(json!({
                "result": {
                    "userId": "ding-user-1"
                }
            }))
        }

        let app = Router::new()
            .route("/token", post(token))
            .route("/me", get(me));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock DingTalk listener should bind");
        let addr = listener.local_addr().expect("mock address should be known");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock DingTalk server should run");
        });
        format!("http://{addr}")
    }

    async fn login_and_cookie(app: Router) -> String {
        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/login/dingtalk")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("login should be handled");
        let location = login_response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .expect("login should include location");
        let state = query_param(location, "state").expect("state should be present");

        let callback_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/auth/callback/dingtalk?code=test-code&state={state}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("callback should be handled");

        callback_response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|cookie| cookie.split(';').next())
            .expect("callback should include cookie")
            .to_string()
    }

    async fn logged_in_user_id(context: &TestContext) -> Uuid {
        context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should work")
            .expect("logged in user should exist")
            .id
    }

    async fn default_category_id(context: &TestContext) -> Uuid {
        context
            .product_categories
            .find_by_category_name("产品")
            .await
            .expect("category lookup should work")
            .expect("default category should exist")
            .id
    }

    async fn medical_category_id(context: &TestContext) -> Uuid {
        context
            .product_categories
            .find_by_category_name("医疗")
            .await
            .expect("category lookup should work")
            .expect("medical category should exist")
            .id
    }

    async fn grant(context: &TestContext, user_id: Uuid, object: &str, action: &str) {
        context
            .authz
            .create_policy(
                "user".to_string(),
                user_id,
                object.to_string(),
                action.to_string(),
                "allow".to_string(),
            )
            .await
            .expect("seed policy should be created");
    }

    fn request(
        method: Method,
        uri: &str,
        cookie: Option<&str>,
        body: Option<Value>,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        }
    }

    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&body).expect("response body should be json")
    }

    fn query_param(url: &str, name: &str) -> Option<String> {
        let query = url.split_once('?')?.1;
        query.split('&').find_map(|part| {
            let (key, value) = part.split_once('=')?;
            if key == name {
                Some(value.to_string())
            } else {
                None
            }
        })
    }
}

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
        sales_records::{
            CreateOperationUsageRequest, CreateSalesRecordBatchRequest, ListOperationCountsQuery,
            ListOperationUsagesQuery, ListSalesRecordsQuery, UpdateOperationCountRequest,
            UpdateOperationUsageRequest, UpdateSalesRecordRequest,
        },
    },
    repositories::RepositoryError,
    services::sales_records::SalesRecordError,
    state::AppState,
};

pub async fn list_sales_records(
    State(state): State<AppState>,
    query: Result<Query<ListSalesRecordsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.sales_records.list_sales_records(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn sales_record_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let sales_record_id = match path {
        Ok(Path(sales_record_id)) => sales_record_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_id path parameter", error);
        }
    };

    match state
        .sales_records
        .sales_record_detail(sales_record_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn create_sales_record_batch(
    State(state): State<AppState>,
    request: Result<Json<CreateSalesRecordBatchRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state.sales_records.create_sales_record_batch(request).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn update_sales_record(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateSalesRecordRequest>, JsonRejection>,
) -> Response {
    let sales_record_id = match path {
        Ok(Path(sales_record_id)) => sales_record_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_id path parameter", error);
        }
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .update_sales_record(sales_record_id, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn void_sales_record(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let sales_record_id = match path {
        Ok(Path(sales_record_id)) => sales_record_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_id path parameter", error);
        }
    };

    match state.sales_records.void_sales_record(sales_record_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn delete_sales_record(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let sales_record_id = match path {
        Ok(Path(sales_record_id)) => sales_record_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_id path parameter", error);
        }
    };

    match state
        .sales_records
        .delete_sales_record(sales_record_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn list_operation_counts(
    State(state): State<AppState>,
    query: Result<Query<ListOperationCountsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.sales_records.list_operation_counts(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn operation_count_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let sales_record_id = match path {
        Ok(Path(sales_record_id)) => sales_record_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_id path parameter", error);
        }
    };

    match state
        .sales_records
        .operation_count_detail(sales_record_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn update_operation_count(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateOperationCountRequest>, JsonRejection>,
) -> Response {
    let sales_record_id = match path {
        Ok(Path(sales_record_id)) => sales_record_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_id path parameter", error);
        }
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .update_operation_count(sales_record_id, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn list_operation_usages(
    State(state): State<AppState>,
    query: Result<Query<ListOperationUsagesQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.sales_records.list_operation_usages(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn operation_usage_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let usage_id = match path {
        Ok(Path(usage_id)) => usage_id,
        Err(error) => return validation_error_response("invalid usage_id path parameter", error),
    };

    match state.sales_records.operation_usage_detail(usage_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn create_operation_usage(
    State(state): State<AppState>,
    request: Result<Json<CreateOperationUsageRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state.sales_records.create_operation_usage(request).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn update_operation_usage(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateOperationUsageRequest>, JsonRejection>,
) -> Response {
    let usage_id = match path {
        Ok(Path(usage_id)) => usage_id,
        Err(error) => return validation_error_response("invalid usage_id path parameter", error),
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .update_operation_usage(usage_id, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn void_operation_usage(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let usage_id = match path {
        Ok(Path(usage_id)) => usage_id,
        Err(error) => return validation_error_response("invalid usage_id path parameter", error),
    };

    match state.sales_records.void_operation_usage(usage_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn delete_operation_usage(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let usage_id = match path {
        Ok(Path(usage_id)) => usage_id,
        Err(error) => return validation_error_response("invalid usage_id path parameter", error),
    };

    match state.sales_records.delete_operation_usage(usage_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

fn sales_record_error_response(error: SalesRecordError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "sales record request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "sales record request rejected"
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
    warn!(error, detail = %detail, "sales record request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &SalesRecordError) -> StatusCode {
    match error {
        SalesRecordError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        SalesRecordError::SalesRecordNotFound
        | SalesRecordError::OperationCountNotFound
        | SalesRecordError::OperationUsageNotFound
        | SalesRecordError::CustomerNotFound
        | SalesRecordError::SystemNotFound
        | SalesRecordError::StoreNotFound
        | SalesRecordError::ProductCategoryNotFound
        | SalesRecordError::UserNotFound { .. } => StatusCode::NOT_FOUND,
        SalesRecordError::OperationCountInsufficient
        | SalesRecordError::OperationCountBelowUsed
        | SalesRecordError::SalesRecordHasActiveUsages
        | SalesRecordError::SalesRecordVoided
        | SalesRecordError::OperationCountVoided
        | SalesRecordError::OperationUsageVoided => StatusCode::CONFLICT,
        SalesRecordError::CustomerDisabled
        | SalesRecordError::ProductCategoryDisabled
        | SalesRecordError::ReferencedUserDisabled { .. }
        | SalesRecordError::StoreSystemMismatch
        | SalesRecordError::EmptyBatch
        | SalesRecordError::MissingRequiredField { .. }
        | SalesRecordError::FieldTooLong { .. }
        | SalesRecordError::InvalidEnumValue { .. }
        | SalesRecordError::InvalidMoney { .. }
        | SalesRecordError::NegativeMoney { .. }
        | SalesRecordError::MoneyTooLarge { .. }
        | SalesRecordError::InvalidCountMinimum { .. }
        | SalesRecordError::OperationTotalCountRequired
        | SalesRecordError::OperationTotalCountNotAllowed
        | SalesRecordError::MissingExpertFields
        | SalesRecordError::UnexpectedExpertFields
        | SalesRecordError::ContentCategoryOperationCountMismatch
        | SalesRecordError::InvalidPaginationMinimum { .. }
        | SalesRecordError::InvalidPaginationMaximum { .. } => StatusCode::BAD_REQUEST,
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
            events::EventRepository,
            product_categories::ProductCategoryRepository,
            products::ProductRepository,
            sales_records::SalesRecordRepository,
            sessions::SessionRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            user_profiles::UserProfileRepository,
            users::UserRepository,
        },
        services::{
            auth::AuthService, authz::AuthzService, customers::CustomerService,
            events::EventService, product_categories::ProductCategoryService,
            products::ProductService, review::ApplierRegistry, sales_records::SalesRecordService,
            stores::StoreService, systems::SystemService, users::UserService,
        },
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, header},
        routing::{get, post},
    };
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    struct TestContext {
        app: Router,
        users: UserRepository,
        authz: AuthzService,
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
        categories: ProductCategoryRepository,
    }

    #[tokio::test]
    async fn sales_record_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/sales-records/list",
                None,
                None,
            ))
            .await
            .expect("sales records list request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!(
                    "/api/v1/sales-record-operation-usages/delete/{}",
                    Uuid::new_v4()
                ),
                None,
                None,
            ))
            .await
            .expect("operation usage delete request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sales_record_permissions_are_enforced() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let (system_id, store_id) = create_scope(&context, "scope-a").await;
        let customer_id = create_customer(&context, user_id, system_id, store_id).await;
        let category_id = operation_category(&context).await;

        let forbidden = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/sales-records/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("sales list request should be handled");
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        grant(&context, user_id, "sales:records", "read").await;
        let read_ok = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/sales-records/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("sales list request should be handled");
        assert_eq!(read_ok.status(), StatusCode::OK);

        let write_forbidden = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-records/create-batch",
                Some(&cookie),
                Some(create_batch_body(
                    customer_id,
                    system_id,
                    store_id,
                    category_id,
                    user_id,
                    Some(2),
                )),
            ))
            .await
            .expect("sales create request should be handled");
        assert_eq!(write_forbidden.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn sales_record_operation_flow_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant_all_sales_permissions(&context, user_id).await;
        let (system_id, store_id) = create_scope(&context, "scope-a").await;
        let customer_id = create_customer(&context, user_id, system_id, store_id).await;
        let category_id = operation_category(&context).await;

        let mut legacy_body = create_batch_body(
            customer_id,
            system_id,
            store_id,
            category_id,
            user_id,
            Some(2),
        );
        legacy_body["records"][0]["department_id"] = json!(Uuid::new_v4());
        let legacy_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-records/create-batch",
                Some(&cookie),
                Some(legacy_body),
            ))
            .await
            .expect("legacy sales create request should be handled");
        assert_eq!(legacy_response.status(), StatusCode::BAD_REQUEST);

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-records/create-batch",
                Some(&cookie),
                Some(create_batch_body(
                    customer_id,
                    system_id,
                    store_id,
                    category_id,
                    user_id,
                    Some(2),
                )),
            ))
            .await
            .expect("sales create request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = response_json(create_response).await;
        assert!(created.pointer("/sales_records/0/department_id").is_none());
        let sales_record_id = created
            .pointer("/sales_records/0/id")
            .and_then(Value::as_str)
            .expect("sales record id should exist")
            .to_string();
        assert_eq!(
            created
                .pointer("/sales_records/0/operation_count/total_count")
                .and_then(Value::as_i64),
            Some(2)
        );

        let count_detail = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/sales-record-operation-counts/detail/{sales_record_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("count detail request should be handled");
        assert_eq!(count_detail.status(), StatusCode::OK);

        let usage_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-record-operation-usages/create",
                Some(&cookie),
                Some(json!({
                    "sales_record_id": sales_record_id,
                    "operated_at": "2026-07-08T09:00:00Z",
                    "operator_user_id": user_id,
                    "operation_count": 1,
                    "remark": "first"
                })),
            ))
            .await
            .expect("usage create request should be handled");
        assert_eq!(usage_response.status(), StatusCode::CREATED);
        let usage = response_json(usage_response).await;
        let usage_id = usage
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("usage id should exist")
            .to_string();

        let update_usage = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/sales-record-operation-usages/update/{usage_id}"),
                Some(&cookie),
                Some(json!({"operation_count": 2, "remark": null})),
            ))
            .await
            .expect("usage update request should be handled");
        assert_eq!(update_usage.status(), StatusCode::OK);
        let updated_usage = response_json(update_usage).await;
        assert_eq!(
            updated_usage
                .pointer("/operation_count")
                .and_then(Value::as_i64),
            Some(2)
        );

        let list_usages = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!(
                    "/api/v1/sales-record-operation-usages/list?sales_record_id={sales_record_id}"
                ),
                Some(&cookie),
                None,
            ))
            .await
            .expect("usages list request should be handled");
        assert_eq!(list_usages.status(), StatusCode::OK);
        assert_eq!(
            response_json(list_usages)
                .await
                .pointer("/total_count")
                .and_then(Value::as_u64),
            Some(1)
        );

        let void_usage = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/sales-record-operation-usages/void/{usage_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("usage void request should be handled");
        assert_eq!(void_usage.status(), StatusCode::OK);

        let delete_usage = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/sales-record-operation-usages/delete/{usage_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("usage delete request should be handled");
        assert_eq!(delete_usage.status(), StatusCode::NO_CONTENT);

        let void_record = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/sales-records/void/{sales_record_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("sales void request should be handled");
        assert_eq!(void_record.status(), StatusCode::OK);
        assert_eq!(
            response_json(void_record)
                .await
                .pointer("/status")
                .and_then(Value::as_str),
            Some("voided")
        );

        let delete_record = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/sales-records/delete/{sales_record_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("sales delete request should be handled");
        assert_eq!(delete_record.status(), StatusCode::NO_CONTENT);
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
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db.clone());
        let customers = CustomerRepository::new(db.clone());
        let sales_records = SalesRecordRepository::new(db.clone());
        let auth = AuthService::new(
            DingTalkConfig {
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
            },
            users.clone(),
            profiles.clone(),
            EventRepository::new(db.clone()),
            sessions.clone(),
            86_400,
        );
        let authz = AuthzService::new(AuthzRepository::new(db.clone()))
            .await
            .expect("test authz service should initialize");
        let users_service = UserService::new(users.clone(), profiles, sessions);
        let product_categories_service =
            ProductCategoryService::new(product_categories.clone(), products.clone());
        let products_service = ProductService::new(products, product_categories.clone());
        let stores_service = StoreService::new(stores.clone(), systems.clone());
        let systems_service = SystemService::new(systems.clone(), stores.clone());
        let customers_service =
            CustomerService::new(customers.clone(), systems.clone(), stores.clone());
        let sales_records_service = SalesRecordService::new(
            sales_records,
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
        let state = AppState::new(
            auth,
            authz.clone(),
            users_service,
            product_categories_service,
            products_service,
            systems_service,
            stores_service,
            customers_service,
            sales_records_service,
            events_service,
            AuthConfig {
                frontend_callback_url: "".to_string(),
            },
            SessionConfig {
                ttl_seconds: 86_400,
                cookie_secure: false,
            },
        );
        TestContext {
            app: app::router(state),
            users,
            authz,
            systems,
            stores,
            customers,
            categories: product_categories,
        }
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

    async fn create_scope(context: &TestContext, name: &str) -> (Uuid, Uuid) {
        let system = context
            .systems
            .create_system(
                NewSystem {
                    name: name.to_string(),
                    status: "active".to_string(),
                },
                chrono::Utc::now(),
            )
            .await
            .expect("system should be created");
        let store = context
            .stores
            .create_store(
                NewStore {
                    name: name.to_string(),
                    system_id: system.id,
                    status: "active".to_string(),
                },
                chrono::Utc::now(),
            )
            .await
            .expect("store should be created");
        (system.id, store.id)
    }

    async fn create_customer(
        context: &TestContext,
        user_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
    ) -> Uuid {
        context
            .customers
            .create_customer(
                NewCustomer {
                    name: "Alice".to_string(),
                    creator_user_id: user_id,
                    system_id,
                    store_id,
                    remark: None,
                    status: "active".to_string(),
                    attachments: None,
                },
                chrono::Utc::now(),
            )
            .await
            .expect("customer should be created")
            .id
    }

    async fn operation_category(context: &TestContext) -> Uuid {
        context
            .categories
            .list_categories(Some("active"), Some(true), 1, 50)
            .await
            .expect("categories should list")
            .0[0]
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

    async fn grant_all_sales_permissions(context: &TestContext, user_id: Uuid) {
        for (object, action) in [
            ("sales:records", "read"),
            ("sales:records", "write"),
            ("sales:operation-counts", "read"),
            ("sales:operation-counts", "write"),
            ("sales:operation-usages", "read"),
            ("sales:operation-usages", "write"),
        ] {
            grant(context, user_id, object, action).await;
        }
    }

    fn create_batch_body(
        customer_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
        category_id: Uuid,
        handler_user_id: Uuid,
        operation_total_count: Option<i32>,
    ) -> Value {
        json!({
            "records": [{
                "customer_id": customer_id,
                "sale_date": "2026-07-08",
                "deal_status": "closed",
                "customer_type": "new",
                "deal_type": "non_salon",
                "content_category_id": category_id,
                "handler_user_id": handler_user_id,
                "paid_amount": "100.00",
                "unpaid_amount": "0.00",
                "system_id": system_id,
                "store_id": store_id,
                "collaboration_type": "self_sale",
                "operation_total_count": operation_total_count
            }]
        })
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

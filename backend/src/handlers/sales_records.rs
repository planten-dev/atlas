use axum::{
    Extension, Json,
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
            CreateCollectionPaymentRequest, CreateOperationUsageRequest, CreateSaleRecordRequest,
            CreateServiceRecordRequest, ListOperationCountsQuery, ListOperationUsagesQuery,
            ListSalesPaymentsQuery, ListSalesRecordsQuery, UpdateOperationCountRequest,
            UpdateOperationUsageRequest,
        },
    },
    repositories::RepositoryError,
    services::{auth::CurrentSession, sales_records::SalesRecordError},
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

pub async fn create_sale_record(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    request: Result<Json<CreateSaleRecordRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .create_sale_record(current_session.user.id, request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn create_service_record(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    request: Result<Json<CreateServiceRecordRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .create_service_record(current_session.user.id, request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
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

pub async fn list_sales_payments(
    State(state): State<AppState>,
    query: Result<Query<ListSalesPaymentsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.sales_records.list_sales_payments(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn sales_payment_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let payment_id = match path {
        Ok(Path(payment_id)) => payment_id,
        Err(error) => return validation_error_response("invalid payment_id path parameter", error),
    };

    match state.sales_records.sales_payment_detail(payment_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn create_collection_payment(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    request: Result<Json<CreateCollectionPaymentRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .create_collection_payment(current_session.user.id, request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => sales_record_error_response(error),
    }
}

pub async fn void_sales_payment(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let payment_id = match path {
        Ok(Path(payment_id)) => payment_id,
        Err(error) => return validation_error_response("invalid payment_id path parameter", error),
    };

    match state.sales_records.void_sales_payment(payment_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
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
    let sales_record_line_id = match path {
        Ok(Path(sales_record_line_id)) => sales_record_line_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_line_id path parameter", error);
        }
    };

    match state
        .sales_records
        .operation_count_detail(sales_record_line_id)
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
    let sales_record_line_id = match path {
        Ok(Path(sales_record_line_id)) => sales_record_line_id,
        Err(error) => {
            return validation_error_response("invalid sales_record_line_id path parameter", error);
        }
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .sales_records
        .update_operation_count(sales_record_line_id, request)
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
        | SalesRecordError::SalesRecordLineNotFound
        | SalesRecordError::SalesPaymentNotFound
        | SalesRecordError::OperationCountNotFound
        | SalesRecordError::OperationUsageNotFound
        | SalesRecordError::CustomerNotFound
        | SalesRecordError::SystemNotFound
        | SalesRecordError::StoreNotFound
        | SalesRecordError::ProductNotFound
        | SalesRecordError::ProductCategoryNotFound
        | SalesRecordError::UserNotFound { .. } => StatusCode::NOT_FOUND,
        SalesRecordError::PaymentExceedsOutstanding
        | SalesRecordError::CollectionRequiresSaleRecord
        | SalesRecordError::OperationCountRequiresSaleRecord
        | SalesRecordError::SalesRecordVoided
        | SalesRecordError::SalesRecordLineVoided
        | SalesRecordError::OperationCountVoided
        | SalesRecordError::OperationUsageVoided
        | SalesRecordError::OperationCountInsufficient
        | SalesRecordError::OperationCountBelowUsed
        | SalesRecordError::SalesRecordHasActiveUsages => StatusCode::CONFLICT,
        SalesRecordError::CustomerDisabled
        | SalesRecordError::SystemDisabled
        | SalesRecordError::StoreDisabled
        | SalesRecordError::ProductDisabled
        | SalesRecordError::ProductCategoryDisabled
        | SalesRecordError::ReferencedUserDisabled { .. }
        | SalesRecordError::StoreSystemMismatch
        | SalesRecordError::SalesRecordLinesRequired
        | SalesRecordError::PaymentAllocationsRequired
        | SalesRecordError::DuplicatePaymentGuide
        | SalesRecordError::MissingRequiredField { .. }
        | SalesRecordError::FieldTooLong { .. }
        | SalesRecordError::InvalidEnumValue { .. }
        | SalesRecordError::InvalidMoney { .. }
        | SalesRecordError::NegativeMoney { .. }
        | SalesRecordError::NonPositiveMoney { .. }
        | SalesRecordError::MoneyTooLarge { .. }
        | SalesRecordError::InvalidRatio { .. }
        | SalesRecordError::RatioOutOfRange { .. }
        | SalesRecordError::PaymentAllocationRatioTotalInvalid
        | SalesRecordError::PaymentAllocationAmountTotalInvalid
        | SalesRecordError::InvalidCountMinimum { .. }
        | SalesRecordError::OperationTotalCountRequired
        | SalesRecordError::OperationTotalCountNotAllowed
        | SalesRecordError::ServiceOperationCountNotAllowed
        | SalesRecordError::SaleReceivableRequired
        | SalesRecordError::ServiceReceivableMustBeZero
        | SalesRecordError::InvalidPaginationMinimum { .. }
        | SalesRecordError::InvalidPaginationMaximum { .. }
        | SalesRecordError::InvalidRecordTypeInternal => StatusCode::BAD_REQUEST,
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
            products::{NewProduct, ProductRepository},
            sales_records::SalesRecordRepository,
            sessions::SessionRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            user_profiles::UserProfileRepository,
            users::UserRepository,
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
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
        categories: ProductCategoryRepository,
        products: ProductRepository,
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
                "/api/v1/sales-records/create-sale",
                None,
                Some(json!({})),
            ))
            .await
            .expect("sales create request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sales_record_permissions_are_enforced() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;

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
                "/api/v1/sales-records/create-sale",
                Some(&cookie),
                Some(json!({})),
            ))
            .await
            .expect("sales create request should be handled");
        assert_eq!(write_forbidden.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn sales_record_and_payment_flow_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant_all_sales_permissions(&context, user_id).await;
        let (system_id, store_id) = create_scope(&context, "scope-a").await;
        let customer_id = create_customer(&context, user_id, system_id, store_id).await;
        let product_id = create_product(&context, true).await;

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-records/create-sale",
                Some(&cookie),
                Some(json!({
                    "customer_id": customer_id,
                    "record_date": "2026-07-08",
                    "customer_type": "new",
                    "deal_type": "non_salon",
                    "handler_user_id": user_id,
                    "lines": [{
                        "product_id": product_id,
                        "item_name": "operation item",
                        "receivable_amount": "300.00",
                        "operation_total_count": 3
                    }],
                    "payment": {
                        "paid_amount": "100.00",
                        "paid_at": "2026-07-08T10:00:00Z",
                        "allocations": [{
                            "guide_user_id": user_id,
                            "allocation_ratio": "100.00"
                        }]
                    }
                })),
            ))
            .await
            .expect("sales create request should be handled");
        let create_status = create_response.status();
        let created = response_json(create_response).await;
        assert_eq!(create_status, StatusCode::CREATED, "{created}");
        let sales_record_id = created
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("sales record id should exist")
            .to_string();
        let sales_record_line_id = created
            .pointer("/lines/0/id")
            .and_then(Value::as_str)
            .expect("line id should exist")
            .to_string();
        assert_eq!(
            created
                .pointer("/outstanding_amount")
                .and_then(Value::as_str),
            Some("200.00")
        );

        let collect_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-payments/collect",
                Some(&cookie),
                Some(json!({
                    "sales_record_id": sales_record_id,
                    "paid_amount": "200.00",
                    "paid_at": "2026-07-09T10:00:00Z",
                    "allocations": [{
                        "guide_user_id": user_id,
                        "allocation_ratio": "100.00"
                    }]
                })),
            ))
            .await
            .expect("collection request should be handled");
        assert_eq!(collect_response.status(), StatusCode::CREATED);
        let collection = response_json(collect_response).await;
        let payment_id = collection
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("payment id should exist")
            .to_string();
        assert_eq!(
            collection.pointer("/payment_type").and_then(Value::as_str),
            Some("collection")
        );

        let count_detail = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/sales-record-operation-counts/detail/{sales_record_line_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("operation count detail should be handled");
        assert_eq!(count_detail.status(), StatusCode::OK);

        let usage_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/sales-record-operation-usages/create",
                Some(&cookie),
                Some(json!({
                    "sales_record_line_id": sales_record_line_id,
                    "operated_at": "2026-07-09T09:00:00Z",
                    "operator_user_id": user_id,
                    "operation_count": 1
                })),
            ))
            .await
            .expect("operation usage create should be handled");
        assert_eq!(usage_response.status(), StatusCode::CREATED);

        let void_payment = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/sales-payments/void/{payment_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("payment void should be handled");
        assert_eq!(void_payment.status(), StatusCode::OK);
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
            user_getuserinfo_url: format!("{mock_base_url}/getuserinfo"),
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
                absolute_ttl_seconds: 604_800,
                renew_before_seconds: 7_200,
                cookie_secure: false,
            },
        });
        TestContext {
            app: app::router(state),
            users,
            authz,
            systems,
            stores,
            customers,
            categories: product_categories,
            products,
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

    async fn create_product(context: &TestContext, requires_operation_count: bool) -> Uuid {
        let category = context
            .categories
            .list_categories(Some("active"), Some(requires_operation_count), 1, 50)
            .await
            .expect("categories should list")
            .0[0]
            .clone();
        context
            .products
            .create_product(
                NewProduct {
                    name: "operation item".to_string(),
                    category_id: category.id,
                    series: None,
                    brand_name: None,
                    specification: None,
                    unit: Some("unit".to_string()),
                    unit_price: Decimal::new(10000, 2),
                    status: "active".to_string(),
                },
                chrono::Utc::now(),
            )
            .await
            .expect("product should be created")
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

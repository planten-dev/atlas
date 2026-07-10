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
        customers::{CreateCustomerRequest, ListCustomersQuery, UpdateCustomerRequest},
    },
    repositories::RepositoryError,
    services::{auth::CurrentSession, customers::CustomerError},
    state::AppState,
};

pub async fn list_customers(
    State(state): State<AppState>,
    query: Result<Query<ListCustomersQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.customers.list_customers(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => customer_error_response(error),
    }
}

pub async fn customer_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let customer_id = match path {
        Ok(Path(customer_id)) => customer_id,
        Err(error) => {
            return validation_error_response("invalid customer_id path parameter", error);
        }
    };

    match state.customers.customer_detail(customer_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => customer_error_response(error),
    }
}

pub async fn create_customer(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    request: Result<Json<CreateCustomerRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .customers
        .create_customer(current_session.user.id, request)
        .await
    {
        Ok(response) => match state
            .events
            .record_create(
                "customers",
                current_session.user.id,
                Some(response.id),
                &response,
            )
            .await
        {
            Ok(_) => (StatusCode::CREATED, Json(response)).into_response(),
            Err(error) => crate::handlers::events::event_error_response(error),
        },
        Err(error) => customer_error_response(error),
    }
}

pub async fn update_customer(
    State(state): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateCustomerRequest>, JsonRejection>,
) -> Response {
    let customer_id = match path {
        Ok(Path(customer_id)) => customer_id,
        Err(error) => {
            return validation_error_response("invalid customer_id path parameter", error);
        }
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    let old = match state.customers.customer_detail(customer_id).await {
        Ok(v) => v,
        Err(e) => return customer_error_response(e),
    };
    match state.customers.update_customer(customer_id, request).await {
        Ok(response) => match state
            .events
            .record_update("customers", session.user.id, customer_id, &old, &response)
            .await
        {
            Ok(_) => (StatusCode::OK, Json(response)).into_response(),
            Err(error) => crate::handlers::events::event_error_response(error),
        },
        Err(error) => customer_error_response(error),
    }
}

pub async fn disable_customer(
    State(state): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let customer_id = match path {
        Ok(Path(customer_id)) => customer_id,
        Err(error) => {
            return validation_error_response("invalid customer_id path parameter", error);
        }
    };

    let old = match state.customers.customer_detail(customer_id).await {
        Ok(v) => v,
        Err(e) => return customer_error_response(e),
    };
    match state.customers.disable_customer(customer_id).await {
        Ok(response) => match state
            .events
            .record_update("customers", session.user.id, customer_id, &old, &response)
            .await
        {
            Ok(_) => (StatusCode::OK, Json(response)).into_response(),
            Err(error) => crate::handlers::events::event_error_response(error),
        },
        Err(error) => customer_error_response(error),
    }
}

pub async fn delete_customer(
    State(state): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let customer_id = match path {
        Ok(Path(customer_id)) => customer_id,
        Err(error) => {
            return validation_error_response("invalid customer_id path parameter", error);
        }
    };

    let old = match state.customers.customer_detail(customer_id).await {
        Ok(v) => v,
        Err(e) => return customer_error_response(e),
    };
    match state.customers.delete_customer(customer_id).await {
        Ok(()) => match state
            .events
            .record_delete("customers", session.user.id, customer_id, &old)
            .await
        {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => crate::handlers::events::event_error_response(error),
        },
        Err(error) => customer_error_response(error),
    }
}

fn customer_error_response(error: CustomerError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "customer request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "customer request rejected"
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
    warn!(error, detail = %detail, "customer request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &CustomerError) -> StatusCode {
    match error {
        CustomerError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        CustomerError::CustomerNotFound
        | CustomerError::SystemNotFound
        | CustomerError::StoreNotFound => StatusCode::NOT_FOUND,
        CustomerError::StoreSystemMismatch
        | CustomerError::MissingRequiredField { .. }
        | CustomerError::FieldTooLong { .. }
        | CustomerError::InvalidStatus { .. }
        | CustomerError::InvalidAttachmentMimeType { .. }
        | CustomerError::InvalidPaginationMinimum { .. }
        | CustomerError::InvalidPaginationMaximum { .. } => StatusCode::BAD_REQUEST,
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
            authz::AuthzRepository, customers::CustomerRepository,
            departments::DepartmentRepository, events::EventRepository,
            product_categories::ProductCategoryRepository, products::ProductRepository,
            sales_records::SalesRecordRepository, sessions::SessionRepository,
            stores::StoreRepository, systems::SystemRepository,
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
    }

    #[tokio::test]
    async fn customers_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .clone()
            .oneshot(request(Method::GET, "/api/v1/customers/list", None, None))
            .await
            .expect("customers list request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/customers/delete/{}", Uuid::new_v4()),
                None,
                None,
            ))
            .await
            .expect("customer delete request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_without_customer_permission_is_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/customers/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("customers list request should be handled");

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
        let (_, store_id) = create_scope(&context, "scope-a").await;
        grant(&context, user_id, "customers", "read").await;

        let read_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/customers/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("customers list request should be handled");
        assert_eq!(read_response.status(), StatusCode::OK);

        let write_response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/customers/create",
                Some(&cookie),
                Some(json!({
                    "name": "Alice",
                    "store_id": store_id
                })),
            ))
            .await
            .expect("customer create request should be handled");
        assert_eq!(write_response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn customer_crud_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let (system_a, store_a) = create_scope(&context, "scope-a").await;
        let (system_b, store_b) = create_scope(&context, "scope-b").await;
        grant(&context, user_id, "customers", "read").await;
        grant(&context, user_id, "customers", "write").await;

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/customers/create",
                Some(&cookie),
                Some(json!({
                    "name": "Alice",
                    "store_id": store_a
                })),
            ))
            .await
            .expect("customer create request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = response_json(create_response).await;
        assert_eq!(
            created.pointer("/name").and_then(Value::as_str),
            Some("Alice")
        );
        assert_eq!(
            created.pointer("/creator_user_id").and_then(Value::as_str),
            Some(user_id.to_string().as_str())
        );
        assert!(created.pointer("/department_id").is_none());
        assert_eq!(
            created.pointer("/system_id").and_then(Value::as_str),
            Some(system_a.to_string().as_str())
        );
        assert_eq!(
            created
                .pointer("/attachments")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        let customer_id = created
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("customer id should be present")
            .to_string();

        let list_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!(
                    "/api/v1/customers/list?status_filter=active&system_id={system_a}&store_id={store_a}&creator_user_id={user_id}&name_keyword=Ali&page_number=1&page_size=20"
                ),
                Some(&cookie),
                None,
            ))
            .await
            .expect("customers list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list = response_json(list_response).await;
        assert_eq!(
            list.pointer("/total_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            list.pointer("/customers/0/id").and_then(Value::as_str),
            Some(customer_id.as_str())
        );

        let update_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/customers/update/{customer_id}"),
                Some(&cookie),
                Some(json!({
                    "name": "Alice Updated",
                    "system_id": system_b,
                    "store_id": store_b,
                    "remark": null,
                    "attachments": null
                })),
            ))
            .await
            .expect("customer update request should be handled");
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated = response_json(update_response).await;
        assert_eq!(
            updated.pointer("/name").and_then(Value::as_str),
            Some("Alice Updated")
        );
        assert!(updated.pointer("/remark").is_some_and(Value::is_null));
        assert_eq!(
            updated
                .pointer("/attachments")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );

        let disable_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/customers/disable/{customer_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("customer disable request should be handled");
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
                &format!("/api/v1/customers/delete/{customer_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("customer delete request should be handled");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let missing_detail = context
            .app
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/customers/detail/{customer_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("customer detail request should be handled");
        assert_eq!(missing_detail.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn customer_api_validates_inputs_and_missing_resources() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let (_, store_id) = create_scope(&context, "scope-a").await;
        grant(&context, user_id, "customers", "read").await;
        grant(&context, user_id, "customers", "write").await;

        for body in [
            json!({"name": "", "store_id": store_id}),
            json!({"name": "Alice", "store_id": store_id, "status": "deleted"}),
            json!({"name": "Alice", "store_id": store_id, "attachments": [{"file_id": "file-1", "mime_type": "application/pdf"}]}),
            json!({"name": "Alice", "store_id": store_id, "department_id": Uuid::new_v4()}),
        ] {
            let response = context
                .app
                .clone()
                .oneshot(request(
                    Method::POST,
                    "/api/v1/customers/create",
                    Some(&cookie),
                    Some(body),
                ))
                .await
                .expect("customer create request should be handled");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        let mismatched_system = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/customers/create",
                Some(&cookie),
                Some(json!({
                    "name": "Alice",
                    "system_id": Uuid::new_v4(),
                    "store_id": store_id
                })),
            ))
            .await
            .expect("customer create request should be handled");
        assert_eq!(mismatched_system.status(), StatusCode::BAD_REQUEST);

        let missing_store = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/customers/create",
                Some(&cookie),
                Some(json!({
                    "name": "Alice",
                    "store_id": Uuid::new_v4()
                })),
            ))
            .await
            .expect("customer create request should be handled");
        assert_eq!(missing_store.status(), StatusCode::NOT_FOUND);

        let invalid_status_query = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/customers/list?status_filter=deleted",
                Some(&cookie),
                None,
            ))
            .await
            .expect("customers list request should be handled");
        assert_eq!(invalid_status_query.status(), StatusCode::BAD_REQUEST);

        let invalid_store_query = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/customers/list?store_id=not-a-uuid",
                Some(&cookie),
                None,
            ))
            .await
            .expect("customers list request should be handled");
        assert_eq!(invalid_store_query.status(), StatusCode::BAD_REQUEST);

        let invalid_id = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/customers/update/not-a-uuid",
                Some(&cookie),
                Some(json!({"name": "Alice"})),
            ))
            .await
            .expect("customer update request should be handled");
        assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);

        let missing_delete = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/customers/delete/{}", Uuid::new_v4()),
                Some(&cookie),
                None,
            ))
            .await
            .expect("customer delete request should be handled");
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
                crate::repositories::systems::NewSystem {
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
                crate::repositories::stores::NewStore {
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

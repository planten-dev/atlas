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
        stores::{CreateStoreRequest, ListStoresQuery, UpdateStoreRequest},
    },
    repositories::RepositoryError,
    services::stores::StoreError,
    state::AppState,
};

pub async fn list_stores(
    State(state): State<AppState>,
    query: Result<Query<ListStoresQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.stores.list_stores(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => store_error_response(error),
    }
}

pub async fn store_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let store_id = match path {
        Ok(Path(store_id)) => store_id,
        Err(error) => return validation_error_response("invalid store_id path parameter", error),
    };

    match state.stores.store_detail(store_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => store_error_response(error),
    }
}

pub async fn create_store(
    State(state): State<AppState>,
    request: Result<Json<CreateStoreRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state.stores.create_store(request).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(error) => store_error_response(error),
    }
}

pub async fn update_store(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateStoreRequest>, JsonRejection>,
) -> Response {
    let store_id = match path {
        Ok(Path(store_id)) => store_id,
        Err(error) => return validation_error_response("invalid store_id path parameter", error),
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state.stores.update_store(store_id, request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => store_error_response(error),
    }
}

pub async fn disable_store(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let store_id = match path {
        Ok(Path(store_id)) => store_id,
        Err(error) => return validation_error_response("invalid store_id path parameter", error),
    };

    match state.stores.disable_store(store_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => store_error_response(error),
    }
}

pub async fn delete_store(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let store_id = match path {
        Ok(Path(store_id)) => store_id,
        Err(error) => return validation_error_response("invalid store_id path parameter", error),
    };

    match state.stores.delete_store(store_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => store_error_response(error),
    }
}

fn store_error_response(error: StoreError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "store request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "store request rejected"
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
    warn!(error, detail = %detail, "store request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &StoreError) -> StatusCode {
    match error {
        StoreError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        StoreError::StoreNotFound | StoreError::SystemNotFound => StatusCode::NOT_FOUND,
        StoreError::MissingRequiredField { .. }
        | StoreError::FieldTooLong { .. }
        | StoreError::InvalidStatus { .. }
        | StoreError::InvalidPaginationMinimum { .. }
        | StoreError::InvalidPaginationMaximum { .. } => StatusCode::BAD_REQUEST,
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
            authz::AuthzRepository, customers::CustomerRepository, events::EventRepository,
            product_categories::ProductCategoryRepository, products::ProductRepository,
            sales_records::SalesRecordRepository, sessions::SessionRepository,
            stores::StoreRepository, systems::SystemRepository,
            user_profiles::UserProfileRepository, users::UserRepository,
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
    }

    #[tokio::test]
    async fn stores_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .clone()
            .oneshot(request(Method::GET, "/api/v1/stores/list", None, None))
            .await
            .expect("stores list request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/stores/delete/{}", Uuid::new_v4()),
                None,
                None,
            ))
            .await
            .expect("store delete request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_without_store_permission_is_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/stores/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("stores list request should be handled");

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
        let system_id = create_system(&context, "system-a").await;
        grant(&context, user_id, "stores", "read").await;

        let read_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/stores/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("stores list request should be handled");
        assert_eq!(read_response.status(), StatusCode::OK);

        let write_response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/stores/create",
                Some(&cookie),
                Some(json!({"name": "store-a", "system_id": system_id})),
            ))
            .await
            .expect("store create request should be handled");
        assert_eq!(write_response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn store_crud_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let system_a = create_system(&context, "system-a").await;
        let system_b = create_system(&context, "system-b").await;
        grant(&context, user_id, "stores", "read").await;
        grant(&context, user_id, "stores", "write").await;

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/stores/create",
                Some(&cookie),
                Some(json!({
                    "name": "store-a",
                    "system_id": system_a
                })),
            ))
            .await
            .expect("store create request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = response_json(create_response).await;
        assert_eq!(
            created.pointer("/name").and_then(Value::as_str),
            Some("store-a")
        );
        assert_eq!(
            created.pointer("/system_id").and_then(Value::as_str),
            Some(system_a.to_string().as_str())
        );
        assert_eq!(
            created.pointer("/status").and_then(Value::as_str),
            Some("active")
        );
        let store_id = created
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("store id should be present")
            .to_string();

        let list_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!(
                    "/api/v1/stores/list?status_filter=active&system_id={system_a}&page_number=1&page_size=20"
                ),
                Some(&cookie),
                None,
            ))
            .await
            .expect("stores list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list = response_json(list_response).await;
        assert_eq!(
            list.pointer("/total_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            list.pointer("/stores/0/id").and_then(Value::as_str),
            Some(store_id.as_str())
        );

        let update_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/stores/update/{store_id}"),
                Some(&cookie),
                Some(json!({
                    "name": "store-b",
                    "system_id": system_b
                })),
            ))
            .await
            .expect("store update request should be handled");
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated = response_json(update_response).await;
        assert_eq!(
            updated.pointer("/name").and_then(Value::as_str),
            Some("store-b")
        );
        assert_eq!(
            updated.pointer("/system_id").and_then(Value::as_str),
            Some(system_b.to_string().as_str())
        );

        let disable_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/stores/disable/{store_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("store disable request should be handled");
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
                &format!("/api/v1/stores/delete/{store_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("store delete request should be handled");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let missing_detail = context
            .app
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/stores/detail/{store_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("store detail request should be handled");
        assert_eq!(missing_detail.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn store_api_validates_inputs_and_missing_resources() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        let system_id = create_system(&context, "system-a").await;
        grant(&context, user_id, "stores", "read").await;
        grant(&context, user_id, "stores", "write").await;

        for body in [
            json!({"name": "", "system_id": system_id}),
            json!({"name": "store-a", "system_id": system_id, "status": "deleted"}),
        ] {
            let response = context
                .app
                .clone()
                .oneshot(request(
                    Method::POST,
                    "/api/v1/stores/create",
                    Some(&cookie),
                    Some(body),
                ))
                .await
                .expect("store create request should be handled");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        let missing_system = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/stores/create",
                Some(&cookie),
                Some(json!({"name": "store-a", "system_id": Uuid::new_v4()})),
            ))
            .await
            .expect("store create request should be handled");
        assert_eq!(missing_system.status(), StatusCode::NOT_FOUND);

        let invalid_status_query = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/stores/list?status_filter=deleted",
                Some(&cookie),
                None,
            ))
            .await
            .expect("stores list request should be handled");
        assert_eq!(invalid_status_query.status(), StatusCode::BAD_REQUEST);

        let invalid_system_query = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/stores/list?system_id=not-a-uuid",
                Some(&cookie),
                None,
            ))
            .await
            .expect("stores list request should be handled");
        assert_eq!(invalid_system_query.status(), StatusCode::BAD_REQUEST);

        let invalid_id = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/stores/update/not-a-uuid",
                Some(&cookie),
                Some(json!({"name": "store-a"})),
            ))
            .await
            .expect("store update request should be handled");
        assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);

        let missing_delete = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/stores/delete/{}", Uuid::new_v4()),
                Some(&cookie),
                None,
            ))
            .await
            .expect("store delete request should be handled");
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
            systems,
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

    async fn create_system(context: &TestContext, name: &str) -> Uuid {
        context
            .systems
            .create_system(
                crate::repositories::systems::NewSystem {
                    name: name.to_string(),
                    status: "active".to_string(),
                },
                chrono::Utc::now(),
            )
            .await
            .expect("system should be created")
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

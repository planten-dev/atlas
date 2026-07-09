use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{PathRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    dto::{auth::ErrorResponse, departments::ListDepartmentsQuery},
    integrations::dingtalk::DingTalkError,
    repositories::RepositoryError,
    services::departments::DepartmentError,
    state::AppState,
};

pub async fn list_departments(
    State(state): State<AppState>,
    query: Result<Query<ListDepartmentsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.departments.list_departments(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => department_error_response(error),
    }
}

pub async fn department_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let department_id = match path {
        Ok(Path(department_id)) => department_id,
        Err(error) => {
            return validation_error_response("invalid department_id path parameter", error);
        }
    };

    match state.departments.department_detail(department_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => department_error_response(error),
    }
}

pub async fn sync_dingtalk_departments(State(state): State<AppState>) -> Response {
    match state.departments.sync_from_dingtalk().await {
        Ok(summary) => (
            StatusCode::OK,
            Json(crate::dto::departments::DepartmentSyncResponse::from(
                summary,
            )),
        )
            .into_response(),
        Err(error) => department_error_response(error),
    }
}

fn department_error_response(error: DepartmentError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "department request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "department request rejected"
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
    warn!(error, detail = %detail, "department request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &DepartmentError) -> StatusCode {
    match error {
        DepartmentError::DingTalk(error) => match error {
            DingTalkError::MissingConfig(_) => StatusCode::INTERNAL_SERVER_ERROR,
            DingTalkError::ProviderHttp { .. }
            | DingTalkError::ProviderApi { .. }
            | DingTalkError::MissingResponseField { .. }
            | DingTalkError::Http(_) => StatusCode::BAD_GATEWAY,
            DingTalkError::MissingRequiredField { .. } | DingTalkError::MissingIdentityField(_) => {
                StatusCode::BAD_REQUEST
            }
        },
        DepartmentError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        DepartmentError::DepartmentNotFound => StatusCode::NOT_FOUND,
        DepartmentError::InvalidStatus { .. }
        | DepartmentError::InvalidSource { .. }
        | DepartmentError::InvalidPaginationMinimum { .. }
        | DepartmentError::InvalidPaginationMaximum { .. } => StatusCode::BAD_REQUEST,
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
        Form, Router,
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
        departments: DepartmentRepository,
    }

    #[tokio::test]
    async fn departments_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        for (method, uri) in [
            (Method::GET, "/api/v1/departments/list".to_string()),
            (
                Method::GET,
                format!("/api/v1/departments/detail/{}", Uuid::new_v4()),
            ),
            (
                Method::POST,
                "/api/v1/departments/sync/dingtalk".to_string(),
            ),
        ] {
            let response = context
                .app
                .clone()
                .oneshot(request(method, &uri, None, None))
                .await
                .expect("department request should be handled");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn session_without_department_permission_is_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/departments/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("departments list request should be handled");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_denied")
        );

        // Sync mutates local rows, so read permission alone must not allow it.
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "departments", "read").await;
        let response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/departments/sync/dingtalk",
                Some(&cookie),
                None,
            ))
            .await
            .expect("department sync request should be handled");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn lists_and_reads_departments_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "departments", "read").await;

        let now = chrono::Utc::now();
        let parent = context
            .departments
            .insert_department(Uuid::new_v4(), "dingtalk", "10", "总裁办", None, now)
            .await
            .expect("parent department should be created");
        let child = context
            .departments
            .insert_department(
                Uuid::new_v4(),
                "dingtalk",
                "11",
                "秘书处",
                Some(parent.id),
                now,
            )
            .await
            .expect("child department should be created");

        let list_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!(
                    "/api/v1/departments/list?status_filter=active&source_filter=dingtalk&parent_id={}&page_number=1&page_size=20",
                    parent.id
                ),
                Some(&cookie),
                None,
            ))
            .await
            .expect("departments list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list = response_json(list_response).await;
        assert_eq!(
            list.pointer("/total_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            list.pointer("/departments/0/id").and_then(Value::as_str),
            Some(child.id.to_string().as_str())
        );
        assert_eq!(
            list.pointer("/departments/0/parent_id")
                .and_then(Value::as_str),
            Some(parent.id.to_string().as_str())
        );

        let detail_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/departments/detail/{}", parent.id),
                Some(&cookie),
                None,
            ))
            .await
            .expect("department detail request should be handled");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail = response_json(detail_response).await;
        assert_eq!(
            detail.pointer("/name").and_then(Value::as_str),
            Some("总裁办")
        );
        assert_eq!(
            detail.pointer("/source").and_then(Value::as_str),
            Some("dingtalk")
        );

        let missing_detail = context
            .app
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/departments/detail/{}", Uuid::new_v4()),
                Some(&cookie),
                None,
            ))
            .await
            .expect("department detail request should be handled");
        assert_eq!(missing_detail.status(), StatusCode::NOT_FOUND);
        let body = response_json(missing_detail).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("department_not_found")
        );
    }

    #[tokio::test]
    async fn rejects_invalid_department_query_parameters() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "departments", "read").await;

        for uri in [
            "/api/v1/departments/list?status_filter=deleted",
            "/api/v1/departments/list?source_filter=wechat",
            "/api/v1/departments/list?parent_id=not-a-uuid",
            "/api/v1/departments/list?page_number=0",
            "/api/v1/departments/list?page_size=201",
        ] {
            let response = context
                .app
                .clone()
                .oneshot(request(Method::GET, uri, Some(&cookie), None))
                .await
                .expect("departments list request should be handled");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "uri: {uri}");
        }

        let invalid_id = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/departments/detail/not-a-uuid",
                Some(&cookie),
                None,
            ))
            .await
            .expect("department detail request should be handled");
        assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn syncs_departments_from_dingtalk_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "departments", "write").await;

        let sync_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/departments/sync/dingtalk",
                Some(&cookie),
                None,
            ))
            .await
            .expect("department sync request should be handled");
        assert_eq!(sync_response.status(), StatusCode::OK);
        let summary = response_json(sync_response).await;
        assert_eq!(summary.pointer("/total").and_then(Value::as_u64), Some(3));
        assert_eq!(summary.pointer("/created").and_then(Value::as_u64), Some(3));
        assert_eq!(summary.pointer("/updated").and_then(Value::as_u64), Some(0));
        assert_eq!(
            summary.pointer("/unchanged").and_then(Value::as_u64),
            Some(0)
        );

        let synced = context
            .departments
            .list_by_source("dingtalk")
            .await
            .expect("synced departments should list");
        assert_eq!(synced.len(), 3);

        // A second sync sees the same tree and changes nothing.
        let second_response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/departments/sync/dingtalk",
                Some(&cookie),
                None,
            ))
            .await
            .expect("department sync request should be handled");
        assert_eq!(second_response.status(), StatusCode::OK);
        let second = response_json(second_response).await;
        assert_eq!(second.pointer("/created").and_then(Value::as_u64), Some(0));
        assert_eq!(
            second.pointer("/unchanged").and_then(Value::as_u64),
            Some(3)
        );
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
        let authz = AuthzService::new(AuthzRepository::new(db.clone()))
            .await
            .expect("test authz service should initialize");
        let users_service = UserService::new(users.clone(), profiles, sessions);
        let product_categories_service =
            ProductCategoryService::new(product_categories.clone(), products.clone());
        let products_service = ProductService::new(products, product_categories.clone());
        let stores_service = StoreService::new(stores.clone(), systems.clone());
        let systems_service = SystemService::new(systems.clone(), stores.clone());
        let departments_service = DepartmentService::new(dingtalk_config, departments.clone());
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
        let state = crate::state::AppState::new(crate::state::AppStateParts {
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
            departments,
        }
    }

    #[derive(serde::Deserialize)]
    struct ListSubForm {
        dept_id: i64,
        language: String,
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

        async fn gettoken() -> Json<Value> {
            Json(json!({
                "errcode": 0,
                "errmsg": "ok",
                "access_token": "corp-token",
                "expires_in": 7200
            }))
        }

        // Small fixed org tree: root -> (10 总裁办, 20 研发), 20 -> 21 后端.
        async fn listsub(Form(form): Form<ListSubForm>) -> Json<Value> {
            assert_eq!(form.language, "zh_CN");
            let result = match form.dept_id {
                1 => json!([
                    {"dept_id": 10, "name": "总裁办", "parent_id": 1},
                    {"dept_id": 20, "name": "研发", "parent_id": 1},
                ]),
                20 => json!([{"dept_id": 21, "name": "后端", "parent_id": 20}]),
                _ => json!([]),
            };
            Json(json!({
                "errcode": 0,
                "errmsg": "ok",
                "result": result
            }))
        }

        let app = Router::new()
            .route("/token", post(token))
            .route("/me", get(me))
            .route("/gettoken", get(gettoken))
            .route("/listsub", post(listsub));
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

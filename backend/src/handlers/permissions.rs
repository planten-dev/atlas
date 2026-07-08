use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::{
    dto::permissions::{
        CatalogEntryResponse, CreatePolicyRequest, CreateRoleRequest, ListPoliciesQuery,
        MyPermissionsResponse, PolicyResponse, ReplaceSubjectPoliciesRequest, RoleDetailResponse,
        RoleResponse, SetRoleParentsRequest, SetUserRolesRequest, SubjectPoliciesResponse,
        UpdateRoleRequest, UserRolesResponse,
    },
    handlers::error::authz_error_response,
    repositories::authz::NewPolicy,
    services::auth::CurrentSession,
    state::AppState,
};

/// Effective permissions of the calling user, for frontend menu and
/// button gating. Requires only an authenticated session: every user may
/// read their own permissions.
pub async fn my_permissions(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
) -> Response {
    match state
        .authz
        .effective_permissions(current_session.user.id)
        .await
    {
        Ok(permissions) => {
            (StatusCode::OK, Json(MyPermissionsResponse { permissions })).into_response()
        }
        Err(error) => authz_error_response(error),
    }
}

/// Every permission the backend enforces, grouped for the permission
/// panel's selection matrix.
pub async fn permission_catalog(State(state): State<AppState>) -> Response {
    let entries: Vec<CatalogEntryResponse> = state
        .authz
        .catalog()
        .entries()
        .iter()
        .map(|entry| CatalogEntryResponse {
            object: entry.object().to_string(),
            actions: entry.actions().to_vec(),
            group: entry.group().to_string(),
            label: entry.label().to_string(),
        })
        .collect();
    (StatusCode::OK, Json(entries)).into_response()
}

/// Replaces the full policy set of one subject in a single transaction
/// with one enforcer reload — the bulk save behind the permission panel.
pub async fn replace_subject_policies(
    State(state): State<AppState>,
    Path((subject_kind, subject_id)): Path<(String, Uuid)>,
    Json(request): Json<ReplaceSubjectPoliciesRequest>,
) -> Response {
    let policies: Vec<NewPolicy> = request
        .policies
        .into_iter()
        .map(|policy| NewPolicy {
            object: policy.object,
            action: policy.action,
            effect: policy.effect,
        })
        .collect();

    match state
        .authz
        .replace_subject_policies(subject_kind.clone(), subject_id, policies)
        .await
    {
        Ok(created) => (
            StatusCode::OK,
            Json(SubjectPoliciesResponse {
                subject_kind,
                subject_id,
                policies: created
                    .into_iter()
                    .map(PolicyResponse::from_model)
                    .collect(),
            }),
        )
            .into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn list_roles(State(state): State<AppState>) -> Response {
    match state.authz.list_roles().await {
        Ok(roles) => {
            let roles: Vec<RoleResponse> =
                roles.into_iter().map(RoleResponse::from_model).collect();
            (StatusCode::OK, Json(roles)).into_response()
        }
        Err(error) => authz_error_response(error),
    }
}

pub async fn create_role(
    State(state): State<AppState>,
    Json(request): Json<CreateRoleRequest>,
) -> Response {
    match state
        .authz
        .create_role(request.code, request.name, request.kind, request.priority)
        .await
    {
        Ok(role) => (StatusCode::CREATED, Json(RoleResponse::from_model(role))).into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn get_role(State(state): State<AppState>, Path(role_id): Path<Uuid>) -> Response {
    match state.authz.get_role(role_id).await {
        Ok((role, parent_role_ids)) => (
            StatusCode::OK,
            Json(RoleDetailResponse {
                role: RoleResponse::from_model(role),
                parent_role_ids,
            }),
        )
            .into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
    Json(request): Json<UpdateRoleRequest>,
) -> Response {
    match state
        .authz
        .update_role(role_id, request.name, request.priority)
        .await
    {
        Ok(role) => (StatusCode::OK, Json(RoleResponse::from_model(role))).into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn delete_role(State(state): State<AppState>, Path(role_id): Path<Uuid>) -> Response {
    match state.authz.delete_role(role_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn set_role_parents(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
    Json(request): Json<SetRoleParentsRequest>,
) -> Response {
    match state
        .authz
        .set_role_parents(role_id, request.parent_role_ids)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn list_user_roles(State(state): State<AppState>, Path(user_id): Path<Uuid>) -> Response {
    match state.authz.list_user_roles(user_id).await {
        Ok(roles) => (
            StatusCode::OK,
            Json(UserRolesResponse {
                user_id,
                roles: roles.into_iter().map(RoleResponse::from_model).collect(),
            }),
        )
            .into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn set_user_roles(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(request): Json<SetUserRolesRequest>,
) -> Response {
    match state.authz.set_user_roles(user_id, request.role_ids).await {
        Ok(roles) => (
            StatusCode::OK,
            Json(UserRolesResponse {
                user_id,
                roles: roles.into_iter().map(RoleResponse::from_model).collect(),
            }),
        )
            .into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn list_policies(
    State(state): State<AppState>,
    Query(query): Query<ListPoliciesQuery>,
) -> Response {
    match state
        .authz
        .list_policies(query.subject_kind, query.subject_id)
        .await
    {
        Ok(policies) => {
            let policies: Vec<PolicyResponse> = policies
                .into_iter()
                .map(PolicyResponse::from_model)
                .collect();
            (StatusCode::OK, Json(policies)).into_response()
        }
        Err(error) => authz_error_response(error),
    }
}

pub async fn create_policy(
    State(state): State<AppState>,
    Json(request): Json<CreatePolicyRequest>,
) -> Response {
    match state
        .authz
        .create_policy(
            request.subject_kind,
            request.subject_id,
            request.object,
            request.action,
            request.effect,
        )
        .await
    {
        Ok(policy) => (
            StatusCode::CREATED,
            Json(PolicyResponse::from_model(policy)),
        )
            .into_response(),
        Err(error) => authz_error_response(error),
    }
}

pub async fn delete_policy(State(state): State<AppState>, Path(policy_id): Path<Uuid>) -> Response {
    match state.authz.delete_policy(policy_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => authz_error_response(error),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        app,
        config::{AuthConfig, DatabaseConfig, DatabaseKind, DingTalkConfig, SessionConfig},
        db,
        repositories::{
            authz::AuthzRepository, departments::DepartmentRepository, events::EventRepository,
            product_categories::ProductCategoryRepository, products::ProductRepository,
            sessions::SessionRepository, stores::StoreRepository, systems::SystemRepository,
            users::UserRepository,
        },
        services::{
            auth::AuthService, authz::AuthzService, events::EventService,
            product_categories::ProductCategoryService, products::ProductService,
            review::ApplierRegistry, stores::StoreService, systems::SystemService,
            users::UserService,
        },
        state::AppState,
    };
    use axum::{
        Json, Router,
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
        routing::{get, post},
    };
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tokio::net::TcpListener;
    use tower::ServiceExt;
    use uuid::Uuid;

    struct TestContext {
        app: Router,
        users: UserRepository,
        authz: AuthzService,
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
        let sessions = SessionRepository::new(db.clone());
        let product_categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db.clone());
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db.clone());
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
                scope: "openid".to_string(),
                corp_id: "".to_string(),
                external_id_fields: vec!["userId".to_string()],
            },
            users.clone(),
            sessions.clone(),
            86_400,
        );
        let authz = AuthzService::new(AuthzRepository::new(db.clone()))
            .await
            .expect("test authz service should initialize");
        let users_service = UserService::new(users.clone(), sessions);
        let product_categories_service =
            ProductCategoryService::new(product_categories.clone(), products.clone());
        let products_service = ProductService::new(products, product_categories);
        let stores_service = StoreService::new(stores.clone(), systems.clone());
        let systems_service = SystemService::new(systems, departments, stores);
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

    /// Grants the caller a user-level permission directly through the
    /// service — the "dev seeds the database manually" bootstrap path.
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

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        serde_json::from_slice(&bytes).expect("body should be json")
    }

    #[tokio::test]
    async fn my_permissions_requires_only_a_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        // Unauthenticated: 401.
        let anonymous = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/auth/me/permissions",
                None,
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

        // Authenticated with no policies: 200 with an empty list.
        let cookie = login_and_cookie(context.app.clone()).await;
        let empty = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/auth/me/permissions",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(empty.status(), StatusCode::OK);
        let body = json_body(empty).await;
        assert_eq!(
            body.pointer("/permissions")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );

        // Grants become visible without any permission-management rights.
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "products", "read").await;
        grant(&context, user_id, "products:categories", "write").await;

        let granted = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/auth/me/permissions",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(granted.status(), StatusCode::OK);
        let body = json_body(granted).await;
        let permissions: Vec<&str> = body
            .pointer("/permissions")
            .and_then(Value::as_array)
            .expect("permissions should be an array")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(permissions.contains(&"products:read"));
        assert!(permissions.contains(&"products:categories:write"));
        assert!(!permissions.contains(&"products:write"));
    }

    #[tokio::test]
    async fn catalog_lists_grouped_permissions_behind_read_permission() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let forbidden = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/catalog",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "system:permissions", "read").await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/catalog",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        let entries = body.as_array().expect("catalog should be an array");
        assert!(!entries.is_empty());

        let products = entries
            .iter()
            .find(|entry| entry.pointer("/object").and_then(Value::as_str) == Some("products"))
            .expect("products entry should exist");
        let actions: Vec<&str> = products
            .pointer("/actions")
            .and_then(Value::as_array)
            .expect("actions should be an array")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(actions.contains(&"read"));
        assert!(actions.contains(&"write"));
        assert!(
            products.pointer("/group").and_then(Value::as_str).is_some(),
            "entries should carry group metadata"
        );
        assert!(
            products.pointer("/label").and_then(Value::as_str).is_some(),
            "entries should carry label metadata"
        );
    }

    #[tokio::test]
    async fn replace_subject_policies_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "system:permissions", "write").await;

        let role_id = context
            .authz
            .create_role("editors".into(), "编辑".into(), "custom".into(), None)
            .await
            .expect("role should be created")
            .id;

        let replace = context
            .app
            .clone()
            .oneshot(request(
                Method::PUT,
                &format!("/api/v1/permissions/subjects/role/{role_id}/policies"),
                Some(&cookie),
                Some(json!({
                    "policies": [
                        {"object": "products", "action": "read", "effect": "allow"},
                        {"object": "products", "action": "write", "effect": "allow"},
                        {"object": "stores", "action": "read", "effect": "deny"}
                    ]
                })),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(replace.status(), StatusCode::OK);
        let body = json_body(replace).await;
        assert_eq!(
            body.pointer("/subject_kind").and_then(Value::as_str),
            Some("role")
        );
        assert_eq!(
            body.pointer("/policies")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );

        // A second replace fully swaps the set instead of appending.
        let swap = context
            .app
            .clone()
            .oneshot(request(
                Method::PUT,
                &format!("/api/v1/permissions/subjects/role/{role_id}/policies"),
                Some(&cookie),
                Some(json!({
                    "policies": [
                        {"object": "systems", "action": "read", "effect": "allow"}
                    ]
                })),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(swap.status(), StatusCode::OK);
        let body = json_body(swap).await;
        let policies = body
            .pointer("/policies")
            .and_then(Value::as_array)
            .expect("policies should be an array");
        assert_eq!(policies.len(), 1);
        assert_eq!(
            policies[0].pointer("/object").and_then(Value::as_str),
            Some("systems")
        );

        // Validation failures surface as 422 and leave the set untouched.
        let invalid = context
            .app
            .clone()
            .oneshot(request(
                Method::PUT,
                &format!("/api/v1/permissions/subjects/role/{role_id}/policies"),
                Some(&cookie),
                Some(json!({
                    "policies": [
                        {"object": "not:a:real:object", "action": "read", "effect": "allow"}
                    ]
                })),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = json_body(invalid).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_not_in_catalog")
        );

        // Unknown subject kinds and missing subjects are rejected.
        let bad_kind = context
            .app
            .clone()
            .oneshot(request(
                Method::PUT,
                &format!("/api/v1/permissions/subjects/group/{role_id}/policies"),
                Some(&cookie),
                Some(json!({"policies": []})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(bad_kind.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let missing_role = context
            .app
            .oneshot(request(
                Method::PUT,
                &format!(
                    "/api/v1/permissions/subjects/role/{}/policies",
                    Uuid::new_v4()
                ),
                Some(&cookie),
                Some(json!({"policies": []})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(missing_role.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn permission_routes_require_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/roles",
                None,
                None,
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_without_permission_is_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/roles",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = json_body(response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_denied")
        );
    }

    #[tokio::test]
    async fn read_permission_allows_get_but_not_post() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "system:permissions", "read").await;

        let get_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/roles",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(get_response.status(), StatusCode::OK);

        let post_response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/roles",
                Some(&cookie),
                Some(json!({"code": "x", "name": "x", "kind": "custom"})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(post_response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn role_crud_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "system:permissions", "read").await;
        grant(&context, user_id, "system:permissions", "write").await;

        let create_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/roles",
                Some(&cookie),
                Some(json!({"code": "finance-dept", "name": "财务部", "kind": "department"})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created = json_body(create_response).await;
        assert_eq!(
            created.pointer("/priority").and_then(Value::as_i64),
            Some(30)
        );
        let role_id = created
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("role id should be present")
            .to_string();

        let duplicate_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/roles",
                Some(&cookie),
                Some(json!({"code": "finance-dept", "name": "dup", "kind": "department"})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(duplicate_response.status(), StatusCode::CONFLICT);

        let invalid_kind_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/roles",
                Some(&cookie),
                Some(json!({"code": "y", "name": "y", "kind": "team"})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(
            invalid_kind_response.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );

        let patch_response = context
            .app
            .clone()
            .oneshot(request(
                Method::PATCH,
                &format!("/api/v1/permissions/roles/{role_id}"),
                Some(&cookie),
                Some(json!({"name": "财务一部", "priority": 25})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(patch_response.status(), StatusCode::OK);
        let patched = json_body(patch_response).await;
        assert_eq!(
            patched.pointer("/name").and_then(Value::as_str),
            Some("财务一部")
        );
        assert_eq!(
            patched.pointer("/priority").and_then(Value::as_i64),
            Some(25)
        );

        let get_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/permissions/roles/{role_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(get_response.status(), StatusCode::OK);

        let delete_response = context
            .app
            .clone()
            .oneshot(request(
                Method::DELETE,
                &format!("/api/v1/permissions/roles/{role_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let get_missing_response = context
            .app
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/permissions/roles/{role_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(get_missing_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn inheritance_cycle_is_rejected_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "system:permissions", "write").await;

        let role_a = context
            .authz
            .create_role("dept-a".into(), "A".into(), "department".into(), None)
            .await
            .expect("role a should be created")
            .id;
        let role_b = context
            .authz
            .create_role("dept-b".into(), "B".into(), "department".into(), None)
            .await
            .expect("role b should be created")
            .id;
        context
            .authz
            .set_role_parents(role_a, vec![role_b])
            .await
            .expect("a -> b should be allowed");

        let cycle_response = context
            .app
            .oneshot(request(
                Method::PUT,
                &format!("/api/v1/permissions/roles/{role_b}/parents"),
                Some(&cookie),
                Some(json!({"parent_role_ids": [role_a]})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(cycle_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = json_body(cycle_response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("inheritance_cycle")
        );
    }

    #[tokio::test]
    async fn policy_and_role_assignment_changes_are_enforced_on_next_request() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user_id = logged_in_user_id(&context).await;
        grant(&context, user_id, "system:permissions", "write").await;

        // No read permission yet: listing policies is forbidden.
        let forbidden = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/policies",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        // Grant read through the HTTP API itself (role-based, assigned to
        // the caller), then the same request must succeed.
        let role_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/roles",
                Some(&cookie),
                Some(json!({"code": "perm-admins", "name": "权限管理员", "kind": "custom"})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(role_response.status(), StatusCode::CREATED);
        let role_id = json_body(role_response)
            .await
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("role id should be present")
            .to_string();

        let policy_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/policies",
                Some(&cookie),
                Some(json!({
                    "subject_kind": "role",
                    "subject_id": role_id,
                    "object": "system:permissions",
                    "action": "read",
                    "effect": "allow"
                })),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(policy_response.status(), StatusCode::CREATED);
        let policy_id = json_body(policy_response)
            .await
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("policy id should be present")
            .to_string();

        let assign_response = context
            .app
            .clone()
            .oneshot(request(
                Method::PUT,
                &format!("/api/v1/permissions/users/{user_id}/roles"),
                Some(&cookie),
                Some(json!({"role_ids": [role_id]})),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(assign_response.status(), StatusCode::OK);

        // The role-based read permission is now live: listing succeeds.
        let allowed = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/policies",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(allowed.status(), StatusCode::OK);

        // Duplicate policy creation conflicts.
        let duplicate_policy = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                "/api/v1/permissions/policies",
                Some(&cookie),
                Some(json!({
                    "subject_kind": "role",
                    "subject_id": role_id,
                    "object": "system:permissions",
                    "action": "read",
                    "effect": "allow"
                })),
            ))
            .await
            .expect("request should be handled");
        assert_eq!(duplicate_policy.status(), StatusCode::CONFLICT);

        // Deleting the policy revokes read again on the next request.
        let delete_policy = context
            .app
            .clone()
            .oneshot(request(
                Method::DELETE,
                &format!("/api/v1/permissions/policies/{policy_id}"),
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(delete_policy.status(), StatusCode::NO_CONTENT);

        let forbidden_again = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/permissions/policies",
                Some(&cookie),
                None,
            ))
            .await
            .expect("request should be handled");
        assert_eq!(forbidden_again.status(), StatusCode::FORBIDDEN);
    }
}

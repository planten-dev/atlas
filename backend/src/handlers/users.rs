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
        users::{ListUsersQuery, UpdateUserStatusRequest, UserStatus},
    },
    handlers::error::{auth_error_response, authz_error_response, permission_denied_response},
    repositories::RepositoryError,
    services::{auth::CurrentSession, users::UserError},
    state::AppState,
};

pub async fn list_users(
    State(state): State<AppState>,
    query: Result<Query<ListUsersQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.users.list_users(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => user_error_response(error),
    }
}

pub async fn user_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let user_id = match path {
        Ok(Path(user_id)) => user_id,
        Err(error) => return validation_error_response("invalid user_id path parameter", error),
    };

    match state.users.user_detail(user_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => user_error_response(error),
    }
}

pub async fn user_profile(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let user_id = match path {
        Ok(Path(user_id)) => user_id,
        Err(error) => return validation_error_response("invalid user_id path parameter", error),
    };

    if user_id != current_session.user.id {
        match state
            .authz
            .check(current_session.user.id, "users", "read")
            .await
        {
            Ok(true) => {}
            Ok(false) => return permission_denied_response("users", "read"),
            Err(error) => return authz_error_response(error),
        }
    }

    match state.users.user_profile(user_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => user_error_response(error),
    }
}

pub async fn sync_user_profile(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let user_id = match path {
        Ok(Path(user_id)) => user_id,
        Err(error) => return validation_error_response("invalid user_id path parameter", error),
    };

    if user_id != current_session.user.id {
        match state
            .authz
            .check(current_session.user.id, "users", "write")
            .await
        {
            Ok(true) => {}
            Ok(false) => return permission_denied_response("users", "write"),
            Err(error) => return authz_error_response(error),
        }
    }

    match state
        .auth
        .sync_dingtalk_profile_for_user(current_session.user.id, user_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => auth_error_response(error),
    }
}

pub async fn update_user_status(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<UpdateUserStatusRequest>, JsonRejection>,
) -> Response {
    let user_id = match path {
        Ok(Path(user_id)) => user_id,
        Err(error) => return validation_error_response("invalid user_id path parameter", error),
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };
    if let Err(error) = UserStatus::parse("target_status", &request.target_status) {
        return user_error_response(UserError::InvalidStatus {
            field: error.field,
            value: error.value,
        });
    }
    let old_user = match state.users.user_detail(user_id).await {
        Ok(value) => serde_json::json!({"id": value.id, "status": value.status}),
        Err(error) => return user_error_response(error),
    };

    match state
        .users
        .update_status(current_session.user.id, user_id, request)
        .await
    {
        Ok(response) => {
            let new_user = serde_json::json!({"id": response.id, "status": response.status});
            match state
                .events
                .record_update(
                    "users",
                    current_session.user.id,
                    user_id,
                    &old_user,
                    &new_user,
                )
                .await
            {
                Ok(_) => (StatusCode::OK, Json(response)).into_response(),
                Err(error) => crate::handlers::events::event_error_response(error),
            }
        }
        Err(error) => user_error_response(error),
    }
}

pub async fn delete_user(
    State(state): State<AppState>,
    Extension(session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let user_id = match path {
        Ok(Path(user_id)) => user_id,
        Err(error) => return validation_error_response("invalid user_id path parameter", error),
    };

    let old_user = match state.users.user_detail(user_id).await {
        Ok(v) => serde_json::json!({"id": v.id, "status": v.status}),
        Err(e) => return user_error_response(e),
    };
    match state.users.delete_user(user_id).await {
        Ok(()) => match state
            .events
            .record_delete("users", session.user.id, user_id, &old_user)
            .await
        {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => crate::handlers::events::event_error_response(error),
        },
        Err(error) => user_error_response(error),
    }
}

fn user_error_response(error: UserError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "user request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "user request rejected"
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
    warn!(error, detail = %detail, "user request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &UserError) -> StatusCode {
    match error {
        UserError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        UserError::UserNotFound | UserError::UserProfileNotFound => StatusCode::NOT_FOUND,
        UserError::CannotDisableSelf => StatusCode::CONFLICT,
        UserError::InvalidStatus { .. }
        | UserError::InvalidPaginationMinimum { .. }
        | UserError::InvalidPaginationMaximum { .. } => StatusCode::BAD_REQUEST,
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
            customers::CustomerRepository,
            departments::DepartmentRepository,
            events::{EventFilter, EventRepository},
            product_categories::ProductCategoryRepository,
            products::ProductRepository,
            sales_records::SalesRecordRepository,
            sessions::{SessionRepository, hash_secret},
            stores::StoreRepository,
            systems::SystemRepository,
            user_profiles::{UserProfileRepository, UserProfileUpsert},
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
    use chrono::{Duration, Utc};
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    struct TestContext {
        app: Router,
        users: UserRepository,
        profiles: UserProfileRepository,
        events: EventRepository,
        sessions: SessionRepository,
        authz: AuthzService,
    }

    #[tokio::test]
    async fn users_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/users/list")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("users list request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/{}/profile", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("user profile request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/{}/profile/sync", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("user profile sync request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/delete/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("user delete request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn authenticated_user_can_list_and_read_user_detail() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        grant(&context, current.id, "users", "read").await;

        let list_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/users/list?page_number=1&page_size=20")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("users list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let body = response_json(list_response).await;
        let user_id = body
            .pointer("/users/0/id")
            .and_then(Value::as_str)
            .expect("users list should include current user");
        assert_eq!(
            body.pointer("/page_number").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(body.pointer("/page_size").and_then(Value::as_u64), Some(20));
        assert_eq!(
            body.pointer("/total_count").and_then(Value::as_u64),
            Some(1)
        );

        let detail_response = context
            .app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/detail/{user_id}"))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("user detail request should be handled");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let body = response_json(detail_response).await;
        assert_eq!(body.pointer("/id").and_then(Value::as_str), Some(user_id));
        assert_eq!(
            body.pointer("/dingtalk_user_id").and_then(Value::as_str),
            Some("ding-user-1")
        );
    }

    #[tokio::test]
    async fn user_management_routes_require_permissions() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let target = context
            .users
            .find_or_create_for_login("target-user", Utc::now())
            .await
            .expect("target user should be created");

        let list_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/users/list")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("users list request should be handled");
        assert_eq!(list_response.status(), StatusCode::FORBIDDEN);
        let body = response_json(list_response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_denied")
        );

        let detail_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/detail/{}", target.id))
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("user detail request should be handled");
        assert_eq!(detail_response.status(), StatusCode::FORBIDDEN);

        let update_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/update-status/{}", target.id))
                    .header(header::COOKIE, cookie.clone())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"target_status":"disabled"}"#))
                    .unwrap(),
            )
            .await
            .expect("update status request should be handled");
        assert_eq!(update_response.status(), StatusCode::FORBIDDEN);

        let delete_response = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/delete/{}", target.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("delete user request should be handled");
        assert_eq!(delete_response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn authenticated_user_can_read_own_profile_without_users_read_permission() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");

        let response = context
            .app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/{}/profile", current.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/user_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            Some(current.id.to_string())
        );
        assert_eq!(body.pointer("/name").and_then(Value::as_str), Some("张三"));
        assert_eq!(
            body.pointer("/department_external_ids")
                .and_then(Value::as_str),
            Some("[10,20]")
        );
    }

    #[tokio::test]
    async fn authenticated_user_can_sync_own_profile_without_users_write_permission() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        context
            .profiles
            .upsert_profile(current.id, profile_input("旧资料"), Utc::now())
            .await
            .expect("stale profile should be stored");

        let response = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/{}/profile/sync", current.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile sync request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/user_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            Some(current.id.to_string())
        );
        assert_eq!(body.pointer("/name").and_then(Value::as_str), Some("张三"));
        let profile = context
            .profiles
            .find_by_user_id(current.id)
            .await
            .expect("profile lookup should succeed")
            .expect("profile should exist");
        assert_eq!(profile.name.as_deref(), Some("张三"));

        let (events, total_count) = context
            .events
            .list_events(
                EventFilter {
                    resource_type: Some("user_profiles".to_string()),
                    resource_id: Some(current.id),
                    ..EventFilter::default()
                },
                1,
                20,
            )
            .await
            .expect("profile sync events should be listed");
        assert!(total_count >= 1);
        let manual_event = events
            .iter()
            .find(|event| {
                event
                    .new_value
                    .as_ref()
                    .and_then(|value| value.pointer("/trigger"))
                    .and_then(Value::as_str)
                    == Some("manual")
            })
            .expect("manual profile sync event should be recorded");
        assert_eq!(manual_event.actor_user_id, Some(current.id));
        let audit_values = vec![
            manual_event.old_value.clone().unwrap_or(Value::Null),
            manual_event.new_value.clone().unwrap_or(Value::Null),
        ];
        let audit_json = serde_json::to_string(&audit_values).expect("audit should serialize");
        assert!(!audit_json.contains("13800000000"));
        assert!(!audit_json.contains("user@example.test"));
        assert!(!audit_json.contains("avatar.png"));
    }

    #[tokio::test]
    async fn reading_another_users_profile_requires_users_read_permission() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let now = Utc::now();
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        let target = context
            .users
            .find_or_create_for_login("target-user", now)
            .await
            .expect("target user should be created");
        context
            .profiles
            .upsert_profile(target.id, profile_input("李四"), now)
            .await
            .expect("target profile should be created");

        let forbidden = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/{}/profile", target.id))
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile request should be handled");
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        context
            .authz
            .create_policy(
                "user".to_string(),
                current.id,
                "users".to_string(),
                "read".to_string(),
                "allow".to_string(),
            )
            .await
            .expect("users:read policy should be created");

        let allowed = context
            .app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/{}/profile", target.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile request should be handled");
        assert_eq!(allowed.status(), StatusCode::OK);
        let body = response_json(allowed).await;
        assert_eq!(body.pointer("/name").and_then(Value::as_str), Some("李四"));
    }

    #[tokio::test]
    async fn syncing_another_users_profile_requires_users_write_permission() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let now = Utc::now();
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        let target = context
            .users
            .find_or_create_for_login("target-user", now)
            .await
            .expect("target user should be created");

        let forbidden = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/{}/profile/sync", target.id))
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile sync request should be handled");
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        context
            .authz
            .create_policy(
                "user".to_string(),
                current.id,
                "users".to_string(),
                "write".to_string(),
                "allow".to_string(),
            )
            .await
            .expect("users:write policy should be created");

        let allowed = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/{}/profile/sync", target.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile sync request should be handled");
        assert_eq!(allowed.status(), StatusCode::OK);
        let body = response_json(allowed).await;
        assert_eq!(body.pointer("/name").and_then(Value::as_str), Some("李四"));
    }

    #[tokio::test]
    async fn sync_user_profile_reports_missing_user_and_dingtalk_errors() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        context
            .authz
            .create_policy(
                "user".to_string(),
                current.id,
                "users".to_string(),
                "write".to_string(),
                "allow".to_string(),
            )
            .await
            .expect("users:write policy should be created");

        let missing_user = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/{}/profile/sync", Uuid::new_v4()))
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile sync request should be handled");
        assert_eq!(missing_user.status(), StatusCode::NOT_FOUND);

        let target = context
            .users
            .find_or_create_for_login("provider-error", Utc::now())
            .await
            .expect("target user should be created");
        let provider_error = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/{}/profile/sync", target.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile sync request should be handled");
        assert_eq!(provider_error.status(), StatusCode::BAD_GATEWAY);
        let profile = context
            .profiles
            .find_by_user_id(target.id)
            .await
            .expect("profile lookup should succeed");
        assert!(profile.is_none());
    }

    #[tokio::test]
    async fn user_profile_reports_missing_user_or_profile() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        context
            .authz
            .create_policy(
                "user".to_string(),
                current.id,
                "users".to_string(),
                "read".to_string(),
                "allow".to_string(),
            )
            .await
            .expect("users:read policy should be created");

        let missing_user = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/{}/profile", Uuid::new_v4()))
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile request should be handled");
        assert_eq!(missing_user.status(), StatusCode::NOT_FOUND);

        let target = context
            .users
            .find_or_create_for_login("target-without-profile", Utc::now())
            .await
            .expect("target user should be created");
        let missing_profile = context
            .app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/{}/profile", target.id))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("profile request should be handled");
        assert_eq!(missing_profile.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_users_validates_query_parameters() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        grant(&context, current.id, "users", "read").await;

        for uri in [
            "/api/v1/users/list?status_filter=deleted",
            "/api/v1/users/list?page_number=0",
            "/api/v1/users/list?page_size=201",
            "/api/v1/users/list?page_size=abc",
        ] {
            let response = context
                .app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header(header::COOKIE, cookie.clone())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .expect("users list request should be handled");

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn update_status_disables_user_and_revokes_sessions() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let admin_cookie = login_and_cookie(context.app.clone()).await;
        let admin = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("admin user should exist");
        grant(&context, admin.id, "users", "write").await;
        let now = Utc::now();
        let target = context
            .users
            .find_or_create_for_login("target-user", now)
            .await
            .expect("target user should be created");
        let target_token = "target-session-token";
        let target_cookie = format!("atlas_session={target_token}");
        context
            .sessions
            .create_session(
                target.id,
                &hash_secret(target_token),
                now,
                now + Duration::hours(1),
            )
            .await
            .expect("target session should be created");

        let response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/update-status/{}", target.id))
                    .header(header::COOKIE, admin_cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"target_status":"disabled"}"#))
                    .unwrap(),
            )
            .await
            .expect("update status request should be handled");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/status").and_then(Value::as_str),
            Some("disabled")
        );
        assert_eq!(
            body.pointer("/dingtalk_user_id").and_then(Value::as_str),
            Some("target-user")
        );

        let target_me_response = context
            .app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, target_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");
        assert_eq!(target_me_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn update_status_rejects_disabling_own_account() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        grant(&context, current.id, "users", "write").await;

        let response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/update-status/{}", current.id))
                    .header(header::COOKIE, cookie.clone())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"target_status":"disabled"}"#))
                    .unwrap(),
            )
            .await
            .expect("update status request should be handled");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("cannot_disable_self")
        );

        // 账号未被停用,会话仍有效
        let me_response = context
            .app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");
        assert_eq!(me_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn update_status_reports_invalid_body_and_missing_user() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        grant(&context, current.id, "users", "write").await;

        let invalid_status = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/update-status/{}", Uuid::new_v4()))
                    .header(header::COOKIE, cookie.clone())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"target_status":"deleted"}"#))
                    .unwrap(),
            )
            .await
            .expect("update status request should be handled");
        assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);

        let forbidden_mapping_update = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/update-status/{}", Uuid::new_v4()))
                    .header(header::COOKIE, cookie.clone())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"target_status":"active","dingtalk_user_id":"not-allowed"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("update status request should be handled");
        assert_eq!(forbidden_mapping_update.status(), StatusCode::BAD_REQUEST);

        let missing_user = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/update-status/{}", Uuid::new_v4()))
                    .header(header::COOKIE, cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"target_status":"active"}"#))
                    .unwrap(),
            )
            .await
            .expect("update status request should be handled");
        assert_eq!(missing_user.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn authenticated_user_can_delete_user() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let admin_cookie = login_and_cookie(context.app.clone()).await;
        let admin = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("admin user should exist");
        grant(&context, admin.id, "users", "write").await;
        grant(&context, admin.id, "users", "read").await;
        let now = Utc::now();
        let target = context
            .users
            .find_or_create_for_login("target-user", now)
            .await
            .expect("target user should be created");
        let target_id = target.id.to_string();
        let target_token = "target-session-token";
        let target_cookie = format!("atlas_session={target_token}");
        context
            .sessions
            .create_session(
                target.id,
                &hash_secret(target_token),
                now,
                now + Duration::hours(1),
            )
            .await
            .expect("target session should be created");

        let delete_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/delete/{}", target.id))
                    .header(header::COOKIE, admin_cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("delete user request should be handled");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let list_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/users/list")
                    .header(header::COOKIE, admin_cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("users list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let body = response_json(list_response).await;
        let users = body
            .pointer("/users")
            .and_then(Value::as_array)
            .expect("users list should be an array");
        assert!(
            users
                .iter()
                .all(|user| user.pointer("/id").and_then(Value::as_str)
                    != Some(target_id.as_str()))
        );

        let detail_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/users/detail/{}", target.id))
                    .header(header::COOKIE, admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("user detail request should be handled");
        assert_eq!(detail_response.status(), StatusCode::NOT_FOUND);

        let target_me_response = context
            .app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, target_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");
        assert_eq!(target_me_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn authenticated_user_can_delete_self_and_current_cookie_expires() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let me_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");
        assert_eq!(me_response.status(), StatusCode::OK);
        let body = response_json(me_response).await;
        let user_id = body
            .pointer("/id")
            .and_then(Value::as_str)
            .expect("me should include user id");
        let user_id = Uuid::parse_str(user_id).expect("me should return a uuid id");
        grant(&context, user_id, "users", "write").await;

        let delete_response = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/delete/{user_id}"))
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("delete user request should be handled");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let stale_me_response = context
            .app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");
        assert_eq!(stale_me_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn delete_user_reports_invalid_id_and_missing_user() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let current = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("current user should exist");
        grant(&context, current.id, "users", "write").await;

        let invalid_id = context
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/users/delete/not-a-uuid")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("delete user request should be handled");
        assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);

        let missing_user = context
            .app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("/api/v1/users/delete/{}", Uuid::new_v4()))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("delete user request should be handled");
        assert_eq!(missing_user.status(), StatusCode::NOT_FOUND);
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

    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        serde_json::from_slice(&body).expect("response body should be json")
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
        let events = EventRepository::new(db.clone());
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
            events.clone(),
            sessions.clone(),
            86_400,
        );
        let departments_service = DepartmentService::new(dingtalk_config, departments.clone());
        let authz = AuthzService::new(AuthzRepository::new(db.clone()))
            .await
            .expect("test authz service should initialize");
        let users_service = UserService::new(users.clone(), profiles.clone(), sessions.clone());
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
            events.clone(),
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
            profiles,
            events,
            sessions,
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
                    "userId": "ding-user-1",
                    "name": "个人张三",
                    "avatarUrl": "https://example.test/avatar.png",
                    "mobile": "13800000000",
                    "email": "user@example.test"
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

        #[derive(serde::Deserialize)]
        struct UserDetailForm {
            userid: String,
            language: String,
        }

        let user_detail = |axum::Form(form): axum::Form<UserDetailForm>| async move {
            assert_eq!(form.language, "zh_CN");
            match form.userid.as_str() {
                "ding-user-1" => Json(json!({
                    "errcode": 0,
                    "errmsg": "ok",
                    "result": {
                        "name": "张三",
                        "dept_id_list": [10, 20],
                        "active": true
                    }
                })),
                "target-user" => Json(json!({
                    "errcode": 0,
                    "errmsg": "ok",
                    "result": {
                        "name": "李四",
                        "dept_id_list": [30],
                        "active": true
                    }
                })),
                "provider-error" => Json(json!({
                    "errcode": 50001,
                    "errmsg": "provider unavailable"
                })),
                _ => Json(json!({
                    "errcode": 60121,
                    "errmsg": "user not found"
                })),
            }
        };

        let app = Router::new()
            .route("/token", post(token))
            .route("/me", get(me))
            .route("/gettoken", get(gettoken))
            .route("/user_detail", post(user_detail));
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

    fn profile_input(name: &str) -> UserProfileUpsert {
        UserProfileUpsert {
            name: Some(name.to_string()),
            avatar_url: None,
            mobile: None,
            hide_mobile: None,
            telephone: None,
            job_number: None,
            title: None,
            email: None,
            org_email: None,
            work_place: None,
            remark: None,
            department_external_ids: None,
            is_admin: None,
            is_boss: None,
            is_active: Some(true),
            is_senior: None,
            hired_at: None,
        }
    }
}

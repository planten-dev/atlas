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
        users::{ListUsersQuery, UpdateUserStatusRequest},
    },
    repositories::RepositoryError,
    services::users::UserError,
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

pub async fn update_user_status(
    State(state): State<AppState>,
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

    match state.users.update_status(user_id, request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => user_error_response(error),
    }
}

pub async fn delete_user(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let user_id = match path {
        Ok(Path(user_id)) => user_id,
        Err(error) => return validation_error_response("invalid user_id path parameter", error),
    };

    match state.users.delete_user(user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
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
        UserError::UserNotFound => StatusCode::NOT_FOUND,
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
            sessions::{SessionRepository, hash_secret},
            users::UserRepository,
        },
        services::{auth::AuthService, authz::AuthzService, users::UserService},
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
        sessions: SessionRepository,
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
    async fn list_users_validates_query_parameters() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

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
    async fn update_status_reports_invalid_body_and_missing_user() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

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
        let auth = AuthService::new(
            DingTalkConfig {
                client_id: "test-client-id".to_string(),
                client_secret: "test-client-secret".to_string(),
                redirect_uri: "http://127.0.0.1:3000/api/v1/auth/callback/dingtalk".to_string(),
                auth_url: "https://login.dingtalk.com/oauth2/auth".to_string(),
                token_url: format!("{mock_base_url}/token"),
                user_info_url: format!("{mock_base_url}/me"),
                scope: "openid".to_string(),
                corp_id: "".to_string(),
                external_id_fields: vec!["userId".to_string()],
            },
            users.clone(),
            sessions.clone(),
            86_400,
        );
        let authz = AuthzService::new(AuthzRepository::new(db))
            .await
            .expect("test authz service should initialize");
        let users_service = UserService::new(users.clone(), sessions.clone());
        let state = AppState::new(
            auth,
            authz,
            users_service,
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
            sessions,
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
}

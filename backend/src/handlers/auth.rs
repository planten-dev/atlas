use axum::{
    Json,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};

use crate::{
    dto::auth::DingTalkCallbackQuery,
    handlers::error::auth_error_response,
    middleware::auth::SESSION_COOKIE_NAME,
    services::auth::{AuthError, CurrentSession, DingTalkCallbackInput, LoginResponse},
    state::AppState,
};

pub async fn dingtalk_login(State(state): State<AppState>) -> Response {
    match state.auth.begin_dingtalk_login().await {
        Ok(url) => Redirect::temporary(&url).into_response(),
        Err(error) => auth_error_response(error),
    }
}

pub async fn dingtalk_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DingTalkCallbackQuery>,
) -> Response {
    let wants_html_redirect = request_prefers_html(&headers);
    let input = DingTalkCallbackInput {
        code: query.code,
        auth_code: query.auth_code,
        state: query.state,
        error: query.error,
        error_description: query.error_description,
    };

    match state.auth.complete_dingtalk_callback(input).await {
        Ok(login) => {
            let cookie = session_cookie(
                &login.session_token,
                state.session_config.ttl_seconds,
                state.session_config.cookie_secure,
            );

            if wants_html_redirect
                && let Some(mut redirect) =
                    frontend_success_redirect(&state.auth_config.frontend_callback_url)
            {
                redirect.headers_mut().insert(header::SET_COOKIE, cookie);
                return redirect;
            }

            let mut response = (
                StatusCode::OK,
                Json(LoginResponse {
                    user: login.user.clone(),
                }),
            )
                .into_response();
            response.headers_mut().insert(header::SET_COOKIE, cookie);
            response
        }
        Err(error) => {
            if wants_html_redirect
                && let Some(redirect) =
                    frontend_error_redirect(&state.auth_config.frontend_callback_url, &error)
            {
                return redirect;
            }

            auth_error_response(error)
        }
    }
}

pub async fn me(Extension(current_session): Extension<CurrentSession>) -> Response {
    (StatusCode::OK, Json(current_session.user)).into_response()
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
) -> Response {
    let clear_cookie = clear_session_cookie(state.session_config.cookie_secure);

    match state.auth.logout(current_session.session_id).await {
        Ok(()) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            response
                .headers_mut()
                .insert(header::SET_COOKIE, clear_cookie);
            response
        }
        Err(error) => {
            let mut response = auth_error_response(error);
            response
                .headers_mut()
                .insert(header::SET_COOKIE, clear_cookie);
            response
        }
    }
}

fn request_prefers_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html"))
        .unwrap_or(false)
}

fn frontend_success_redirect(callback_url: &str) -> Option<Response> {
    let callback_url = normalize_optional(callback_url)?;
    Some(Redirect::temporary(callback_url).into_response())
}

fn frontend_error_redirect(callback_url: &str, error: &AuthError) -> Option<Response> {
    let callback_url = normalize_optional(callback_url)?;
    let redirect_url = format!(
        "{}#error={}&error_description={}",
        callback_url,
        percent_encode(error.code()),
        percent_encode(&error.to_string())
    );
    Some(Redirect::temporary(&redirect_url).into_response())
}

fn normalize_optional(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() { None } else { Some(value) }
}

fn session_cookie(token: &str, ttl_seconds: u64, secure: bool) -> header::HeaderValue {
    let mut cookie = format!(
        "{SESSION_COOKIE_NAME}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        percent_encode(token),
        ttl_seconds
    );
    if secure {
        cookie.push_str("; Secure");
    }
    header::HeaderValue::from_str(&cookie).expect("session cookie should be a valid header")
}

fn clear_session_cookie(secure: bool) -> header::HeaderValue {
    let mut cookie = format!("{SESSION_COOKIE_NAME}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    header::HeaderValue::from_str(&cookie).expect("clear cookie should be a valid header")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app,
        config::{AuthConfig, DatabaseConfig, DatabaseKind, DingTalkConfig, SessionConfig},
        db,
        entities::users as users_entity,
        repositories::{sessions::SessionRepository, users::UserRepository},
        services::{auth::AuthService, users::UserService},
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request},
        routing::{get, post},
    };
    use sea_orm::{ActiveModelTrait, Set};
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    struct TestContext {
        app: Router,
        users: UserRepository,
    }

    #[tokio::test]
    async fn login_redirects_to_dingtalk_and_callback_creates_session_cookie() {
        let mock_base_url = start_mock_dingtalk().await;
        let app = test_app(&mock_base_url).await;

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/login/dingtalk")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("login request should be handled");
        assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
        let location = login_response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .expect("login should include location");
        assert!(location.contains("clientId=test-client-id"));
        let state = query_param(location, "state").expect("state should be present");

        let callback_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/auth/callback/dingtalk?authCode=test-code&state={state}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("callback request should be handled");

        assert_eq!(callback_response.status(), StatusCode::OK);
        let cookie = callback_response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .expect("callback should set session cookie")
            .to_string();
        assert!(cookie.starts_with("atlas_session="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));

        let body = to_bytes(callback_response.into_body(), usize::MAX)
            .await
            .expect("callback body should be readable");
        let body: Value = serde_json::from_slice(&body).expect("callback body should be json");
        assert_eq!(
            body.pointer("/user/dingtalk_user_id")
                .and_then(Value::as_str),
            Some("ding-user-1")
        );
    }

    #[tokio::test]
    async fn me_and_logout_use_session_cookie() {
        let mock_base_url = start_mock_dingtalk().await;
        let app = test_app(&mock_base_url).await;
        let cookie = login_and_cookie(app.clone()).await;

        let me_response = app
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

        let logout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/auth/logout")
                    .header(header::COOKIE, cookie.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("logout request should be handled");
        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);
        assert!(
            logout_response
                .headers()
                .get(header::SET_COOKIE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .contains("Max-Age=0")
        );

        let me_after_logout = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");
        assert_eq!(me_after_logout.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn callback_rejects_replayed_state() {
        let mock_base_url = start_mock_dingtalk().await;
        let app = test_app(&mock_base_url).await;
        let location = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/login/dingtalk")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("login should be handled")
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .expect("login should include location")
            .to_string();
        let state = query_param(&location, "state").expect("state should be present");

        for expected_status in [StatusCode::OK, StatusCode::BAD_REQUEST] {
            let response = app
                .clone()
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
            assert_eq!(response.status(), expected_status);
        }
    }

    #[tokio::test]
    async fn missing_session_cookie_returns_unauthorized() {
        let mock_base_url = start_mock_dingtalk().await;
        let app = test_app(&mock_base_url).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn invalid_session_cookie_returns_unauthorized() {
        let mock_base_url = start_mock_dingtalk().await;
        let app = test_app(&mock_base_url).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header::COOKIE, "atlas_session=invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("me request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn disabled_user_session_returns_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let user = context
            .users
            .find_by_dingtalk_user_id("ding-user-1")
            .await
            .expect("user lookup should succeed")
            .expect("user should exist after login");
        let mut active: users_entity::ActiveModel = user.into();
        active.status = Set("disabled".to_string());
        active
            .update(&context.users.db)
            .await
            .expect("user should be disabled");

        let response = context
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

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
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

    async fn test_app(mock_base_url: &str) -> Router {
        test_context(mock_base_url).await.app
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
        let sessions = SessionRepository::new(db);
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
        let users_service = UserService::new(users.clone(), sessions);
        let state = AppState::new(
            auth,
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

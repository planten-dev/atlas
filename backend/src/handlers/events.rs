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
        events::{ListEventsQuery, ReviewEventRequest},
    },
    repositories::RepositoryError,
    services::{
        auth::CurrentSession,
        events::{EventError, ReviewDecision},
    },
    state::AppState,
};

pub async fn list_events(
    State(state): State<AppState>,
    query: Result<Query<ListEventsQuery>, QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(error) => return validation_error_response("invalid query parameters", error),
    };

    match state.events.list_events(query).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => event_error_response(error),
    }
}

pub async fn event_detail(
    State(state): State<AppState>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Response {
    let event_id = match path {
        Ok(Path(event_id)) => event_id,
        Err(error) => return validation_error_response("invalid event_id path parameter", error),
    };

    match state.events.event_detail(event_id).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => event_error_response(error),
    }
}

pub async fn approve_event(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<ReviewEventRequest>, JsonRejection>,
) -> Response {
    review_event(
        state,
        current_session,
        path,
        request,
        ReviewDecision::Approve,
    )
    .await
}

pub async fn reject_event(
    State(state): State<AppState>,
    Extension(current_session): Extension<CurrentSession>,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<ReviewEventRequest>, JsonRejection>,
) -> Response {
    review_event(
        state,
        current_session,
        path,
        request,
        ReviewDecision::Reject,
    )
    .await
}

async fn review_event(
    state: AppState,
    current_session: CurrentSession,
    path: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<ReviewEventRequest>, JsonRejection>,
    decision: ReviewDecision,
) -> Response {
    let event_id = match path {
        Ok(Path(event_id)) => event_id,
        Err(error) => return validation_error_response("invalid event_id path parameter", error),
    };
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => return validation_error_response("invalid request body", error),
    };

    match state
        .events
        .review(event_id, current_session.user.id, decision, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => event_error_response(error),
    }
}

fn event_error_response(error: EventError) -> Response {
    let status = status_code(&error);
    let code = error.code();

    if status.is_server_error() {
        error!(
            status = status.as_u16(),
            code,
            message = %error,
            "event request failed"
        );
    } else {
        warn!(
            status = status.as_u16(),
            code,
            message = %error,
            "event request rejected"
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
    warn!(error, detail = %detail, "event request validation failed");
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "validation_error".to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn status_code(error: &EventError) -> StatusCode {
    match error {
        EventError::Repository(error) => match error {
            RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RepositoryError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RepositoryError::DisabledUser => StatusCode::FORBIDDEN,
        },
        EventError::Authz(_) => StatusCode::INTERNAL_SERVER_ERROR,
        EventError::EventNotFound => StatusCode::NOT_FOUND,
        EventError::EventNotReviewable => StatusCode::UNPROCESSABLE_ENTITY,
        EventError::EventNotPending => StatusCode::CONFLICT,
        EventError::DuplicateReview => StatusCode::CONFLICT,
        EventError::PermissionDenied { .. } => StatusCode::FORBIDDEN,
        EventError::UnknownResourceType { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        EventError::MissingPayload { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        EventError::Apply(_) | EventError::Serialization(_) => StatusCode::INTERNAL_SERVER_ERROR,
        EventError::InvalidInput(_)
        | EventError::InvalidPaginationMinimum { .. }
        | EventError::InvalidPaginationMaximum { .. }
        | EventError::InvalidEnumValue { .. } => StatusCode::BAD_REQUEST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app,
        config::{AuthConfig, DatabaseConfig, DatabaseKind, DingTalkConfig, SessionConfig},
        db,
        entities::products,
        repositories::{
            authz::AuthzRepository, events::EventRepository, products::ProductRepository,
            sessions::SessionRepository, users::UserRepository,
        },
        services::{
            auth::AuthService,
            authz::AuthzService,
            events::EventService,
            products::ProductService,
            review::{ApplierRegistry, ApplyError, ReviewableResource},
            users::UserService,
        },
        state::AppState,
    };
    use async_trait::async_trait;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, header},
        routing::{get, post},
    };
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::Decimal;
    use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use std::{path::PathBuf, str::FromStr, sync::Arc};
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    #[derive(Debug, Serialize, Deserialize)]
    struct TestProduct {
        name: String,
        unit_price: String,
    }

    #[async_trait]
    impl ReviewableResource for TestProduct {
        const RESOURCE_TYPE: &'static str = "products";
        const APPROVAL_PERMISSION: &'static str = "products:approve";

        async fn apply_insert(
            self,
            tx: &DatabaseTransaction,
            now: DateTime<Utc>,
        ) -> Result<Uuid, ApplyError> {
            let product = products::ActiveModel {
                id: Set(Uuid::new_v4()),
                name: Set(self.name),
                category: Set(None),
                series: Set(None),
                brand_name: Set(None),
                specification: Set(None),
                unit: Set(None),
                unit_price: Set(
                    Decimal::from_str(&self.unit_price).map_err(|_| ApplyError::ResourceMissing)?
                ),
                status: Set("active".to_string()),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(tx)
            .await?;
            Ok(product.id)
        }

        async fn apply_update(
            self,
            _tx: &DatabaseTransaction,
            _resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            unimplemented!("handler tests only exercise create events")
        }

        async fn apply_delete(
            _tx: &DatabaseTransaction,
            _resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            unimplemented!("handler tests only exercise create events")
        }
    }

    struct TestContext {
        app: Router,
        users: UserRepository,
        authz: AuthzService,
        events: EventService,
    }

    #[tokio::test]
    async fn events_api_requires_session() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;

        let response = context
            .app
            .clone()
            .oneshot(request(Method::GET, "/api/v1/events/list", None, None))
            .await
            .expect("events list request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/approve/{}", Uuid::new_v4()),
                None,
                Some(json!({})),
            ))
            .await
            .expect("event approve request should be handled");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_requires_events_read_permission() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let response = context
            .app
            .oneshot(request(
                Method::GET,
                "/api/v1/events/list",
                Some(&cookie),
                None,
            ))
            .await
            .expect("events list request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_denied")
        );
    }

    #[tokio::test]
    async fn approve_and_reject_via_http() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;
        let reviewer_id = logged_in_user_id(&context).await;
        grant(&context, reviewer_id, "events", "read").await;
        grant(&context, reviewer_id, "products", "approve").await;

        // Events are submitted by handlers through the service; simulate a
        // separate actor here.
        let actor = context
            .users
            .find_or_create_for_login("ding-actor", Utc::now())
            .await
            .expect("actor should be created")
            .id;
        let approve_target = context
            .events
            .submit_create(
                actor,
                &TestProduct {
                    name: "http approved".to_string(),
                    unit_price: "10.00".to_string(),
                },
                1,
            )
            .await
            .expect("event should be submitted");
        let reject_target = context
            .events
            .submit_create(
                actor,
                &TestProduct {
                    name: "http rejected".to_string(),
                    unit_price: "10.00".to_string(),
                },
                1,
            )
            .await
            .expect("event should be submitted");

        let list_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/v1/events/list?approval_status=1",
                Some(&cookie),
                None,
            ))
            .await
            .expect("events list request should be handled");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list = response_json(list_response).await;
        assert_eq!(
            list.pointer("/total_count").and_then(Value::as_u64),
            Some(2)
        );

        let approve_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/approve/{}", approve_target.id),
                Some(&cookie),
                Some(json!({})),
            ))
            .await
            .expect("event approve request should be handled");
        assert_eq!(approve_response.status(), StatusCode::OK);
        let approved = response_json(approve_response).await;
        assert_eq!(
            approved.pointer("/approval_status").and_then(Value::as_i64),
            Some(2)
        );
        assert!(
            approved
                .pointer("/resource_id")
                .and_then(Value::as_str)
                .is_some()
        );

        let reject_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/reject/{}", reject_target.id),
                Some(&cookie),
                Some(json!({"remark": "not needed"})),
            ))
            .await
            .expect("event reject request should be handled");
        assert_eq!(reject_response.status(), StatusCode::OK);
        let rejected = response_json(reject_response).await;
        assert_eq!(
            rejected.pointer("/approval_status").and_then(Value::as_i64),
            Some(3)
        );

        let detail_response = context
            .app
            .clone()
            .oneshot(request(
                Method::GET,
                &format!("/api/v1/events/detail/{}", reject_target.id),
                Some(&cookie),
                None,
            ))
            .await
            .expect("event detail request should be handled");
        assert_eq!(detail_response.status(), StatusCode::OK);

        // Finalized events reject further reviews.
        let conflict_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/approve/{}", reject_target.id),
                Some(&cookie),
                Some(json!({})),
            ))
            .await
            .expect("event approve request should be handled");
        assert_eq!(conflict_response.status(), StatusCode::CONFLICT);

        let missing_response = context
            .app
            .clone()
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/approve/{}", Uuid::new_v4()),
                Some(&cookie),
                Some(json!({})),
            ))
            .await
            .expect("event approve request should be handled");
        assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);

        let invalid_response = context
            .app
            .oneshot(request(
                Method::POST,
                "/api/v1/events/approve/not-a-uuid",
                Some(&cookie),
                Some(json!({})),
            ))
            .await
            .expect("event approve request should be handled");
        assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn review_without_approval_permission_is_forbidden() {
        let mock_base_url = start_mock_dingtalk().await;
        let context = test_context(&mock_base_url).await;
        let cookie = login_and_cookie(context.app.clone()).await;

        let actor = context
            .users
            .find_or_create_for_login("ding-actor", Utc::now())
            .await
            .expect("actor should be created")
            .id;
        let event = context
            .events
            .submit_create(
                actor,
                &TestProduct {
                    name: "guarded".to_string(),
                    unit_price: "10.00".to_string(),
                },
                1,
            )
            .await
            .expect("event should be submitted");

        let response = context
            .app
            .oneshot(request(
                Method::POST,
                &format!("/api/v1/events/approve/{}", event.id),
                Some(&cookie),
                Some(json!({})),
            ))
            .await
            .expect("event approve request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(
            body.pointer("/error").and_then(Value::as_str),
            Some("permission_denied")
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
        let sessions = SessionRepository::new(db.clone());
        let products = ProductRepository::new(db.clone());
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
        let products_service = ProductService::new(products);
        let mut registry = ApplierRegistry::new();
        registry.register::<TestProduct>();
        let events_service = EventService::new(
            EventRepository::new(db),
            authz.clone(),
            Arc::new(registry),
            180,
        );
        let state = AppState::new(
            auth,
            authz.clone(),
            users_service,
            products_service,
            events_service.clone(),
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
            events: events_service,
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

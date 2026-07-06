use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::users::{CreateUserRequest, LoginUserRequest, UserResponse},
    errors::AppResult,
    services::users::UserService,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/create", post(create_user))
        .route("/users/login", post(login_user))
        .route("/users/{id}", get(get_user))
}

#[tracing::instrument(
    name = "users.handler.create_user",
    skip(state, payload),
    fields(username_len = tracing::field::Empty, phone_len = tracing::field::Empty)
)]
async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> AppResult<(StatusCode, Json<UserResponse>)> {
    tracing::Span::current().record("username_len", payload.username.chars().count());
    tracing::Span::current().record("phone_len", payload.phone.chars().count());
    tracing::debug!("received create user request");
    let user = UserService::create_user(state.db.as_ref(), payload).await?;
    tracing::info!(user_id = %user.id, "create user request completed");

    Ok((StatusCode::CREATED, Json(user)))
}

#[tracing::instrument(name = "users.handler.get_user", skip(state), fields(user_id = %id))]
async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserResponse>> {
    tracing::debug!("received get user request");
    let user = UserService::get_user(state.db.as_ref(), id).await?;
    tracing::debug!(user_id = %user.id, "get user request completed");

    Ok(Json(user))
}

#[tracing::instrument(
    name = "users.handler.login_user",
    skip(state, payload),
    fields(identifier_kind = tracing::field::Empty, identifier_len = tracing::field::Empty)
)]
async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginUserRequest>,
) -> AppResult<Json<UserResponse>> {
    tracing::Span::current().record(
        "identifier_kind",
        classify_login_identifier(&payload.identifier),
    );
    tracing::Span::current().record("identifier_len", payload.identifier.chars().count());
    tracing::debug!("received login request");
    let user = UserService::login_user(state.db.as_ref(), payload).await?;
    tracing::info!(user_id = %user.id, "login request completed");

    Ok(Json(user))
}

fn classify_login_identifier(identifier: &str) -> &'static str {
    if identifier
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '+')
    {
        "phone"
    } else {
        "username"
    }
}

#[cfg(test)]
mod tests {
    use super::routes;
    use crate::{
        app_state::AppState,
        entities::users::{Model as UserModel, UserStatus},
    };
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use chrono::NaiveDate;
    use sea_orm::{DbBackend, MockDatabase};
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_user_endpoint_returns_created_user_without_password_hash() {
        let user = user_model("alice", "13800138000", "stored-hash", UserStatus::Active);
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<UserModel>::new(), Vec::<UserModel>::new(), vec![user]])
            .into_connection();
        let app = routes().with_state(AppState::new(db));

        let response = app
            .oneshot(json_request(
                "POST",
                "/users/create",
                json!({
                    "username": "alice",
                    "phone": "13800138000",
                    "password": "secret-password"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = response_body(response).await;
        assert_eq!(body["username"], "alice");
        assert_eq!(body["phone"], "13800138000");
        assert_eq!(body["status"], "active");
        assert!(body.get("password_hash").is_none());
    }

    #[tokio::test]
    async fn get_user_endpoint_returns_user_without_password_hash() {
        let user = user_model("alice", "13800138000", "stored-hash", UserStatus::Active);
        let id = user.id;
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![user]])
            .into_connection();
        let app = routes().with_state(AppState::new(db));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/users/{id}"))
                    .body(Body::empty())
                    .expect("request should be built"),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK);

        let body = response_body(response).await;
        assert_eq!(body["id"], id.to_string());
        assert_eq!(body["username"], "alice");
        assert!(body.get("password_hash").is_none());
    }

    #[tokio::test]
    async fn login_endpoint_returns_unauthorized_for_unknown_identifier() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<UserModel>::new()])
            .into_connection();
        let app = routes().with_state(AppState::new(db));

        let response = app
            .oneshot(json_request(
                "POST",
                "/users/login",
                json!({
                    "identifier": "alice",
                    "password": "secret-password"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response_body(response).await;
        assert_eq!(body["code"], "unauthorized");
        assert_eq!(body["error"], "invalid login credentials");
    }

    fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should be built")
    }

    async fn response_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");

        serde_json::from_slice(&bytes).expect("body should be json")
    }

    fn user_model(
        username: &str,
        phone: &str,
        password_hash: &str,
        status: UserStatus,
    ) -> UserModel {
        UserModel {
            id: Uuid::from_u128(1),
            username: username.to_string(),
            phone: phone.to_string(),
            password_hash: password_hash.to_string(),
            status,
            created_at: NaiveDate::from_ymd_opt(2026, 1, 1)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time"),
            updated_at: NaiveDate::from_ymd_opt(2026, 1, 1)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time"),
        }
    }
}

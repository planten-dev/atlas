use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::NoContent,
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    dto::users::{
        ChangeUserPasswordRequest, CreateUserRequest, ListUsersQuery, ListUsersResponse,
        LoginUserRequest, UpdateUserRequest, UpdateUserStatusRequest, UserResponse,
    },
    errors::AppResult,
    services::users::UserService,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/create", post(create_user))
        .route("/users/list", get(list_users))
        .route("/users/login", post(login_user))
        .route("/users/{id}", get(get_user))
        .route("/users/{id}/update", post(update_user))
        .route("/users/{id}/status/update", post(update_user_status))
        .route("/users/{id}/password/change", post(change_user_password))
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

#[tracing::instrument(name = "users.handler.list_users", skip(state, query))]
async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> AppResult<Json<ListUsersResponse>> {
    tracing::debug!("received list users request");
    let users = UserService::list_users(state.db.as_ref(), query).await?;
    tracing::debug!(
        total = users.total,
        returned = users.items.len(),
        "list users request completed"
    );

    Ok(Json(users))
}

#[tracing::instrument(
    name = "users.handler.update_user",
    skip(state, payload),
    fields(user_id = %id, has_username = payload.username.is_some(), has_phone = payload.phone.is_some())
)]
async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> AppResult<Json<UserResponse>> {
    tracing::debug!("received update user request");
    let user = UserService::update_user(state.db.as_ref(), id, payload).await?;
    tracing::info!(user_id = %user.id, "update user request completed");

    Ok(Json(user))
}

#[tracing::instrument(
    name = "users.handler.update_user_status",
    skip(state, payload),
    fields(user_id = %id, status = ?payload.status)
)]
async fn update_user_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateUserStatusRequest>,
) -> AppResult<Json<UserResponse>> {
    tracing::debug!("received update user status request");
    let user = UserService::update_user_status(state.db.as_ref(), id, payload).await?;
    tracing::info!(user_id = %user.id, status = ?user.status, "update user status request completed");

    Ok(Json(user))
}

#[tracing::instrument(name = "users.handler.change_user_password", skip(state, payload), fields(user_id = %id))]
async fn change_user_password(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ChangeUserPasswordRequest>,
) -> AppResult<NoContent> {
    tracing::debug!("received change user password request");
    match UserService::change_user_password(state.db.as_ref(), id, payload).await {
        Ok(()) => {
            tracing::info!(user_id = %id, "change user password request succeeded");
            Ok(NoContent)
        }
        Err(err) => {
            tracing::warn!(user_id = %id, error = %err, "change user password request failed");
            Err(err)
        }
    }
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
    use argon2::{
        Argon2, PasswordHasher,
        password_hash::{SaltString, rand_core::OsRng},
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
    async fn create_user_endpoint_rejects_id_in_body() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let app = routes().with_state(AppState::new(db));

        let response = app
            .oneshot(json_request(
                "POST",
                "/users/create",
                json!({
                    "id": Uuid::from_u128(1),
                    "username": "alice",
                    "phone": "13800138000",
                    "password": "secret-password"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
    async fn update_user_endpoint_rejects_id_in_body() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let app = routes().with_state(AppState::new(db));
        let id = Uuid::from_u128(1);

        let response = app
            .oneshot(json_request(
                "POST",
                &format!("/users/{id}/update"),
                json!({
                    "id": Uuid::from_u128(2),
                    "username": "alice2"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn update_status_endpoint_rejects_id_in_body() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let app = routes().with_state(AppState::new(db));
        let id = Uuid::from_u128(1);

        let response = app
            .oneshot(json_request(
                "POST",
                &format!("/users/{id}/status/update"),
                json!({
                    "id": Uuid::from_u128(2),
                    "status": "disabled"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn update_status_endpoint_rejects_invalid_status() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let app = routes().with_state(AppState::new(db));
        let id = Uuid::from_u128(1);

        let response = app
            .oneshot(json_request(
                "POST",
                &format!("/users/{id}/status/update"),
                json!({
                    "status": "locked"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn change_password_endpoint_rejects_id_in_body() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        let app = routes().with_state(AppState::new(db));
        let id = Uuid::from_u128(1);

        let response = app
            .oneshot(json_request(
                "POST",
                &format!("/users/{id}/password/change"),
                json!({
                    "id": Uuid::from_u128(2),
                    "current_password": "secret-password",
                    "new_password": "new-secret-password"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn change_password_endpoint_rejects_wrong_current_password() {
        let id = Uuid::from_u128(1);
        let user = user_model(
            "alice",
            "13800138000",
            &test_password_hash("secret-password"),
            UserStatus::Active,
        );
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![user]])
            .into_connection();
        let app = routes().with_state(AppState::new(db));

        let response = app
            .oneshot(json_request(
                "POST",
                &format!("/users/{id}/password/change"),
                json!({
                    "current_password": "wrong-password",
                    "new_password": "new-secret-password"
                }),
            ))
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response_body(response).await;
        assert_eq!(body["code"], "unauthorized");
        assert_eq!(body["error"], "invalid current password");
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

    fn test_password_hash(password: &str) -> String {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .expect("password hash should be created")
            .to_string()
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

use argon2::{
    Argon2,
    password_hash::{
        Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
};
use chrono::Utc;
use rand_core::OsRng;
use sea_orm::{DatabaseConnection, DbErr, Set};
use uuid::Uuid;

use crate::{
    dto::users::{CreateUserRequest, LoginUserRequest, UserResponse},
    entities::users::{ActiveModel, UserStatus},
    errors::{AppError, AppResult},
    repositories::users::UserRepository,
};

pub struct UserService;

impl UserService {
    #[tracing::instrument(
        name = "users.service.create_user",
        skip(db, request),
        fields(username_len = tracing::field::Empty, phone_len = tracing::field::Empty)
    )]
    pub async fn create_user(
        db: &DatabaseConnection,
        request: CreateUserRequest,
    ) -> AppResult<UserResponse> {
        let username = normalize_required(&request.username, "username")?;
        let phone = normalize_required(&request.phone, "phone")?;
        tracing::Span::current().record("username_len", username.chars().count());
        tracing::Span::current().record("phone_len", phone.chars().count());

        if username.chars().count() > 64 {
            tracing::warn!(
                field = "username",
                username_len = username.chars().count(),
                "user create validation failed"
            );
            return Err(AppError::BadRequest(
                "username must be at most 64 characters".to_string(),
            ));
        }

        if phone.chars().count() > 20 {
            tracing::warn!(
                field = "phone",
                phone_len = phone.chars().count(),
                "user create validation failed"
            );
            return Err(AppError::BadRequest(
                "phone must be at most 20 characters".to_string(),
            ));
        }

        if request.password.trim().is_empty() {
            tracing::warn!(field = "password", "user create validation failed");
            return Err(AppError::BadRequest("password is required".to_string()));
        }

        if UserRepository::find_by_username(db, &username)
            .await?
            .is_some()
        {
            tracing::warn!(
                field = "username",
                "user create rejected by unique constraint check"
            );
            return Err(AppError::Conflict("username already exists".to_string()));
        }

        if UserRepository::find_by_phone(db, &phone).await?.is_some() {
            tracing::warn!(
                field = "phone",
                "user create rejected by unique constraint check"
            );
            return Err(AppError::Conflict("phone already exists".to_string()));
        }

        tracing::debug!("creating password hash for new user");
        let now = Utc::now().naive_utc();
        let user = ActiveModel {
            id: Set(Uuid::new_v4()),
            username: Set(username),
            phone: Set(phone),
            password_hash: Set(hash_password(&request.password)?),
            status: Set(UserStatus::Active),
            created_at: Set(now),
            updated_at: Set(now),
        };

        match UserRepository::create(db, user).await {
            Ok(user) => {
                tracing::info!(user_id = %user.id, "user created");
                Ok(user.into())
            }
            Err(err) if is_unique_constraint_error(&err) => {
                tracing::warn!(error = %err, "user create rejected by database unique constraint");
                Err(AppError::Conflict(
                    "username or phone already exists".to_string(),
                ))
            }
            Err(err) => Err(err.into()),
        }
    }

    #[tracing::instrument(name = "users.service.get_user", skip(db), fields(user_id = %id))]
    pub async fn get_user(db: &DatabaseConnection, id: Uuid) -> AppResult<UserResponse> {
        let user = UserRepository::find_by_id(db, id).await?.ok_or_else(|| {
            tracing::warn!("user not found");
            AppError::NotFound("user not found".to_string())
        })?;

        tracing::debug!(status = ?user.status, "user loaded");

        Ok(user.into())
    }

    #[tracing::instrument(
        name = "users.service.login_user",
        skip(db, request),
        fields(
            identifier_kind = tracing::field::Empty,
            identifier_len = tracing::field::Empty,
            user_id = tracing::field::Empty
        )
    )]
    pub async fn login_user(
        db: &DatabaseConnection,
        request: LoginUserRequest,
    ) -> AppResult<UserResponse> {
        let identifier = normalize_required(&request.identifier, "identifier")?;
        tracing::Span::current().record("identifier_kind", classify_login_identifier(&identifier));
        tracing::Span::current().record("identifier_len", identifier.chars().count());

        if request.password.trim().is_empty() {
            tracing::warn!(field = "password", "login validation failed");
            return Err(AppError::BadRequest("password is required".to_string()));
        }

        let user = UserRepository::find_by_login_identifier(db, &identifier)
            .await?
            .ok_or_else(|| {
                tracing::warn!("login rejected because user was not found");
                AppError::Unauthorized("invalid login credentials".to_string())
            })?;
        tracing::Span::current().record("user_id", tracing::field::display(user.id));

        if user.status != UserStatus::Active {
            tracing::warn!(user_id = %user.id, status = ?user.status, "login rejected because user is disabled");
            return Err(AppError::Forbidden("user is disabled".to_string()));
        }

        if !verify_password(&request.password, &user.password_hash)? {
            tracing::warn!(user_id = %user.id, "login rejected because password did not verify");
            return Err(AppError::Unauthorized(
                "invalid login credentials".to_string(),
            ));
        }

        tracing::info!(user_id = %user.id, "user login succeeded");

        Ok(user.into())
    }
}

fn normalize_required(value: &str, field: &str) -> AppResult<String> {
    let value = value.trim();

    if value.is_empty() {
        tracing::warn!(field, "required value was empty");
        return Err(AppError::BadRequest(format!("{field} is required")));
    }

    Ok(value.to_string())
}

fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;

    Ok(password_hash.to_string())
}

fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let password_hash = PasswordHash::new(password_hash)?;

    match Argon2::default().verify_password(password.as_bytes(), &password_hash) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::Password) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

fn is_unique_constraint_error(err: &DbErr) -> bool {
    let message = err.to_string().to_lowercase();

    message.contains("duplicate key") || message.contains("unique constraint")
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
    use super::*;
    use crate::entities::users::Model as UserModel;
    use chrono::NaiveDate;
    use sea_orm::{DatabaseConnection, DbBackend, MockDatabase};

    const USERNAME: &str = "alice";
    const PHONE: &str = "13800138000";
    const PASSWORD: &str = "secret-password";

    #[tokio::test]
    async fn create_user_trims_input_hashes_password_and_returns_public_response() {
        let returned_user = user_model(USERNAME, PHONE, "stored-hash", UserStatus::Active);
        let db = mock_db(vec![
            Vec::<UserModel>::new(),
            Vec::<UserModel>::new(),
            vec![returned_user],
        ]);

        let response = UserService::create_user(
            &db,
            CreateUserRequest {
                username: " alice ".to_string(),
                phone: " 13800138000 ".to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await
        .expect("user should be created");

        assert_eq!(response.username, USERNAME);
        assert_eq!(response.phone, PHONE);
        assert_eq!(response.status, UserStatus::Active);

        let transaction_log = format!("{:?}", db.into_transaction_log());
        assert!(transaction_log.contains(USERNAME));
        assert!(transaction_log.contains(PHONE));
        assert!(!transaction_log.contains(PASSWORD));
    }

    #[tokio::test]
    async fn create_user_rejects_blank_username() {
        let db = mock_db(Vec::new());

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: " ".to_string(),
                phone: PHONE.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_bad_request(result, "username is required");
    }

    #[tokio::test]
    async fn create_user_rejects_blank_phone() {
        let db = mock_db(Vec::new());

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: USERNAME.to_string(),
                phone: " ".to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_bad_request(result, "phone is required");
    }

    #[tokio::test]
    async fn create_user_rejects_blank_password() {
        let db = mock_db(Vec::new());

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: USERNAME.to_string(),
                phone: PHONE.to_string(),
                password: " ".to_string(),
            },
        )
        .await;

        assert_bad_request(result, "password is required");
    }

    #[tokio::test]
    async fn create_user_rejects_overlong_username() {
        let db = mock_db(Vec::new());

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: "a".repeat(65),
                phone: PHONE.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_bad_request(result, "username must be at most 64 characters");
    }

    #[tokio::test]
    async fn create_user_rejects_overlong_phone() {
        let db = mock_db(Vec::new());

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: USERNAME.to_string(),
                phone: "1".repeat(21),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_bad_request(result, "phone must be at most 20 characters");
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_username() {
        let existing_user = user_model(USERNAME, PHONE, "stored-hash", UserStatus::Active);
        let db = mock_db(vec![vec![existing_user]]);

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: USERNAME.to_string(),
                phone: PHONE.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_conflict(result, "username already exists");
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_phone() {
        let existing_user = user_model(USERNAME, PHONE, "stored-hash", UserStatus::Active);
        let db = mock_db(vec![Vec::<UserModel>::new(), vec![existing_user]]);

        let result = UserService::create_user(
            &db,
            CreateUserRequest {
                username: USERNAME.to_string(),
                phone: PHONE.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_conflict(result, "phone already exists");
    }

    #[tokio::test]
    async fn get_user_returns_public_response() {
        let user = user_model(USERNAME, PHONE, "stored-hash", UserStatus::Active);
        let db = mock_db(vec![vec![user.clone()]]);

        let response = UserService::get_user(&db, user.id)
            .await
            .expect("user should exist");

        assert_eq!(response.id, user.id);
        assert_eq!(response.username, USERNAME);
        assert_eq!(response.phone, PHONE);
        assert_eq!(response.status, UserStatus::Active);
    }

    #[tokio::test]
    async fn get_user_returns_not_found_when_missing() {
        let db = mock_db(vec![Vec::<UserModel>::new()]);

        let result = UserService::get_user(&db, Uuid::from_u128(7)).await;

        assert_not_found(result, "user not found");
    }

    #[tokio::test]
    async fn login_user_accepts_username_with_correct_password() {
        let user = user_model(
            USERNAME,
            PHONE,
            &hash_password(PASSWORD).expect("hash should be created"),
            UserStatus::Active,
        );
        let db = mock_db(vec![vec![user.clone()]]);

        let response = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: USERNAME.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await
        .expect("login should succeed");

        assert_eq!(response.id, user.id);
        assert_eq!(response.username, USERNAME);
    }

    #[tokio::test]
    async fn login_user_accepts_phone_with_correct_password() {
        let user = user_model(
            USERNAME,
            PHONE,
            &hash_password(PASSWORD).expect("hash should be created"),
            UserStatus::Active,
        );
        let db = mock_db(vec![vec![user.clone()]]);

        let response = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: PHONE.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await
        .expect("login should succeed");

        assert_eq!(response.id, user.id);
        assert_eq!(response.phone, PHONE);
    }

    #[tokio::test]
    async fn login_user_rejects_unknown_identifier() {
        let db = mock_db(vec![Vec::<UserModel>::new()]);

        let result = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: USERNAME.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_unauthorized(result, "invalid login credentials");
    }

    #[tokio::test]
    async fn login_user_rejects_wrong_password() {
        let user = user_model(
            USERNAME,
            PHONE,
            &hash_password(PASSWORD).expect("hash should be created"),
            UserStatus::Active,
        );
        let db = mock_db(vec![vec![user]]);

        let result = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: USERNAME.to_string(),
                password: "wrong-password".to_string(),
            },
        )
        .await;

        assert_unauthorized(result, "invalid login credentials");
    }

    #[tokio::test]
    async fn login_user_rejects_disabled_user() {
        let user = user_model(
            USERNAME,
            PHONE,
            &hash_password(PASSWORD).expect("hash should be created"),
            UserStatus::Disabled,
        );
        let db = mock_db(vec![vec![user]]);

        let result = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: USERNAME.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_forbidden(result, "user is disabled");
    }

    #[tokio::test]
    async fn login_user_rejects_blank_identifier() {
        let db = mock_db(Vec::new());

        let result = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: " ".to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await;

        assert_bad_request(result, "identifier is required");
    }

    #[tokio::test]
    async fn login_user_rejects_blank_password() {
        let db = mock_db(Vec::new());

        let result = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: USERNAME.to_string(),
                password: " ".to_string(),
            },
        )
        .await;

        assert_bad_request(result, "password is required");
    }

    #[test]
    fn password_hash_does_not_store_plaintext_and_verifies_passwords() {
        let password_hash = hash_password(PASSWORD).expect("hash should be created");

        assert_ne!(password_hash, PASSWORD);
        assert!(verify_password(PASSWORD, &password_hash).expect("password should verify"));
        assert!(
            !verify_password("wrong-password", &password_hash)
                .expect("wrong password should not verify")
        );
    }

    fn mock_db(query_results: Vec<Vec<UserModel>>) -> DatabaseConnection {
        MockDatabase::new(DbBackend::Postgres)
            .append_query_results(query_results)
            .into_connection()
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

    fn assert_bad_request(result: Result<UserResponse, AppError>, expected: &str) {
        match result {
            Err(AppError::BadRequest(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected bad request, got {other:?}"),
            Ok(_) => panic!("expected bad request, got ok"),
        }
    }

    fn assert_conflict(result: Result<UserResponse, AppError>, expected: &str) {
        match result {
            Err(AppError::Conflict(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected conflict, got {other:?}"),
            Ok(_) => panic!("expected conflict, got ok"),
        }
    }

    fn assert_forbidden(result: Result<UserResponse, AppError>, expected: &str) {
        match result {
            Err(AppError::Forbidden(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected forbidden, got {other:?}"),
            Ok(_) => panic!("expected forbidden, got ok"),
        }
    }

    fn assert_not_found(result: Result<UserResponse, AppError>, expected: &str) {
        match result {
            Err(AppError::NotFound(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected not found, got {other:?}"),
            Ok(_) => panic!("expected not found, got ok"),
        }
    }

    fn assert_unauthorized(result: Result<UserResponse, AppError>, expected: &str) {
        match result {
            Err(AppError::Unauthorized(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected unauthorized, got {other:?}"),
            Ok(_) => panic!("expected unauthorized, got ok"),
        }
    }
}

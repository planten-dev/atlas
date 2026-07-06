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
    dto::users::{
        ChangeUserPasswordRequest, CreateUserRequest, ListUsersQuery, ListUsersResponse,
        LoginUserRequest, UpdateUserRequest, UpdateUserStatusRequest, UserResponse,
    },
    entities::users::{ActiveModel, Model as UserModel, UserStatus},
    errors::{AppError, AppResult},
    repositories::users::{UserListFilter, UserRepository, UserSortBy, UserSortOrder},
};

pub struct UserService;

const DEFAULT_PAGE: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 20;
const MAX_PAGE_SIZE: u64 = 100;

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
        name = "users.service.list_users",
        skip(db, query),
        fields(
            page = tracing::field::Empty,
            page_size = tracing::field::Empty,
            status_count = tracing::field::Empty,
            has_keyword = tracing::field::Empty,
            sort_by = tracing::field::Empty,
            sort_order = tracing::field::Empty
        )
    )]
    pub async fn list_users(
        db: &DatabaseConnection,
        query: ListUsersQuery,
    ) -> AppResult<ListUsersResponse> {
        let params = normalize_list_users_query(query)?;
        tracing::Span::current().record("page", params.page);
        tracing::Span::current().record("page_size", params.page_size);
        tracing::Span::current().record("status_count", params.filter.statuses.len());
        tracing::Span::current().record("has_keyword", params.filter.keyword.is_some());
        tracing::Span::current().record("sort_by", params.sort_by.as_str());
        tracing::Span::current().record("sort_order", params.sort_order.as_str());

        let offset = (params.page - 1) * params.page_size;
        let total = UserRepository::count(db, &params.filter).await?;
        let users = UserRepository::list(
            db,
            &params.filter,
            params.sort_by,
            params.sort_order,
            offset,
            params.page_size,
        )
        .await?;

        tracing::debug!(total, returned = users.len(), "user list prepared");

        Ok(ListUsersResponse {
            items: users.into_iter().map(Into::into).collect(),
            total,
            page: params.page,
            page_size: params.page_size,
        })
    }

    #[tracing::instrument(
        name = "users.service.update_user",
        skip(db, request),
        fields(user_id = %id, has_username = request.username.is_some(), has_phone = request.phone.is_some())
    )]
    pub async fn update_user(
        db: &DatabaseConnection,
        id: Uuid,
        request: UpdateUserRequest,
    ) -> AppResult<UserResponse> {
        let username = normalize_optional_required(request.username, "username")?;
        let phone = normalize_optional_required(request.phone, "phone")?;

        if username.is_none() && phone.is_none() {
            tracing::warn!("user update rejected because no fields were provided");
            return Err(AppError::BadRequest(
                "username or phone is required".to_string(),
            ));
        }

        validate_optional_username(&username)?;
        validate_optional_phone(&phone)?;

        let existing = load_user_by_id(db, id).await?;
        ensure_unique_username_for_update(db, &existing, username.as_deref()).await?;
        ensure_unique_phone_for_update(db, &existing, phone.as_deref()).await?;

        let updated =
            UserRepository::update_profile(db, id, username, phone, Utc::now().naive_utc()).await?;

        tracing::info!(user_id = %updated.id, "user profile updated");

        Ok(updated.into())
    }

    #[tracing::instrument(
        name = "users.service.update_user_status",
        skip(db, request),
        fields(user_id = %id, status = ?request.status)
    )]
    pub async fn update_user_status(
        db: &DatabaseConnection,
        id: Uuid,
        request: UpdateUserStatusRequest,
    ) -> AppResult<UserResponse> {
        load_user_by_id(db, id).await?;

        let updated =
            UserRepository::update_status(db, id, request.status, Utc::now().naive_utc()).await?;

        tracing::info!(user_id = %updated.id, status = ?updated.status, "user status updated");

        Ok(updated.into())
    }

    #[tracing::instrument(
        name = "users.service.change_user_password",
        skip(db, request),
        fields(user_id = %id)
    )]
    pub async fn change_user_password(
        db: &DatabaseConnection,
        id: Uuid,
        request: ChangeUserPasswordRequest,
    ) -> AppResult<()> {
        if request.current_password.trim().is_empty() {
            tracing::warn!(
                field = "current_password",
                "password change validation failed"
            );
            return Err(AppError::BadRequest(
                "current_password is required".to_string(),
            ));
        }

        if request.new_password.trim().is_empty() {
            tracing::warn!(field = "new_password", "password change validation failed");
            return Err(AppError::BadRequest("new_password is required".to_string()));
        }

        let user = load_user_by_id(db, id).await?;

        if user.status != UserStatus::Active {
            tracing::warn!(user_id = %user.id, status = ?user.status, "password change rejected because user is disabled");
            return Err(AppError::Forbidden("user is disabled".to_string()));
        }

        if !verify_password(&request.current_password, &user.password_hash)? {
            tracing::warn!(user_id = %user.id, "password change rejected because current password did not verify");
            return Err(AppError::Unauthorized(
                "invalid current password".to_string(),
            ));
        }

        let password_hash = hash_password(&request.new_password)?;
        UserRepository::update_password_hash(db, id, password_hash, Utc::now().naive_utc()).await?;

        tracing::info!(user_id = %id, "user password changed");

        Ok(())
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

struct NormalizedListUsersQuery {
    page: u64,
    page_size: u64,
    filter: UserListFilter,
    sort_by: UserSortBy,
    sort_order: UserSortOrder,
}

fn normalize_list_users_query(query: ListUsersQuery) -> AppResult<NormalizedListUsersQuery> {
    let page = query.page.unwrap_or(DEFAULT_PAGE);
    if page == 0 {
        return Err(AppError::BadRequest("page must be at least 1".to_string()));
    }

    let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(AppError::BadRequest(format!(
            "page_size must be between 1 and {MAX_PAGE_SIZE}"
        )));
    }

    let statuses = parse_user_statuses(query.status.as_deref())?;
    let keyword = query
        .keyword
        .map(|keyword| keyword.trim().to_string())
        .filter(|keyword| !keyword.is_empty());
    let sort_by = parse_sort_by(query.sort_by.as_deref().unwrap_or("created_at"))?;
    let sort_order = parse_sort_order(query.sort_order.as_deref().unwrap_or("desc"))?;

    Ok(NormalizedListUsersQuery {
        page,
        page_size,
        filter: UserListFilter { statuses, keyword },
        sort_by,
        sort_order,
    })
}

fn parse_user_statuses(status: Option<&str>) -> AppResult<Vec<UserStatus>> {
    let Some(status) = status else {
        return Ok(Vec::new());
    };

    let mut statuses = Vec::new();
    for item in status.split(',') {
        let parsed = parse_user_status(item.trim())?;
        if !statuses.contains(&parsed) {
            statuses.push(parsed);
        }
    }

    Ok(statuses)
}

fn parse_user_status(status: &str) -> AppResult<UserStatus> {
    match status {
        "active" => Ok(UserStatus::Active),
        "disabled" => Ok(UserStatus::Disabled),
        _ => Err(AppError::BadRequest(
            "status must be active or disabled".to_string(),
        )),
    }
}

fn parse_sort_by(sort_by: &str) -> AppResult<UserSortBy> {
    match sort_by {
        "created_at" => Ok(UserSortBy::CreatedAt),
        "updated_at" => Ok(UserSortBy::UpdatedAt),
        "username" => Ok(UserSortBy::Username),
        "phone" => Ok(UserSortBy::Phone),
        "status" => Ok(UserSortBy::Status),
        _ => Err(AppError::BadRequest(
            "sort_by must be one of created_at, updated_at, username, phone, status".to_string(),
        )),
    }
}

fn parse_sort_order(sort_order: &str) -> AppResult<UserSortOrder> {
    match sort_order {
        "asc" => Ok(UserSortOrder::Asc),
        "desc" => Ok(UserSortOrder::Desc),
        _ => Err(AppError::BadRequest(
            "sort_order must be asc or desc".to_string(),
        )),
    }
}

impl UserSortBy {
    fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::Username => "username",
            Self::Phone => "phone",
            Self::Status => "status",
        }
    }
}

impl UserSortOrder {
    fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
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

fn normalize_optional_required(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    value
        .map(|value| normalize_required(&value, field))
        .transpose()
}

fn validate_optional_username(username: &Option<String>) -> AppResult<()> {
    if let Some(username) = username {
        if username.chars().count() > 64 {
            tracing::warn!(
                field = "username",
                username_len = username.chars().count(),
                "user update validation failed"
            );
            return Err(AppError::BadRequest(
                "username must be at most 64 characters".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_optional_phone(phone: &Option<String>) -> AppResult<()> {
    if let Some(phone) = phone {
        if phone.chars().count() > 20 {
            tracing::warn!(
                field = "phone",
                phone_len = phone.chars().count(),
                "user update validation failed"
            );
            return Err(AppError::BadRequest(
                "phone must be at most 20 characters".to_string(),
            ));
        }
    }

    Ok(())
}

async fn load_user_by_id(db: &DatabaseConnection, id: Uuid) -> AppResult<UserModel> {
    UserRepository::find_by_id(db, id).await?.ok_or_else(|| {
        tracing::warn!(user_id = %id, "user not found");
        AppError::NotFound("user not found".to_string())
    })
}

async fn ensure_unique_username_for_update(
    db: &DatabaseConnection,
    existing: &UserModel,
    username: Option<&str>,
) -> AppResult<()> {
    let Some(username) = username else {
        return Ok(());
    };

    if username == existing.username {
        return Ok(());
    }

    if UserRepository::find_by_username(db, username)
        .await?
        .is_some()
    {
        tracing::warn!(
            user_id = %existing.id,
            "user update rejected because username already exists"
        );
        return Err(AppError::Conflict("username already exists".to_string()));
    }

    Ok(())
}

async fn ensure_unique_phone_for_update(
    db: &DatabaseConnection,
    existing: &UserModel,
    phone: Option<&str>,
) -> AppResult<()> {
    let Some(phone) = phone else {
        return Ok(());
    };

    if phone == existing.phone {
        return Ok(());
    }

    if UserRepository::find_by_phone(db, phone).await?.is_some() {
        tracing::warn!(
            user_id = %existing.id,
            "user update rejected because phone already exists"
        );
        return Err(AppError::Conflict("phone already exists".to_string()));
    }

    Ok(())
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
    use crate::{
        cli::DatabaseKind,
        database::{DatabaseSettings, connect},
    };
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

    async fn sqlite_db() -> DatabaseConnection {
        let settings = DatabaseSettings::from_parts(DatabaseKind::Sqlite, None)
            .expect("sqlite settings should resolve");

        connect(&settings)
            .await
            .expect("sqlite database should connect")
    }

    async fn create_sqlite_user(
        db: &DatabaseConnection,
        username: &str,
        phone: &str,
    ) -> UserResponse {
        UserService::create_user(
            db,
            CreateUserRequest {
                username: username.to_string(),
                phone: phone.to_string(),
                password: PASSWORD.to_string(),
            },
        )
        .await
        .expect("user should be created")
    }

    #[tokio::test]
    async fn list_users_uses_default_pagination() {
        let db = sqlite_db().await;
        create_sqlite_user(&db, "alice", "13800138000").await;
        create_sqlite_user(&db, "bob", "13800138001").await;

        let response = UserService::list_users(
            &db,
            ListUsersQuery {
                page: None,
                page_size: None,
                status: None,
                keyword: None,
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .expect("users should be listed");

        assert_eq!(response.total, 2);
        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 20);
        assert_eq!(response.items.len(), 2);
    }

    #[tokio::test]
    async fn list_users_filters_by_status_and_keyword() {
        let db = sqlite_db().await;
        create_sqlite_user(&db, "alice", "13800138000").await;
        let disabled_user = create_sqlite_user(&db, "carol", "13800138002").await;
        UserService::update_user_status(
            &db,
            disabled_user.id,
            UpdateUserStatusRequest {
                status: UserStatus::Disabled,
            },
        )
        .await
        .expect("user should be disabled");

        let response = UserService::list_users(
            &db,
            ListUsersQuery {
                page: Some(1),
                page_size: Some(10),
                status: Some("disabled".to_string()),
                keyword: Some("car".to_string()),
                sort_by: Some("username".to_string()),
                sort_order: Some("asc".to_string()),
            },
        )
        .await
        .expect("users should be filtered");

        assert_eq!(response.total, 1);
        assert_eq!(response.items[0].username, "carol");
        assert_eq!(response.items[0].status, UserStatus::Disabled);
    }

    #[tokio::test]
    async fn list_users_filters_by_multiple_statuses() {
        let db = sqlite_db().await;
        create_sqlite_user(&db, "alice", "13800138000").await;
        let disabled_user = create_sqlite_user(&db, "carol", "13800138002").await;
        UserService::update_user_status(
            &db,
            disabled_user.id,
            UpdateUserStatusRequest {
                status: UserStatus::Disabled,
            },
        )
        .await
        .expect("user should be disabled");

        let response = UserService::list_users(
            &db,
            ListUsersQuery {
                page: Some(1),
                page_size: Some(10),
                status: Some("active,disabled".to_string()),
                keyword: None,
                sort_by: Some("username".to_string()),
                sort_order: Some("asc".to_string()),
            },
        )
        .await
        .expect("users should be filtered by multiple statuses");

        assert_eq!(response.total, 2);
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].username, "alice");
        assert_eq!(response.items[0].status, UserStatus::Active);
        assert_eq!(response.items[1].username, "carol");
        assert_eq!(response.items[1].status, UserStatus::Disabled);
    }

    #[tokio::test]
    async fn list_users_filters_keyword_by_phone_and_sorts() {
        let db = sqlite_db().await;
        create_sqlite_user(&db, "charlie", "13800138003").await;
        create_sqlite_user(&db, "alice", "13900139000").await;

        let response = UserService::list_users(
            &db,
            ListUsersQuery {
                page: Some(1),
                page_size: Some(10),
                status: None,
                keyword: Some("138001".to_string()),
                sort_by: Some("username".to_string()),
                sort_order: Some("asc".to_string()),
            },
        )
        .await
        .expect("users should be filtered by phone");

        assert_eq!(response.total, 1);
        assert_eq!(response.items[0].username, "charlie");
    }

    #[tokio::test]
    async fn list_users_returns_empty_page() {
        let db = sqlite_db().await;

        let response = UserService::list_users(
            &db,
            ListUsersQuery {
                page: Some(1),
                page_size: Some(10),
                status: None,
                keyword: Some("missing".to_string()),
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .expect("empty list should be returned");

        assert_eq!(response.total, 0);
        assert!(response.items.is_empty());
    }

    #[tokio::test]
    async fn list_users_rejects_invalid_parameters() {
        let db = sqlite_db().await;

        assert_bad_request(
            UserService::list_users(
                &db,
                ListUsersQuery {
                    page: Some(0),
                    page_size: None,
                    status: None,
                    keyword: None,
                    sort_by: None,
                    sort_order: None,
                },
            )
            .await,
            "page must be at least 1",
        );

        assert_bad_request(
            UserService::list_users(
                &db,
                ListUsersQuery {
                    page: None,
                    page_size: Some(101),
                    status: None,
                    keyword: None,
                    sort_by: None,
                    sort_order: None,
                },
            )
            .await,
            "page_size must be between 1 and 100",
        );

        assert_bad_request(
            UserService::list_users(
                &db,
                ListUsersQuery {
                    page: None,
                    page_size: None,
                    status: Some("locked".to_string()),
                    keyword: None,
                    sort_by: None,
                    sort_order: None,
                },
            )
            .await,
            "status must be active or disabled",
        );

        assert_bad_request(
            UserService::list_users(
                &db,
                ListUsersQuery {
                    page: None,
                    page_size: None,
                    status: Some("active,locked".to_string()),
                    keyword: None,
                    sort_by: None,
                    sort_order: None,
                },
            )
            .await,
            "status must be active or disabled",
        );

        assert_bad_request(
            UserService::list_users(
                &db,
                ListUsersQuery {
                    page: None,
                    page_size: None,
                    status: None,
                    keyword: None,
                    sort_by: Some("id".to_string()),
                    sort_order: None,
                },
            )
            .await,
            "sort_by must be one of created_at, updated_at, username, phone, status",
        );

        assert_bad_request(
            UserService::list_users(
                &db,
                ListUsersQuery {
                    page: None,
                    page_size: None,
                    status: None,
                    keyword: None,
                    sort_by: None,
                    sort_order: Some("sideways".to_string()),
                },
            )
            .await,
            "sort_order must be asc or desc",
        );
    }

    #[tokio::test]
    async fn update_user_updates_profile_and_preserves_id() {
        let db = sqlite_db().await;
        let created = create_sqlite_user(&db, "alice", "13800138000").await;

        let updated = UserService::update_user(
            &db,
            created.id,
            UpdateUserRequest {
                username: Some("alice2".to_string()),
                phone: Some("13800138009".to_string()),
            },
        )
        .await
        .expect("user should be updated");

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.username, "alice2");
        assert_eq!(updated.phone, "13800138009");
    }

    #[tokio::test]
    async fn update_user_rejects_duplicate_username_and_phone() {
        let db = sqlite_db().await;
        let first = create_sqlite_user(&db, "alice", "13800138000").await;
        let second = create_sqlite_user(&db, "bob", "13800138001").await;

        assert_conflict(
            UserService::update_user(
                &db,
                second.id,
                UpdateUserRequest {
                    username: Some(first.username),
                    phone: None,
                },
            )
            .await,
            "username already exists",
        );

        assert_conflict(
            UserService::update_user(
                &db,
                second.id,
                UpdateUserRequest {
                    username: None,
                    phone: Some(first.phone),
                },
            )
            .await,
            "phone already exists",
        );
    }

    #[tokio::test]
    async fn update_user_rejects_empty_request_overlong_fields_and_missing_user() {
        let db = sqlite_db().await;
        let created = create_sqlite_user(&db, "alice", "13800138000").await;

        assert_bad_request(
            UserService::update_user(
                &db,
                created.id,
                UpdateUserRequest {
                    username: None,
                    phone: None,
                },
            )
            .await,
            "username or phone is required",
        );

        assert_bad_request(
            UserService::update_user(
                &db,
                created.id,
                UpdateUserRequest {
                    username: Some("a".repeat(65)),
                    phone: None,
                },
            )
            .await,
            "username must be at most 64 characters",
        );

        assert_bad_request(
            UserService::update_user(
                &db,
                created.id,
                UpdateUserRequest {
                    username: None,
                    phone: Some("1".repeat(21)),
                },
            )
            .await,
            "phone must be at most 20 characters",
        );

        assert_not_found(
            UserService::update_user(
                &db,
                Uuid::from_u128(99),
                UpdateUserRequest {
                    username: Some("missing".to_string()),
                    phone: None,
                },
            )
            .await,
            "user not found",
        );
    }

    #[tokio::test]
    async fn update_user_status_enables_and_disables_user() {
        let db = sqlite_db().await;
        let created = create_sqlite_user(&db, "alice", "13800138000").await;

        let disabled = UserService::update_user_status(
            &db,
            created.id,
            UpdateUserStatusRequest {
                status: UserStatus::Disabled,
            },
        )
        .await
        .expect("user should be disabled");
        assert_eq!(disabled.status, UserStatus::Disabled);

        let active = UserService::update_user_status(
            &db,
            created.id,
            UpdateUserStatusRequest {
                status: UserStatus::Active,
            },
        )
        .await
        .expect("user should be active");
        assert_eq!(active.status, UserStatus::Active);
        assert_eq!(active.id, created.id);
    }

    #[tokio::test]
    async fn update_user_status_rejects_missing_user() {
        let db = sqlite_db().await;

        assert_not_found(
            UserService::update_user_status(
                &db,
                Uuid::from_u128(99),
                UpdateUserStatusRequest {
                    status: UserStatus::Disabled,
                },
            )
            .await,
            "user not found",
        );
    }

    #[tokio::test]
    async fn change_user_password_updates_password_hash() {
        let db = sqlite_db().await;
        let created = create_sqlite_user(&db, "alice", "13800138000").await;

        UserService::change_user_password(
            &db,
            created.id,
            ChangeUserPasswordRequest {
                current_password: PASSWORD.to_string(),
                new_password: "new-secret-password".to_string(),
            },
        )
        .await
        .expect("password should be changed");

        assert_unauthorized(
            UserService::login_user(
                &db,
                LoginUserRequest {
                    identifier: "alice".to_string(),
                    password: PASSWORD.to_string(),
                },
            )
            .await,
            "invalid login credentials",
        );

        let logged_in = UserService::login_user(
            &db,
            LoginUserRequest {
                identifier: "alice".to_string(),
                password: "new-secret-password".to_string(),
            },
        )
        .await
        .expect("new password should work");
        assert_eq!(logged_in.id, created.id);
    }

    #[tokio::test]
    async fn change_user_password_rejects_invalid_inputs_and_states() {
        let db = sqlite_db().await;
        let created = create_sqlite_user(&db, "alice", "13800138000").await;

        assert_bad_request(
            UserService::change_user_password(
                &db,
                created.id,
                ChangeUserPasswordRequest {
                    current_password: PASSWORD.to_string(),
                    new_password: " ".to_string(),
                },
            )
            .await,
            "new_password is required",
        );

        assert_unauthorized(
            UserService::change_user_password(
                &db,
                created.id,
                ChangeUserPasswordRequest {
                    current_password: "wrong-password".to_string(),
                    new_password: "new-secret-password".to_string(),
                },
            )
            .await,
            "invalid current password",
        );

        UserService::update_user_status(
            &db,
            created.id,
            UpdateUserStatusRequest {
                status: UserStatus::Disabled,
            },
        )
        .await
        .expect("user should be disabled");

        assert_forbidden(
            UserService::change_user_password(
                &db,
                created.id,
                ChangeUserPasswordRequest {
                    current_password: PASSWORD.to_string(),
                    new_password: "new-secret-password".to_string(),
                },
            )
            .await,
            "user is disabled",
        );

        assert_not_found(
            UserService::change_user_password(
                &db,
                Uuid::from_u128(99),
                ChangeUserPasswordRequest {
                    current_password: PASSWORD.to_string(),
                    new_password: "new-secret-password".to_string(),
                },
            )
            .await,
            "user not found",
        );
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

    fn assert_bad_request<T>(result: Result<T, AppError>, expected: &str) {
        match result {
            Err(AppError::BadRequest(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected bad request, got {other:?}"),
            Ok(_) => panic!("expected bad request, got ok"),
        }
    }

    fn assert_conflict<T>(result: Result<T, AppError>, expected: &str) {
        match result {
            Err(AppError::Conflict(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected conflict, got {other:?}"),
            Ok(_) => panic!("expected conflict, got ok"),
        }
    }

    fn assert_forbidden<T>(result: Result<T, AppError>, expected: &str) {
        match result {
            Err(AppError::Forbidden(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected forbidden, got {other:?}"),
            Ok(_) => panic!("expected forbidden, got ok"),
        }
    }

    fn assert_not_found<T>(result: Result<T, AppError>, expected: &str) {
        match result {
            Err(AppError::NotFound(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected not found, got {other:?}"),
            Ok(_) => panic!("expected not found, got ok"),
        }
    }

    fn assert_unauthorized<T>(result: Result<T, AppError>, expected: &str) {
        match result {
            Err(AppError::Unauthorized(message)) => assert_eq!(message, expected),
            Err(other) => panic!("expected unauthorized, got {other:?}"),
            Ok(_) => panic!("expected unauthorized, got ok"),
        }
    }
}

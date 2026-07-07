use chrono::Utc;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::users::{
        ListUsersQuery, ListUsersResponse, UpdateUserStatusRequest, UserResponse, UserStatus,
        UserStatusParseError,
    },
    repositories::{RepositoryError, sessions::SessionRepository, users::UserRepository},
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

#[derive(Clone)]
pub struct UserService {
    users: UserRepository,
    sessions: SessionRepository,
}

impl UserService {
    pub fn new(users: UserRepository, sessions: SessionRepository) -> Self {
        Self { users, sessions }
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_users(&self, query: ListUsersQuery) -> Result<ListUsersResponse, UserError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| UserStatus::parse("status_filter", value))
            .transpose()?;
        let (users, total_count) = self
            .users
            .list_users(
                status_filter.map(UserStatus::as_str),
                page_number,
                page_size,
            )
            .await?;

        debug!(
            count = users.len(),
            total_count, page_number, page_size, "listed users through service"
        );
        Ok(ListUsersResponse {
            users: users.into_iter().map(UserResponse::from).collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn user_detail(&self, user_id: Uuid) -> Result<UserResponse, UserError> {
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .ok_or(UserError::UserNotFound)?;

        debug!(%user_id, "loaded user detail");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_status(
        &self,
        user_id: Uuid,
        request: UpdateUserStatusRequest,
    ) -> Result<UserResponse, UserError> {
        let target_status = UserStatus::parse("target_status", &request.target_status)?;
        let now = Utc::now();
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .ok_or(UserError::UserNotFound)?;

        let user = self
            .users
            .update_status(&user, target_status.as_str(), now)
            .await?;

        if target_status == UserStatus::Disabled {
            let revoked_count = self
                .sessions
                .revoke_active_sessions_for_user(user_id, now)
                .await?;
            info!(
                %user_id,
                revoked_count,
                "disabled user and revoked active sessions"
            );
        } else {
            info!(%user_id, "activated user without creating sessions");
        }

        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_user(&self, user_id: Uuid) -> Result<(), UserError> {
        if self.users.find_by_id(user_id).await?.is_none() {
            return Err(UserError::UserNotFound);
        }

        let deleted_session_count = self.sessions.delete_sessions_for_user(user_id).await?;
        let deleted = self.users.delete_by_id(user_id).await?;
        if !deleted {
            warn!(%user_id, "user disappeared before delete completed");
            return Err(UserError::UserNotFound);
        }

        info!(
            %user_id,
            deleted_session_count,
            "deleted user record and auth sessions"
        );
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("user was not found")]
    UserNotFound,
    #[error("{field} must be one of: active, disabled")]
    InvalidStatus { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidPaginationMinimum { field: &'static str, minimum: u64 },
    #[error("{field} must be less than or equal to {maximum}")]
    InvalidPaginationMaximum { field: &'static str, maximum: u64 },
}

impl UserError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::UserNotFound => "user_not_found",
            Self::InvalidStatus { .. }
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
        }
    }
}

impl From<UserStatusParseError> for UserError {
    fn from(error: UserStatusParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            "rejected invalid user status"
        );
        Self::InvalidStatus {
            field: error.field,
            value: error.value,
        }
    }
}

fn validate_page_number(page_number: u64) -> Result<(), UserError> {
    if page_number == 0 {
        return Err(UserError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), UserError> {
    if page_size == 0 {
        return Err(UserError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(UserError::InvalidPaginationMaximum {
            field: "page_size",
            maximum: MAX_PAGE_SIZE,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        repositories::sessions::hash_secret,
    };
    use chrono::{Duration, TimeZone};
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_services() -> (UserRepository, SessionRepository, UserService) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let users = UserRepository::new(db.clone());
        let sessions = SessionRepository::new(db);
        let service = UserService::new(users.clone(), sessions.clone());
        (users, sessions, service)
    }

    #[tokio::test]
    async fn lists_users_with_status_filter_and_pagination_defaults() {
        let (users, _, service) = test_services().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        users
            .find_or_create_for_login("active-user", now)
            .await
            .expect("active user should be created");
        let disabled = users
            .find_or_create_for_login("disabled-user", now)
            .await
            .expect("disabled user should be created");
        users
            .update_status(&disabled, "disabled", now)
            .await
            .expect("user should be disabled");

        let response = service
            .list_users(ListUsersQuery {
                status_filter: Some("disabled".to_string()),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("users should be listed");

        assert_eq!(response.page_number, DEFAULT_PAGE_NUMBER);
        assert_eq!(response.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(response.total_count, 1);
        assert_eq!(response.users[0].dingtalk_user_id, "disabled-user");
    }

    #[tokio::test]
    async fn rejects_invalid_list_parameters() {
        let (_, _, service) = test_services().await;

        assert!(matches!(
            service
                .list_users(ListUsersQuery {
                    status_filter: Some("deleted".to_string()),
                    page_number: None,
                    page_size: None,
                })
                .await,
            Err(UserError::InvalidStatus {
                field: "status_filter",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_users(ListUsersQuery {
                    status_filter: None,
                    page_number: Some(0),
                    page_size: None,
                })
                .await,
            Err(UserError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_users(ListUsersQuery {
                    status_filter: None,
                    page_number: None,
                    page_size: Some(MAX_PAGE_SIZE + 1),
                })
                .await,
            Err(UserError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn updates_status_without_changing_dingtalk_mapping() {
        let (users, _, service) = test_services().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");

        let updated = service
            .update_status(
                user.id,
                UpdateUserStatusRequest {
                    target_status: "disabled".to_string(),
                },
            )
            .await
            .expect("status should be updated");

        assert_eq!(updated.status, "disabled");
        assert_eq!(updated.dingtalk_user_id, "ding-user-1");
    }

    #[tokio::test]
    async fn disabling_user_revokes_active_sessions() {
        let (users, sessions, service) = test_services().await;
        let now = Utc::now();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let token_hash = hash_secret("raw-session-token");
        sessions
            .create_session(user.id, &token_hash, now, now + Duration::hours(1))
            .await
            .expect("session should be created");

        service
            .update_status(
                user.id,
                UpdateUserStatusRequest {
                    target_status: "disabled".to_string(),
                },
            )
            .await
            .expect("user should be disabled");

        assert!(
            sessions
                .find_valid_session(&token_hash, now + Duration::minutes(1))
                .await
                .expect("session lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn activating_user_does_not_create_session() {
        let (users, sessions, service) = test_services().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let disabled = users
            .update_status(&user, "disabled", now)
            .await
            .expect("user should be disabled");

        let updated = service
            .update_status(
                disabled.id,
                UpdateUserStatusRequest {
                    target_status: "active".to_string(),
                },
            )
            .await
            .expect("user should be activated");

        assert_eq!(updated.status, "active");
        assert_eq!(
            sessions
                .revoke_active_sessions_for_user(disabled.id, now + Duration::minutes(1))
                .await
                .expect("session revoke should succeed"),
            0
        );
    }

    #[tokio::test]
    async fn deletes_user_and_existing_sessions() {
        let (users, sessions, service) = test_services().await;
        let now = Utc::now();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let token_hash = hash_secret("raw-session-token");
        sessions
            .create_session(user.id, &token_hash, now, now + Duration::hours(1))
            .await
            .expect("session should be created");

        service
            .delete_user(user.id)
            .await
            .expect("user should be deleted");

        assert!(
            users
                .find_by_id(user.id)
                .await
                .expect("user lookup should succeed")
                .is_none()
        );
        assert!(
            sessions
                .find_valid_session(&token_hash, now + Duration::minutes(1))
                .await
                .expect("session lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn reports_missing_user_and_invalid_target_status() {
        let (_, _, service) = test_services().await;

        assert!(matches!(
            service.user_detail(Uuid::new_v4()).await,
            Err(UserError::UserNotFound)
        ));
        assert!(matches!(
            service
                .update_status(
                    Uuid::new_v4(),
                    UpdateUserStatusRequest {
                        target_status: "deleted".to_string(),
                    },
                )
                .await,
            Err(UserError::InvalidStatus {
                field: "target_status",
                ..
            })
        ));
        assert!(matches!(
            service.delete_user(Uuid::new_v4()).await,
            Err(UserError::UserNotFound)
        ));
    }
}

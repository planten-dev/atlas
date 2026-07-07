use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{entities::users, repositories::RepositoryError};

#[derive(Clone)]
pub struct UserRepository {
    pub(crate) db: DatabaseConnection,
}

impl UserRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "debug", skip(self), fields(dingtalk_user_id = %dingtalk_user_id))]
    pub async fn find_by_dingtalk_user_id(
        &self,
        dingtalk_user_id: &str,
    ) -> Result<Option<users::Model>, RepositoryError> {
        validate_required("dingtalk_user_id", dingtalk_user_id)?;

        let user = users::Entity::find()
            .filter(users::Column::DingtalkUserId.eq(dingtalk_user_id.trim()))
            .one(&self.db)
            .await?;

        debug!(found = user.is_some(), "looked up user by DingTalk id");
        Ok(user)
    }

    #[tracing::instrument(level = "info", skip(self), fields(dingtalk_user_id = %dingtalk_user_id))]
    pub async fn find_or_create_for_login(
        &self,
        dingtalk_user_id: &str,
        now: DateTime<Utc>,
    ) -> Result<users::Model, RepositoryError> {
        validate_required("dingtalk_user_id", dingtalk_user_id)?;
        let dingtalk_user_id = dingtalk_user_id.trim();

        if let Some(user) = self.find_by_dingtalk_user_id(dingtalk_user_id).await? {
            if user.status == "disabled" {
                warn!(user_id = %user.id, "blocked login for disabled user");
                return Err(RepositoryError::DisabledUser);
            }

            let user = self.touch_login(&user, now).await?;
            info!(user_id = %user.id, "reused existing user for login");
            return Ok(user);
        }

        let user = users::ActiveModel {
            id: Set(Uuid::new_v4()),
            dingtalk_user_id: Set(dingtalk_user_id.to_string()),
            status: Set("active".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            last_login_at: Set(Some(now)),
        }
        .insert(&self.db)
        .await?;

        info!(user_id = %user.id, "created user for first DingTalk login");
        Ok(user)
    }

    #[tracing::instrument(level = "debug", skip(self, user), fields(user_id = %user.id))]
    pub async fn touch_login(
        &self,
        user: &users::Model,
        now: DateTime<Utc>,
    ) -> Result<users::Model, RepositoryError> {
        let mut active: users::ActiveModel = user.clone().into();
        active.updated_at = Set(now);
        active.last_login_at = Set(Some(now));
        let user = active.update(&self.db).await?;
        debug!("updated user login timestamp");
        Ok(user)
    }
}

fn validate_required(field: &'static str, value: &str) -> Result<(), RepositoryError> {
    if value.trim().is_empty() {
        return Err(RepositoryError::MissingRequiredField { field });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
    };
    use chrono::TimeZone;
    use sea_orm::{ActiveModelTrait, Set};
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_repository() -> UserRepository {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        UserRepository::new(db)
    }

    #[tokio::test]
    async fn creates_user_on_first_login() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        let user = repository
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");

        assert_eq!(user.dingtalk_user_id, "ding-user-1");
        assert_eq!(user.status, "active");
        assert_eq!(user.last_login_at, Some(now));
    }

    #[tokio::test]
    async fn reuses_existing_user_and_updates_login_time() {
        let repository = test_repository().await;
        let first_login = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let second_login = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();

        let first = repository
            .find_or_create_for_login("ding-user-1", first_login)
            .await
            .expect("user should be created");
        let second = repository
            .find_or_create_for_login("ding-user-1", second_login)
            .await
            .expect("user should be reused");

        assert_eq!(first.id, second.id);
        assert_eq!(second.last_login_at, Some(second_login));
    }

    #[tokio::test]
    async fn blocks_disabled_user_login() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = repository
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let mut active: users::ActiveModel = user.into();
        active.status = Set("disabled".to_string());
        active
            .update(&repository.db)
            .await
            .expect("user should be disabled");

        let result = repository
            .find_or_create_for_login("ding-user-1", now)
            .await;

        assert!(matches!(result, Err(RepositoryError::DisabledUser)));
    }
}

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    entities::{roles, user_roles, users},
    repositories::{RepositoryError, authz::SUPER_ADMIN_ROLE_CODE},
};

#[derive(Clone)]
pub struct UserRepository {
    pub(crate) db: DatabaseConnection,
    super_admin_bootstrap_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct LoginUser {
    pub user: users::Model,
    pub created: bool,
    pub assigned_super_admin: bool,
}

impl UserRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            super_admin_bootstrap_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(&self, user_id: Uuid) -> Result<Option<users::Model>, RepositoryError> {
        let user = users::Entity::find_by_id(user_id).one(&self.db).await?;

        debug!(found = user.is_some(), %user_id, "looked up user by id");
        Ok(user)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_users(
        &self,
        status_filter: Option<&str>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<users::Model>, u64), RepositoryError> {
        let mut query = users::Entity::find().order_by_asc(users::Column::CreatedAt);

        if let Some(status_filter) = status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(users::Column::Status.eq(status_filter.trim()));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let users = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = users.len(),
            total_count, page_number, page_size, "listed users"
        );
        Ok((users, total_count))
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
        Ok(self
            .find_or_create_login_user(dingtalk_user_id, now)
            .await?
            .user)
    }

    #[tracing::instrument(level = "info", skip(self), fields(dingtalk_user_id = %dingtalk_user_id))]
    pub async fn find_or_create_login_user(
        &self,
        dingtalk_user_id: &str,
        now: DateTime<Utc>,
    ) -> Result<LoginUser, RepositoryError> {
        validate_required("dingtalk_user_id", dingtalk_user_id)?;
        let dingtalk_user_id = dingtalk_user_id.trim();

        if let Some(user) = self.find_by_dingtalk_user_id(dingtalk_user_id).await? {
            if user.status == "disabled" {
                warn!(user_id = %user.id, "blocked login for disabled user");
                return Err(RepositoryError::DisabledUser);
            }

            let user = self.touch_login(&user, now).await?;
            info!(user_id = %user.id, "reused existing user for login");
            return Ok(LoginUser {
                user,
                created: false,
                assigned_super_admin: false,
            });
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
        Ok(LoginUser {
            user,
            created: true,
            assigned_super_admin: false,
        })
    }

    #[tracing::instrument(level = "info", skip(self), fields(dingtalk_user_id = %dingtalk_user_id))]
    pub async fn find_or_create_for_login_with_super_admin_bootstrap(
        &self,
        dingtalk_user_id: &str,
        now: DateTime<Utc>,
    ) -> Result<LoginUser, RepositoryError> {
        validate_required("dingtalk_user_id", dingtalk_user_id)?;
        let dingtalk_user_id = dingtalk_user_id.trim();

        let _guard = self.super_admin_bootstrap_lock.lock().await;
        let tx = self.db.begin().await?;
        let super_admin_role = roles::Entity::find()
            .filter(roles::Column::Code.eq(SUPER_ADMIN_ROLE_CODE))
            .lock_exclusive()
            .one(&tx)
            .await?
            .ok_or_else(|| {
                RepositoryError::Database(sea_orm::DbErr::Custom(
                    "super_admin role seed is missing".to_string(),
                ))
            })?;

        if let Some(user) = users::Entity::find()
            .filter(users::Column::DingtalkUserId.eq(dingtalk_user_id))
            .one(&tx)
            .await?
        {
            if user.status == "disabled" {
                tx.rollback().await?;
                warn!(user_id = %user.id, "blocked login for disabled user");
                return Err(RepositoryError::DisabledUser);
            }

            let user = touch_login_in(&tx, &user, now).await?;
            tx.commit().await?;
            info!(user_id = %user.id, "reused existing user for login");
            return Ok(LoginUser {
                user,
                created: false,
                assigned_super_admin: false,
            });
        }

        let existing_user_count = users::Entity::find().count(&tx).await?;
        let user = users::ActiveModel {
            id: Set(Uuid::new_v4()),
            dingtalk_user_id: Set(dingtalk_user_id.to_string()),
            status: Set("active".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
            last_login_at: Set(Some(now)),
        }
        .insert(&tx)
        .await?;

        let assigned_super_admin = existing_user_count == 0;
        if assigned_super_admin {
            user_roles::ActiveModel {
                user_id: Set(user.id),
                role_id: Set(super_admin_role.id),
                created_at: Set(now),
            }
            .insert(&tx)
            .await?;
        }

        tx.commit().await?;

        info!(
            user_id = %user.id,
            assigned_super_admin,
            "created user for DingTalk login"
        );
        Ok(LoginUser {
            user,
            created: true,
            assigned_super_admin,
        })
    }

    #[tracing::instrument(level = "debug", skip(self, user), fields(user_id = %user.id))]
    pub async fn touch_login(
        &self,
        user: &users::Model,
        now: DateTime<Utc>,
    ) -> Result<users::Model, RepositoryError> {
        let user = touch_login_in(&self.db, user, now).await?;
        debug!("updated user login timestamp");
        Ok(user)
    }

    #[tracing::instrument(level = "info", skip(self), fields(user_id = %user.id, status = %status))]
    pub async fn update_status(
        &self,
        user: &users::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<users::Model, RepositoryError> {
        validate_required("status", status)?;

        let mut active: users::ActiveModel = user.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let user = active.update(&self.db).await?;

        info!(user_id = %user.id, status = %user.status, "updated user status");
        Ok(user)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_by_id(&self, user_id: Uuid) -> Result<bool, RepositoryError> {
        let result = users::Entity::delete_by_id(user_id).exec(&self.db).await?;
        let deleted = result.rows_affected > 0;

        info!(%user_id, deleted, "deleted user by id");
        Ok(deleted)
    }
}

async fn touch_login_in<C>(
    conn: &C,
    user: &users::Model,
    now: DateTime<Utc>,
) -> Result<users::Model, RepositoryError>
where
    C: sea_orm::ConnectionTrait,
{
    let mut active: users::ActiveModel = user.clone().into();
    active.updated_at = Set(now);
    active.last_login_at = Set(Some(now));
    Ok(active.update(conn).await?)
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
        repositories::authz::{AuthzRepository, SUPER_ADMIN_ROLE_CODE},
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
    async fn login_user_reports_whether_user_was_created() {
        let repository = test_repository().await;
        let first_login = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let second_login = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();

        let first = repository
            .find_or_create_login_user("ding-user-1", first_login)
            .await
            .expect("user should be created");
        let second = repository
            .find_or_create_login_user("ding-user-1", second_login)
            .await
            .expect("user should be reused");

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.user.id, second.user.id);
    }

    #[tokio::test]
    async fn finds_user_by_id() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = repository
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");

        let found = repository
            .find_by_id(user.id)
            .await
            .expect("user lookup should succeed")
            .expect("user should be found");

        assert_eq!(found.id, user.id);
        assert!(
            repository
                .find_by_id(Uuid::new_v4())
                .await
                .expect("missing user lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn lists_users_with_status_filter_and_pagination() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        repository
            .find_or_create_for_login("active-user", now)
            .await
            .expect("active user should be created");
        let disabled = repository
            .find_or_create_for_login("disabled-user", now)
            .await
            .expect("disabled user should be created");
        repository
            .update_status(&disabled, "disabled", now)
            .await
            .expect("user should be disabled");

        let (users, total_count) = repository
            .list_users(Some("disabled"), 1, 50)
            .await
            .expect("users should be listed");

        assert_eq!(total_count, 1);
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].dingtalk_user_id, "disabled-user");
    }

    #[tokio::test]
    async fn updates_user_status() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = repository
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();

        let updated = repository
            .update_status(&user, "disabled", updated_at)
            .await
            .expect("status should be updated");

        assert_eq!(updated.id, user.id);
        assert_eq!(updated.dingtalk_user_id, "ding-user-1");
        assert_eq!(updated.status, "disabled");
        assert_eq!(updated.updated_at, updated_at);
    }

    #[tokio::test]
    async fn deletes_user_by_id() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = repository
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");

        let deleted = repository
            .delete_by_id(user.id)
            .await
            .expect("user delete should succeed");

        assert!(deleted);
        assert!(
            repository
                .find_by_id(user.id)
                .await
                .expect("user lookup should succeed")
                .is_none()
        );
        assert!(
            !repository
                .delete_by_id(Uuid::new_v4())
                .await
                .expect("missing user delete should succeed")
        );
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

    #[tokio::test]
    async fn first_bootstrapped_login_gets_super_admin_role() {
        let repository = test_repository().await;
        let authz = AuthzRepository::new(repository.db.clone());
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        let first = repository
            .find_or_create_for_login_with_super_admin_bootstrap("ding-user-1", now)
            .await
            .expect("first user should be created");
        assert!(first.created);
        assert!(first.assigned_super_admin);
        let roles = authz
            .list_user_roles(first.user.id)
            .await
            .expect("roles should list");
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].code, SUPER_ADMIN_ROLE_CODE);

        let second = repository
            .find_or_create_for_login_with_super_admin_bootstrap("ding-user-2", now)
            .await
            .expect("second user should be created");
        assert!(second.created);
        assert!(!second.assigned_super_admin);
        assert!(
            authz
                .list_user_roles(second.user.id)
                .await
                .expect("roles should list")
                .is_empty()
        );

        let reused = repository
            .find_or_create_for_login_with_super_admin_bootstrap("ding-user-1", now)
            .await
            .expect("existing user should be reused");
        assert_eq!(reused.user.id, first.user.id);
        assert!(!reused.created);
        assert!(!reused.assigned_super_admin);
    }

    #[tokio::test]
    async fn concurrent_bootstrapped_logins_assign_super_admin_once() {
        let repository = test_repository().await;
        let first_repository = repository.clone();
        let second_repository = repository.clone();
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        let (first, second) = tokio::join!(
            first_repository
                .find_or_create_for_login_with_super_admin_bootstrap("ding-user-1", now),
            second_repository
                .find_or_create_for_login_with_super_admin_bootstrap("ding-user-2", now),
        );
        let logins = vec![
            first.expect("first concurrent login should succeed"),
            second.expect("second concurrent login should succeed"),
        ];

        assert_eq!(
            logins
                .iter()
                .filter(|login| login.assigned_super_admin)
                .count(),
            1
        );

        let super_admin_role = roles::Entity::find()
            .filter(roles::Column::Code.eq(SUPER_ADMIN_ROLE_CODE))
            .one(&repository.db)
            .await
            .expect("role lookup should succeed")
            .expect("super admin role should exist");
        let assignments = user_roles::Entity::find()
            .filter(user_roles::Column::RoleId.eq(super_admin_role.id))
            .all(&repository.db)
            .await
            .expect("role assignments should list");
        assert_eq!(assignments.len(), 1);
    }
}

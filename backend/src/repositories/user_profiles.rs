use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, Set};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::user_profiles, repositories::RepositoryError};

#[derive(Clone)]
pub struct UserProfileRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProfileUpsert {
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub mobile: Option<String>,
    pub hide_mobile: Option<bool>,
    pub telephone: Option<String>,
    pub job_number: Option<String>,
    pub title: Option<String>,
    pub email: Option<String>,
    pub org_email: Option<String>,
    pub work_place: Option<String>,
    pub remark: Option<String>,
    pub department_external_ids: Option<String>,
    pub is_admin: Option<bool>,
    pub is_boss: Option<bool>,
    pub is_active: Option<bool>,
    pub is_senior: Option<bool>,
    pub hired_at: Option<DateTime<Utc>>,
}

impl UserProfileRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<user_profiles::Model>, RepositoryError> {
        self.find_by_user_id_in(&self.db, user_id).await
    }

    pub(crate) async fn find_by_user_id_in<C>(
        &self,
        conn: &C,
        user_id: Uuid,
    ) -> Result<Option<user_profiles::Model>, RepositoryError>
    where
        C: ConnectionTrait,
    {
        let profile = user_profiles::Entity::find_by_id(user_id).one(conn).await?;

        debug!(
            found = profile.is_some(),
            %user_id,
            "looked up user profile by user id"
        );
        Ok(profile)
    }

    #[tracing::instrument(level = "info", skip(self, input), fields(user_id = %user_id))]
    pub async fn upsert_profile(
        &self,
        user_id: Uuid,
        input: UserProfileUpsert,
        now: DateTime<Utc>,
    ) -> Result<user_profiles::Model, RepositoryError> {
        self.upsert_profile_in(&self.db, user_id, input, now).await
    }

    pub(crate) async fn upsert_profile_in<C>(
        &self,
        conn: &C,
        user_id: Uuid,
        input: UserProfileUpsert,
        now: DateTime<Utc>,
    ) -> Result<user_profiles::Model, RepositoryError>
    where
        C: ConnectionTrait,
    {
        let existing = self.find_by_user_id_in(conn, user_id).await?;
        let profile = match existing {
            Some(existing) => {
                let mut active = model_from_input(user_id, input, now, existing.created_at);
                active.user_id = Set(existing.user_id);
                active.update(conn).await?
            }
            None => {
                model_from_input(user_id, input, now, now)
                    .insert(conn)
                    .await?
            }
        };

        info!(%user_id, "upserted user profile");
        Ok(profile)
    }
}

fn model_from_input(
    user_id: Uuid,
    input: UserProfileUpsert,
    updated_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
) -> user_profiles::ActiveModel {
    user_profiles::ActiveModel {
        user_id: Set(user_id),
        name: Set(input.name),
        avatar_url: Set(input.avatar_url),
        mobile: Set(input.mobile),
        hide_mobile: Set(input.hide_mobile),
        telephone: Set(input.telephone),
        job_number: Set(input.job_number),
        title: Set(input.title),
        email: Set(input.email),
        org_email: Set(input.org_email),
        work_place: Set(input.work_place),
        remark: Set(input.remark),
        department_external_ids: Set(input.department_external_ids),
        is_admin: Set(input.is_admin),
        is_boss: Set(input.is_boss),
        is_active: Set(input.is_active),
        is_senior: Set(input.is_senior),
        hired_at: Set(input.hired_at),
        created_at: Set(created_at),
        updated_at: Set(updated_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        repositories::users::UserRepository,
    };
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_repositories() -> (UserRepository, UserProfileRepository) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        (
            UserRepository::new(db.clone()),
            UserProfileRepository::new(db),
        )
    }

    fn profile_input(name: &str) -> UserProfileUpsert {
        UserProfileUpsert {
            name: Some(name.to_string()),
            avatar_url: Some("https://example.test/avatar.png".to_string()),
            mobile: Some("13800000000".to_string()),
            hide_mobile: Some(false),
            telephone: Some("010-1234".to_string()),
            job_number: Some("A001".to_string()),
            title: Some("工程师".to_string()),
            email: Some("user@example.test".to_string()),
            org_email: Some("user@corp.example.test".to_string()),
            work_place: Some("上海".to_string()),
            remark: Some("备注".to_string()),
            department_external_ids: Some("[10,20]".to_string()),
            is_admin: Some(false),
            is_boss: Some(false),
            is_active: Some(true),
            is_senior: Some(false),
            hired_at: Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()),
        }
    }

    #[tokio::test]
    async fn inserts_updates_and_finds_profile() {
        let (users, profiles) = test_repositories().await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", created_at)
            .await
            .expect("user should be created");

        let profile = profiles
            .upsert_profile(user.id, profile_input("张三"), created_at)
            .await
            .expect("profile should be inserted");
        assert_eq!(profile.user_id, user.id);
        assert_eq!(profile.name.as_deref(), Some("张三"));
        assert_eq!(profile.department_external_ids.as_deref(), Some("[10,20]"));

        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let updated = profiles
            .upsert_profile(user.id, profile_input("李四"), updated_at)
            .await
            .expect("profile should be updated");
        assert_eq!(updated.user_id, user.id);
        assert_eq!(updated.name.as_deref(), Some("李四"));
        assert_eq!(updated.created_at, created_at);
        assert_eq!(updated.updated_at, updated_at);

        let found = profiles
            .find_by_user_id(user.id)
            .await
            .expect("profile lookup should succeed")
            .expect("profile should exist");
        assert_eq!(found.name.as_deref(), Some("李四"));
    }

    #[tokio::test]
    async fn allows_missing_optional_profile_fields() {
        let (users, profiles) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");

        let profile = profiles
            .upsert_profile(
                user.id,
                UserProfileUpsert {
                    name: None,
                    avatar_url: None,
                    mobile: None,
                    hide_mobile: None,
                    telephone: None,
                    job_number: None,
                    title: None,
                    email: None,
                    org_email: None,
                    work_place: None,
                    remark: None,
                    department_external_ids: None,
                    is_admin: None,
                    is_boss: None,
                    is_active: None,
                    is_senior: None,
                    hired_at: None,
                },
                now,
            )
            .await
            .expect("profile with optional fields should be inserted");

        assert_eq!(profile.user_id, user.id);
        assert!(profile.mobile.is_none());
        assert!(profile.email.is_none());
        assert!(profile.avatar_url.is_none());
    }

    #[tokio::test]
    async fn cascades_when_user_is_deleted() {
        let (users, profiles) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        profiles
            .upsert_profile(user.id, profile_input("张三"), now)
            .await
            .expect("profile should be inserted");

        users
            .delete_by_id(user.id)
            .await
            .expect("user delete should succeed");

        assert!(
            profiles
                .find_by_user_id(user.id)
                .await
                .expect("profile lookup should succeed")
                .is_none()
        );
    }
}

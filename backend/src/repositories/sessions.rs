use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    entities::{auth_sessions, oauth_login_states, users},
    repositories::RepositoryError,
};

#[derive(Clone)]
pub struct SessionRepository {
    db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedSessionRecord {
    pub session_id: Uuid,
    pub user: users::Model,
}

impl SessionRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "info", skip(self, session_token_hash), fields(user_id = %user_id))]
    pub async fn create_session(
        &self,
        user_id: Uuid,
        session_token_hash: &str,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<auth_sessions::Model, RepositoryError> {
        validate_required("session_token_hash", session_token_hash)?;

        let session = auth_sessions::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            session_token_hash: Set(session_token_hash.trim().to_string()),
            created_at: Set(now),
            last_seen_at: Set(now),
            expires_at: Set(expires_at),
            revoked_at: Set(None),
        }
        .insert(&self.db)
        .await?;

        info!(session_id = %session.id, "created auth session");
        Ok(session)
    }

    #[tracing::instrument(level = "debug", skip(self, session_token_hash))]
    pub async fn find_valid_session(
        &self,
        session_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<auth_sessions::Model>, RepositoryError> {
        validate_required("session_token_hash", session_token_hash)?;

        let session = auth_sessions::Entity::find()
            .filter(auth_sessions::Column::SessionTokenHash.eq(session_token_hash.trim()))
            .filter(auth_sessions::Column::RevokedAt.is_null())
            .filter(auth_sessions::Column::ExpiresAt.gt(now))
            .one(&self.db)
            .await?;

        debug!(found = session.is_some(), "looked up valid auth session");
        Ok(session)
    }

    #[tracing::instrument(level = "debug", skip(self, session_token_hash))]
    pub async fn find_user_by_valid_session(
        &self,
        session_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<users::Model>, RepositoryError> {
        Ok(self
            .find_session_and_user_by_valid_session(session_token_hash, now)
            .await?
            .map(|session| session.user))
    }

    #[tracing::instrument(level = "debug", skip(self, session_token_hash))]
    pub async fn find_session_and_user_by_valid_session(
        &self,
        session_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<AuthenticatedSessionRecord>, RepositoryError> {
        let Some(session) = self.find_valid_session(session_token_hash, now).await? else {
            return Ok(None);
        };
        let session_id = session.id;

        let user = users::Entity::find_by_id(session.user_id)
            .one(&self.db)
            .await?;

        if let Some(user) = user.as_ref().filter(|user| user.status == "disabled") {
            warn!(user_id = %user.id, "session belongs to disabled user");
            return Err(RepositoryError::DisabledUser);
        }

        let mut active: auth_sessions::ActiveModel = session.into();
        active.last_seen_at = Set(now);
        active.update(&self.db).await?;

        debug!(
            session_id = %session_id,
            found = user.is_some(),
            "resolved user from valid auth session"
        );
        Ok(user.map(|user| AuthenticatedSessionRecord { session_id, user }))
    }

    #[tracing::instrument(level = "info", skip(self, session_token_hash))]
    pub async fn revoke_session(
        &self,
        session_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let Some(session) = self.find_valid_session(session_token_hash, now).await? else {
            debug!("session revoke skipped because session was not active");
            return Ok(false);
        };

        let session_id = session.id;
        let mut active: auth_sessions::ActiveModel = session.into();
        active.revoked_at = Set(Some(now));
        active.update(&self.db).await?;

        info!(session_id = %session_id, "revoked auth session");
        Ok(true)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn revoke_session_by_id(
        &self,
        session_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let session = auth_sessions::Entity::find_by_id(session_id)
            .filter(auth_sessions::Column::RevokedAt.is_null())
            .filter(auth_sessions::Column::ExpiresAt.gt(now))
            .one(&self.db)
            .await?;

        let Some(session) = session else {
            debug!(session_id = %session_id, "session revoke skipped because session was not active");
            return Ok(false);
        };

        let mut active: auth_sessions::ActiveModel = session.into();
        active.revoked_at = Set(Some(now));
        active.update(&self.db).await?;

        info!(session_id = %session_id, "revoked auth session");
        Ok(true)
    }

    #[tracing::instrument(level = "info", skip(self), fields(user_id = %user_id))]
    pub async fn revoke_active_sessions_for_user(
        &self,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let sessions = auth_sessions::Entity::find()
            .filter(auth_sessions::Column::UserId.eq(user_id))
            .filter(auth_sessions::Column::RevokedAt.is_null())
            .filter(auth_sessions::Column::ExpiresAt.gt(now))
            .all(&self.db)
            .await?;
        let count = sessions.len() as u64;

        for session in sessions {
            let mut active: auth_sessions::ActiveModel = session.into();
            active.revoked_at = Set(Some(now));
            active.update(&self.db).await?;
        }

        info!(user_id = %user_id, revoked_count = count, "revoked active sessions for user");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self, state_hash), fields(provider = %provider))]
    pub async fn create_oauth_state(
        &self,
        provider: &str,
        state_hash: &str,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<oauth_login_states::Model, RepositoryError> {
        validate_required("provider", provider)?;
        validate_required("state_hash", state_hash)?;

        let state = oauth_login_states::ActiveModel {
            id: Set(Uuid::new_v4()),
            provider: Set(provider.trim().to_string()),
            state_hash: Set(state_hash.trim().to_string()),
            created_at: Set(now),
            expires_at: Set(expires_at),
            consumed_at: Set(None),
        }
        .insert(&self.db)
        .await?;

        debug!(state_id = %state.id, "created oauth state");
        Ok(state)
    }

    #[tracing::instrument(level = "debug", skip(self, state_hash), fields(provider = %provider))]
    pub async fn consume_oauth_state(
        &self,
        provider: &str,
        state_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        validate_required("provider", provider)?;
        validate_required("state_hash", state_hash)?;

        let state = oauth_login_states::Entity::find()
            .filter(oauth_login_states::Column::Provider.eq(provider.trim()))
            .filter(oauth_login_states::Column::StateHash.eq(state_hash.trim()))
            .filter(oauth_login_states::Column::ConsumedAt.is_null())
            .filter(oauth_login_states::Column::ExpiresAt.gt(now))
            .one(&self.db)
            .await?;

        let Some(state) = state else {
            debug!("oauth state was not found, already consumed, or expired");
            return Ok(false);
        };

        let state_id = state.id;
        let mut active: oauth_login_states::ActiveModel = state.into();
        active.consumed_at = Set(Some(now));
        active.update(&self.db).await?;

        debug!(state_id = %state_id, "consumed oauth state");
        Ok(true)
    }
}

pub fn hash_secret(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)
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
        entities::users as users_entity,
        repositories::users::UserRepository,
    };
    use chrono::{Duration, TimeZone};
    use sea_orm::{ActiveModelTrait, Set};
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_repositories() -> (UserRepository, SessionRepository) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        (UserRepository::new(db.clone()), SessionRepository::new(db))
    }

    #[tokio::test]
    async fn creates_and_reads_valid_session_without_exposing_token() {
        let (users, sessions) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let token_hash = hash_secret("raw-session-token");

        let session = sessions
            .create_session(user.id, &token_hash, now, now + Duration::hours(1))
            .await
            .expect("session should be created");
        let found = sessions
            .find_valid_session(&token_hash, now)
            .await
            .expect("session lookup should succeed")
            .expect("session should be valid");

        assert_eq!(session.id, found.id);
        assert_eq!(found.session_token_hash, token_hash);
        assert_ne!(found.session_token_hash, "raw-session-token");
    }

    #[tokio::test]
    async fn expired_and_revoked_sessions_are_not_valid() {
        let (users, sessions) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let expired_hash = hash_secret("expired");
        let active_hash = hash_secret("active");

        sessions
            .create_session(user.id, &expired_hash, now, now - Duration::seconds(1))
            .await
            .expect("expired session should be created");
        sessions
            .create_session(user.id, &active_hash, now, now + Duration::hours(1))
            .await
            .expect("active session should be created");
        sessions
            .revoke_session(&active_hash, now)
            .await
            .expect("session revoke should succeed");

        assert!(
            sessions
                .find_valid_session(&expired_hash, now)
                .await
                .expect("expired session lookup should succeed")
                .is_none()
        );
        assert!(
            sessions
                .find_valid_session(&active_hash, now)
                .await
                .expect("revoked session lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn revokes_active_sessions_for_user() {
        let (users, sessions) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let other_user = users
            .find_or_create_for_login("ding-user-2", now)
            .await
            .expect("other user should be created");
        let active_hash = hash_secret("active");
        let expired_hash = hash_secret("expired");
        let other_hash = hash_secret("other");
        sessions
            .create_session(user.id, &active_hash, now, now + Duration::hours(1))
            .await
            .expect("active session should be created");
        sessions
            .create_session(user.id, &expired_hash, now, now - Duration::seconds(1))
            .await
            .expect("expired session should be created");
        sessions
            .create_session(other_user.id, &other_hash, now, now + Duration::hours(1))
            .await
            .expect("other session should be created");

        let revoked = sessions
            .revoke_active_sessions_for_user(user.id, now)
            .await
            .expect("sessions should be revoked");

        assert_eq!(revoked, 1);
        assert!(
            sessions
                .find_valid_session(&active_hash, now)
                .await
                .expect("active session lookup should succeed")
                .is_none()
        );
        assert!(
            sessions
                .find_valid_session(&other_hash, now)
                .await
                .expect("other session lookup should succeed")
                .is_some()
        );
    }

    #[tokio::test]
    async fn resolves_user_by_valid_session_and_blocks_disabled_user() {
        let (users, sessions) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login("ding-user-1", now)
            .await
            .expect("user should be created");
        let token_hash = hash_secret("raw-session-token");
        sessions
            .create_session(user.id, &token_hash, now, now + Duration::hours(1))
            .await
            .expect("session should be created");

        let resolved = sessions
            .find_user_by_valid_session(&token_hash, now + Duration::minutes(1))
            .await
            .expect("user lookup should succeed")
            .expect("user should resolve from session");

        assert_eq!(resolved.id, user.id);

        let mut disabled_user: users_entity::ActiveModel = resolved.into();
        disabled_user.status = Set("disabled".to_string());
        disabled_user
            .update(&users.db)
            .await
            .expect("user should be disabled");

        let disabled_result = sessions
            .find_user_by_valid_session(&token_hash, now + Duration::minutes(2))
            .await;

        assert!(matches!(
            disabled_result,
            Err(RepositoryError::DisabledUser)
        ));
    }

    #[tokio::test]
    async fn oauth_state_can_only_be_consumed_once() {
        let (_, sessions) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let state_hash = hash_secret("oauth-state");

        sessions
            .create_oauth_state("dingtalk", &state_hash, now, now + Duration::minutes(10))
            .await
            .expect("state should be created");

        assert!(
            sessions
                .consume_oauth_state("dingtalk", &state_hash, now + Duration::minutes(1))
                .await
                .expect("state consume should succeed")
        );
        assert!(
            !sessions
                .consume_oauth_state("dingtalk", &state_hash, now + Duration::minutes(2))
                .await
                .expect("second state consume should be handled")
        );
    }

    #[tokio::test]
    async fn expired_oauth_state_cannot_be_consumed() {
        let (_, sessions) = test_repositories().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let state_hash = hash_secret("expired-oauth-state");

        sessions
            .create_oauth_state("dingtalk", &state_hash, now, now + Duration::minutes(10))
            .await
            .expect("state should be created");

        assert!(
            !sessions
                .consume_oauth_state("dingtalk", &state_hash, now + Duration::minutes(11))
                .await
                .expect("expired state consume should be handled")
        );
    }
}

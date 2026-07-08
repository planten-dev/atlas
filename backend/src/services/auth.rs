use chrono::{DateTime, Duration, Utc};
use sea_orm::ConnectionTrait;
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    config::DingTalkConfig,
    dto::users::{UserProfileResponse, UserResponse},
    entities::{
        auth_sessions,
        events::{ApprovalStatus, EventType},
        oauth_login_states, user_profiles, users,
    },
    integrations::dingtalk::{
        DingTalkClient, DingTalkError, DingTalkIdentity, DingTalkUserProfile,
    },
    repositories::{
        RepositoryError,
        events::{EventRepository, NewEvent},
        sessions::{SessionRepository, hash_secret},
        user_profiles::{UserProfileRepository, UserProfileUpsert},
        users::UserRepository,
    },
    services::audit::AuditService,
};

const PROVIDER_DINGTALK: &str = "dingtalk";
const OAUTH_STATE_TTL_MINUTES: i64 = 10;

#[derive(Clone)]
pub struct AuthService {
    dingtalk_config: DingTalkConfig,
    users: UserRepository,
    profiles: UserProfileRepository,
    events: EventRepository,
    audit: AuditService,
    sessions: SessionRepository,
    session_ttl_seconds: u64,
}

impl AuthService {
    pub fn new(
        dingtalk_config: DingTalkConfig,
        users: UserRepository,
        profiles: UserProfileRepository,
        events: EventRepository,
        sessions: SessionRepository,
        session_ttl_seconds: u64,
    ) -> Self {
        Self {
            dingtalk_config,
            users,
            profiles,
            audit: AuditService::new(events.clone()),
            events,
            sessions,
            session_ttl_seconds,
        }
    }

    #[tracing::instrument(level = "info", skip(self), fields(provider = PROVIDER_DINGTALK))]
    pub async fn begin_dingtalk_login(&self) -> Result<String, AuthError> {
        let now = Utc::now();
        let state = generate_secret();
        let state_hash = hash_secret(&state);
        let client = DingTalkClient::new(self.dingtalk_config.clone())?;
        let url = client.build_authorization_url(&state)?;
        let expires_at = now + Duration::minutes(OAUTH_STATE_TTL_MINUTES);
        let tx = self.audit.begin().await?;
        let oauth_state = self
            .sessions
            .create_oauth_state_in(&tx, PROVIDER_DINGTALK, &state_hash, now, expires_at)
            .await?;
        self.audit
            .record_create(
                &tx,
                "oauth_login_states",
                oauth_state.id,
                None,
                oauth_state_audit_value(&oauth_state),
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;

        info!(
            provider = PROVIDER_DINGTALK,
            "created DingTalk login redirect"
        );
        Ok(url)
    }

    #[tracing::instrument(level = "info", skip(self, input), fields(provider = PROVIDER_DINGTALK))]
    pub async fn complete_dingtalk_callback(
        &self,
        input: DingTalkCallbackInput,
    ) -> Result<LoginSession, AuthError> {
        if let Some(error) = input.error {
            warn!(
                provider = PROVIDER_DINGTALK,
                provider_error = %error,
                description = input.error_description.as_deref().unwrap_or(""),
                "DingTalk rejected login"
            );
            return Err(AuthError::ProviderRejected {
                error,
                description: input.error_description,
            });
        }

        let state = input
            .state
            .ok_or(AuthError::MissingCallbackField("state"))?;
        let code = input
            .auth_code
            .or(input.code)
            .ok_or(AuthError::MissingCallbackField("code/authCode"))?;
        let now = Utc::now();
        let state_consumed = self.consume_oauth_state_with_audit(&state, now).await?;

        if !state_consumed {
            warn!(
                provider = PROVIDER_DINGTALK,
                "received invalid or expired OAuth state"
            );
            return Err(AuthError::StateMismatch);
        }

        let client = DingTalkClient::new(self.dingtalk_config.clone())?;
        let token = client.exchange_code_for_token(&code).await?;
        let identity = client.identity_from_token(token).await?;
        let user = self
            .find_or_create_user_for_login_with_audit(&identity.dingtalk_user_id, now)
            .await?;

        if let Err(error) = self
            .sync_dingtalk_personal_profile(&identity, user.id, now)
            .await
        {
            warn!(
                user_id = %user.id,
                error_code = error.code(),
                "DingTalk personal profile sync failed; continuing login"
            );
        }

        if let Err(error) = self
            .sync_dingtalk_org_profile(
                &client,
                &identity.dingtalk_user_id,
                user.id,
                user.id,
                ProfileSyncTrigger::Login,
                now,
            )
            .await
        {
            warn!(
                user_id = %user.id,
                error_code = error.code(),
                "DingTalk profile sync failed; continuing login"
            );
        }

        let session_token = generate_secret();
        let expires_at = now + session_ttl(self.session_ttl_seconds)?;
        let session = self
            .create_session_with_audit(user.id, &session_token, now, expires_at)
            .await?;

        info!(
            user_id = %user.id,
            session_id = %session.id,
            "completed DingTalk login and created session"
        );

        Ok(LoginSession {
            user: UserResponse::from(user),
            session_token,
            expires_at,
        })
    }

    async fn consume_oauth_state_with_audit(
        &self,
        state: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, AuthError> {
        let tx = self.audit.begin().await?;
        let consumed = self
            .sessions
            .consume_oauth_state_in(&tx, PROVIDER_DINGTALK, &hash_secret(state), now)
            .await?;
        let Some(consumed) = consumed else {
            tx.rollback().await.map_err(RepositoryError::from)?;
            return Ok(false);
        };

        self.audit
            .record_update(
                &tx,
                "oauth_login_states",
                consumed.state.id,
                None,
                oauth_state_audit_value(&consumed.old_state),
                oauth_state_audit_value(&consumed.state),
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(true)
    }

    async fn find_or_create_user_for_login_with_audit(
        &self,
        dingtalk_user_id: &str,
        now: DateTime<Utc>,
    ) -> Result<users::Model, AuthError> {
        if let Some(user) = self
            .users
            .find_by_dingtalk_user_id(dingtalk_user_id)
            .await?
        {
            if user.status == "disabled" {
                warn!(user_id = %user.id, "blocked login for disabled user");
                return Err(RepositoryError::DisabledUser.into());
            }

            let old_value = user_audit_value(&user);
            let tx = self.audit.begin().await?;
            let user = self.users.touch_login_in(&tx, &user, now).await?;
            self.audit
                .record_update(
                    &tx,
                    "users",
                    user.id,
                    Some(user.id),
                    old_value,
                    user_audit_value(&user),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            info!(user_id = %user.id, "reused existing user for login");
            return Ok(user);
        }

        let tx = self.audit.begin().await?;
        let user = self
            .users
            .create_for_login_in(&tx, dingtalk_user_id, now)
            .await?;
        self.audit
            .record_create(
                &tx,
                "users",
                user.id,
                Some(user.id),
                user_audit_value(&user),
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(user)
    }

    async fn create_session_with_audit(
        &self,
        user_id: Uuid,
        session_token: &str,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<auth_sessions::Model, AuthError> {
        let tx = self.audit.begin().await?;
        let session = self
            .sessions
            .create_session_in(&tx, user_id, &hash_secret(session_token), now, expires_at)
            .await?;
        self.audit
            .record_create(
                &tx,
                "auth_sessions",
                session.id,
                Some(user_id),
                auth_session_audit_value(&session),
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(session)
    }

    async fn sync_dingtalk_personal_profile(
        &self,
        identity: &DingTalkIdentity,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<user_profiles::Model, ProfileSyncError> {
        self.upsert_profile_with_audit(
            user_id,
            user_id,
            profile_upsert_from_dingtalk(identity.profile.clone()),
            ProfileSyncTrigger::Login,
            now,
        )
        .await
    }

    async fn sync_dingtalk_org_profile(
        &self,
        client: &DingTalkClient,
        dingtalk_user_id: &str,
        actor_user_id: Uuid,
        user_id: Uuid,
        trigger: ProfileSyncTrigger,
        now: DateTime<Utc>,
    ) -> Result<user_profiles::Model, ProfileSyncError> {
        let profile = client.fetch_user_profile(dingtalk_user_id).await?;
        self.upsert_profile_with_audit(
            actor_user_id,
            user_id,
            profile_upsert_from_dingtalk(profile),
            trigger,
            now,
        )
        .await
    }

    async fn upsert_profile_with_audit(
        &self,
        actor_user_id: Uuid,
        user_id: Uuid,
        input: UserProfileUpsert,
        trigger: ProfileSyncTrigger,
        now: DateTime<Utc>,
    ) -> Result<user_profiles::Model, ProfileSyncError> {
        let tx = self.events.begin().await?;
        let existing = self.profiles.find_by_user_id_in(&tx, user_id).await?;
        let input = merge_profile_input(input, existing.as_ref());
        let profile = self
            .profiles
            .upsert_profile_in(&tx, user_id, input, now)
            .await?;
        self.record_profile_sync_event(
            &tx,
            actor_user_id,
            user_id,
            existing.as_ref(),
            &profile,
            trigger,
            now,
        )
        .await?;
        tx.commit().await.map_err(RepositoryError::from)?;

        info!(%user_id, "synced DingTalk user profile");
        Ok(profile)
    }

    async fn record_profile_sync_event<C>(
        &self,
        conn: &C,
        actor_user_id: Uuid,
        user_id: Uuid,
        existing: Option<&user_profiles::Model>,
        profile: &user_profiles::Model,
        trigger: ProfileSyncTrigger,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError>
    where
        C: ConnectionTrait,
    {
        let updated_fields = changed_profile_fields(existing, profile);
        self.events
            .insert_event(
                conn,
                NewEvent {
                    resource_type: "user_profiles".to_string(),
                    resource_id: Some(user_id),
                    actor_user_id: Some(actor_user_id),
                    event_type: EventType::Update,
                    approval_status: ApprovalStatus::None,
                    required_approval_count: None,
                    target_event_id: None,
                    old_value: Some(profile_audit_value(
                        user_id,
                        trigger,
                        existing.is_some(),
                        None,
                    )),
                    new_value: Some(profile_audit_value(
                        user_id,
                        trigger,
                        true,
                        Some(updated_fields),
                    )),
                    remark: Some("dingtalk_profile_sync".to_string()),
                },
                now,
            )
            .await?;
        Ok(())
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn sync_dingtalk_profile_for_user(
        &self,
        actor_user_id: Uuid,
        user_id: Uuid,
    ) -> Result<UserProfileResponse, AuthError> {
        let user = self
            .users
            .find_by_id(user_id)
            .await?
            .ok_or(AuthError::UserNotFound)?;
        let client = DingTalkClient::new(self.dingtalk_config.clone())?;
        let profile = self
            .sync_dingtalk_org_profile(
                &client,
                &user.dingtalk_user_id,
                actor_user_id,
                user_id,
                ProfileSyncTrigger::Manual,
                Utc::now(),
            )
            .await?;

        info!(%user_id, "manually synced DingTalk user profile");
        Ok(UserProfileResponse::from(profile))
    }

    #[tracing::instrument(level = "debug", skip(self, session_token))]
    pub async fn authenticate_session(
        &self,
        session_token: Option<&str>,
    ) -> Result<CurrentSession, AuthError> {
        let session_token = session_token.ok_or(AuthError::MissingSession)?;
        let session = self
            .sessions
            .find_session_and_user_by_valid_session(&hash_secret(session_token), Utc::now())
            .await?
            .ok_or(AuthError::InvalidSession)?;
        let current = CurrentSession {
            session_id: session.session_id,
            user: UserResponse::from(session.user),
        };

        debug!(
            user_id = %current.user.id,
            session_id = %current.session_id,
            "authenticated current session"
        );
        Ok(current)
    }

    #[tracing::instrument(level = "debug", skip(self, session_token))]
    pub async fn current_user(
        &self,
        session_token: Option<&str>,
    ) -> Result<UserResponse, AuthError> {
        Ok(self.authenticate_session(session_token).await?.user)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn logout(&self, session_id: Uuid) -> Result<(), AuthError> {
        self.logout_as(None, session_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn logout_as(
        &self,
        actor_user_id: Option<Uuid>,
        session_id: Uuid,
    ) -> Result<(), AuthError> {
        let now = Utc::now();
        let tx = self.audit.begin().await?;
        let old_session = self.sessions.find_session_by_id_in(&tx, session_id).await?;
        let revoked = self
            .sessions
            .revoke_session_by_id_in(&tx, session_id, now)
            .await?;

        if !revoked {
            return Err(AuthError::InvalidSession);
        }

        let session = self
            .sessions
            .find_session_by_id_in(&tx, session_id)
            .await?
            .ok_or(AuthError::InvalidSession)?;
        let actor_user_id = actor_user_id.or(Some(session.user_id));
        self.audit
            .record_update(
                &tx,
                "auth_sessions",
                session_id,
                actor_user_id,
                old_session
                    .as_ref()
                    .map(auth_session_audit_value)
                    .unwrap_or_else(|| json!({ "id": session_id, "present": false })),
                auth_session_audit_value(&session),
                now,
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;

        info!(session_id = %session_id, "logged out current session");
        Ok(())
    }
}

impl From<ProfileSyncError> for AuthError {
    fn from(error: ProfileSyncError) -> Self {
        match error {
            ProfileSyncError::DingTalk(error) => Self::DingTalk(error),
            ProfileSyncError::Repository(error) => Self::Repository(error),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ProfileSyncTrigger {
    Login,
    Manual,
}

impl ProfileSyncTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Error)]
enum ProfileSyncError {
    #[error(transparent)]
    DingTalk(#[from] DingTalkError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl ProfileSyncError {
    fn code(&self) -> &'static str {
        match self {
            Self::DingTalk(error) => match error {
                DingTalkError::MissingConfig(_) => "dingtalk_configuration_error",
                DingTalkError::MissingRequiredField { .. }
                | DingTalkError::MissingIdentityField(_) => "dingtalk_validation_error",
                DingTalkError::ProviderHttp { .. }
                | DingTalkError::ProviderApi { .. }
                | DingTalkError::MissingResponseField { .. }
                | DingTalkError::Http(_) => "dingtalk_error",
            },
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
        }
    }
}

fn profile_upsert_from_dingtalk(profile: DingTalkUserProfile) -> UserProfileUpsert {
    UserProfileUpsert {
        name: profile.name,
        avatar_url: profile.avatar_url,
        mobile: profile.mobile,
        hide_mobile: profile.hide_mobile,
        telephone: profile.telephone,
        job_number: profile.job_number,
        title: profile.title,
        email: profile.email,
        org_email: profile.org_email,
        work_place: profile.work_place,
        remark: profile.remark,
        department_external_ids: profile.department_external_ids,
        is_admin: profile.is_admin,
        is_boss: profile.is_boss,
        is_active: profile.is_active,
        is_senior: profile.is_senior,
        hired_at: profile.hired_at,
    }
}

fn merge_profile_input(
    input: UserProfileUpsert,
    existing: Option<&user_profiles::Model>,
) -> UserProfileUpsert {
    let Some(existing) = existing else {
        return input;
    };

    UserProfileUpsert {
        name: input.name.or_else(|| existing.name.clone()),
        avatar_url: input.avatar_url.or_else(|| existing.avatar_url.clone()),
        mobile: input.mobile.or_else(|| existing.mobile.clone()),
        hide_mobile: input.hide_mobile.or(existing.hide_mobile),
        telephone: input.telephone.or_else(|| existing.telephone.clone()),
        job_number: input.job_number.or_else(|| existing.job_number.clone()),
        title: input.title.or_else(|| existing.title.clone()),
        email: input.email.or_else(|| existing.email.clone()),
        org_email: input.org_email.or_else(|| existing.org_email.clone()),
        work_place: input.work_place.or_else(|| existing.work_place.clone()),
        remark: input.remark.or_else(|| existing.remark.clone()),
        department_external_ids: input
            .department_external_ids
            .or_else(|| existing.department_external_ids.clone()),
        is_admin: input.is_admin.or(existing.is_admin),
        is_boss: input.is_boss.or(existing.is_boss),
        is_active: input.is_active.or(existing.is_active),
        is_senior: input.is_senior.or(existing.is_senior),
        hired_at: input.hired_at.or(existing.hired_at),
    }
}

fn changed_profile_fields(
    existing: Option<&user_profiles::Model>,
    profile: &user_profiles::Model,
) -> Vec<&'static str> {
    let Some(existing) = existing else {
        return [
            ("name", profile.name.is_some()),
            ("avatar_url", profile.avatar_url.is_some()),
            ("mobile", profile.mobile.is_some()),
            ("hide_mobile", profile.hide_mobile.is_some()),
            ("telephone", profile.telephone.is_some()),
            ("job_number", profile.job_number.is_some()),
            ("title", profile.title.is_some()),
            ("email", profile.email.is_some()),
            ("org_email", profile.org_email.is_some()),
            ("work_place", profile.work_place.is_some()),
            ("remark", profile.remark.is_some()),
            (
                "department_external_ids",
                profile.department_external_ids.is_some(),
            ),
            ("is_admin", profile.is_admin.is_some()),
            ("is_boss", profile.is_boss.is_some()),
            ("is_active", profile.is_active.is_some()),
            ("is_senior", profile.is_senior.is_some()),
            ("hired_at", profile.hired_at.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, changed)| changed.then_some(field))
        .collect();
    };

    [
        ("name", existing.name != profile.name),
        ("avatar_url", existing.avatar_url != profile.avatar_url),
        ("mobile", existing.mobile != profile.mobile),
        ("hide_mobile", existing.hide_mobile != profile.hide_mobile),
        ("telephone", existing.telephone != profile.telephone),
        ("job_number", existing.job_number != profile.job_number),
        ("title", existing.title != profile.title),
        ("email", existing.email != profile.email),
        ("org_email", existing.org_email != profile.org_email),
        ("work_place", existing.work_place != profile.work_place),
        ("remark", existing.remark != profile.remark),
        (
            "department_external_ids",
            existing.department_external_ids != profile.department_external_ids,
        ),
        ("is_admin", existing.is_admin != profile.is_admin),
        ("is_boss", existing.is_boss != profile.is_boss),
        ("is_active", existing.is_active != profile.is_active),
        ("is_senior", existing.is_senior != profile.is_senior),
        ("hired_at", existing.hired_at != profile.hired_at),
    ]
    .into_iter()
    .filter_map(|(field, changed)| changed.then_some(field))
    .collect()
}

fn profile_audit_value(
    user_id: Uuid,
    trigger: ProfileSyncTrigger,
    has_profile: bool,
    updated_fields: Option<Vec<&'static str>>,
) -> Value {
    let mut value = json!({
        "user_id": user_id,
        "trigger": trigger.as_str(),
        "source": "dingtalk",
        "has_profile": has_profile,
    });
    if let Some(updated_fields) = updated_fields {
        value["updated_fields"] = json!(updated_fields);
    }
    value
}

fn user_audit_value(user: &users::Model) -> Value {
    json!({
        "id": user.id,
        "dingtalk_user_id_present": !user.dingtalk_user_id.is_empty(),
        "status": user.status,
        "created_at": user.created_at,
        "updated_at": user.updated_at,
        "last_login_at": user.last_login_at,
    })
}

fn auth_session_audit_value(session: &auth_sessions::Model) -> Value {
    json!({
        "id": session.id,
        "user_id": session.user_id,
        "created_at": session.created_at,
        "last_seen_at": session.last_seen_at,
        "expires_at": session.expires_at,
        "revoked_at": session.revoked_at,
    })
}

fn oauth_state_audit_value(state: &oauth_login_states::Model) -> Value {
    json!({
        "id": state.id,
        "provider": state.provider,
        "created_at": state.created_at,
        "expires_at": state.expires_at,
        "consumed_at": state.consumed_at,
    })
}

#[derive(Debug, Clone)]
pub struct DingTalkCallbackInput {
    pub code: Option<String>,
    pub auth_code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoginSession {
    pub user: UserResponse,
    pub session_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSession {
    pub session_id: Uuid,
    pub user: UserResponse,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LoginResponse {
    pub user: UserResponse,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error(transparent)]
    DingTalk(#[from] DingTalkError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("DingTalk callback is missing `{0}`")]
    MissingCallbackField(&'static str),
    #[error("DingTalk rejected login: {description}", description = description.as_deref().unwrap_or(error.as_str()))]
    ProviderRejected {
        error: String,
        description: Option<String>,
    },
    #[error("OAuth state is invalid or expired")]
    StateMismatch,
    #[error("user was not found")]
    UserNotFound,
    #[error("session cookie is required")]
    MissingSession,
    #[error("session is invalid or expired")]
    InvalidSession,
    #[error("session ttl is too large")]
    InvalidSessionTtl,
}

impl AuthError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DingTalk(error) => match error {
                DingTalkError::ProviderHttp { .. }
                | DingTalkError::ProviderApi { .. }
                | DingTalkError::MissingResponseField { .. }
                | DingTalkError::Http(_) => "dingtalk_error",
                DingTalkError::MissingConfig(_) => "dingtalk_configuration_error",
                DingTalkError::MissingRequiredField { .. }
                | DingTalkError::MissingIdentityField(_) => "dingtalk_validation_error",
            },
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::MissingCallbackField(_) => "missing_callback_field",
            Self::ProviderRejected { .. } => "provider_rejected",
            Self::StateMismatch => "state_mismatch",
            Self::UserNotFound => "user_not_found",
            Self::MissingSession => "missing_session",
            Self::InvalidSession => "invalid_session",
            Self::InvalidSessionTtl => "session_configuration_error",
        }
    }
}

fn generate_secret() -> String {
    format!("{}.{}", Uuid::new_v4(), Uuid::new_v4())
}

fn session_ttl(ttl_seconds: u64) -> Result<Duration, AuthError> {
    let ttl_seconds = i64::try_from(ttl_seconds).map_err(|_| AuthError::InvalidSessionTtl)?;
    Duration::try_seconds(ttl_seconds).ok_or(AuthError::InvalidSessionTtl)
}

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    config::DingTalkConfig,
    entities::users,
    integrations::dingtalk::{DingTalkClient, DingTalkError},
    repositories::{
        RepositoryError,
        sessions::{SessionRepository, hash_secret},
        users::UserRepository,
    },
};

const PROVIDER_DINGTALK: &str = "dingtalk";
const OAUTH_STATE_TTL_MINUTES: i64 = 10;

#[derive(Clone)]
pub struct AuthService {
    dingtalk_config: DingTalkConfig,
    users: UserRepository,
    sessions: SessionRepository,
    session_ttl_seconds: u64,
}

impl AuthService {
    pub fn new(
        dingtalk_config: DingTalkConfig,
        users: UserRepository,
        sessions: SessionRepository,
        session_ttl_seconds: u64,
    ) -> Self {
        Self {
            dingtalk_config,
            users,
            sessions,
            session_ttl_seconds,
        }
    }

    #[tracing::instrument(level = "info", skip(self), fields(provider = PROVIDER_DINGTALK))]
    pub async fn begin_dingtalk_login(&self) -> Result<String, AuthError> {
        let now = Utc::now();
        let state = generate_secret();
        let state_hash = hash_secret(&state);
        self.sessions
            .create_oauth_state(
                PROVIDER_DINGTALK,
                &state_hash,
                now,
                now + Duration::minutes(OAUTH_STATE_TTL_MINUTES),
            )
            .await?;

        let client = DingTalkClient::new(self.dingtalk_config.clone())?;
        let url = client.build_authorization_url(&state)?;
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
        let state_consumed = self
            .sessions
            .consume_oauth_state(PROVIDER_DINGTALK, &hash_secret(&state), now)
            .await?;

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
            .users
            .find_or_create_for_login(&identity.dingtalk_user_id, now)
            .await?;
        let session_token = generate_secret();
        let expires_at = now + session_ttl(self.session_ttl_seconds)?;
        let session = self
            .sessions
            .create_session(user.id, &hash_secret(&session_token), now, expires_at)
            .await?;

        info!(
            user_id = %user.id,
            session_id = %session.id,
            "completed DingTalk login and created session"
        );

        Ok(LoginSession {
            user: UserResponse::from_model(user),
            session_token,
            expires_at,
        })
    }

    #[tracing::instrument(level = "debug", skip(self, session_token))]
    pub async fn current_user(
        &self,
        session_token: Option<&str>,
    ) -> Result<UserResponse, AuthError> {
        let session_token = session_token.ok_or(AuthError::MissingSession)?;
        let user = self
            .sessions
            .find_user_by_valid_session(&hash_secret(session_token), Utc::now())
            .await?
            .ok_or(AuthError::InvalidSession)?;

        debug!(user_id = %user.id, "loaded current user from session");
        Ok(UserResponse::from_model(user))
    }

    #[tracing::instrument(level = "info", skip(self, session_token))]
    pub async fn logout(&self, session_token: Option<&str>) -> Result<(), AuthError> {
        let session_token = session_token.ok_or(AuthError::MissingSession)?;
        let revoked = self
            .sessions
            .revoke_session(&hash_secret(session_token), Utc::now())
            .await?;

        if !revoked {
            return Err(AuthError::InvalidSession);
        }

        info!("logged out current session");
        Ok(())
    }
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LoginResponse {
    pub user: UserResponse,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserResponse {
    pub id: Uuid,
    pub dingtalk_user_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl UserResponse {
    fn from_model(user: users::Model) -> Self {
        Self {
            id: user.id,
            dingtalk_user_id: user.dingtalk_user_id,
            status: user.status,
            created_at: user.created_at,
            updated_at: user.updated_at,
            last_login_at: user.last_login_at,
        }
    }
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
                DingTalkError::ProviderHttp { .. } | DingTalkError::Http(_) => "dingtalk_error",
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

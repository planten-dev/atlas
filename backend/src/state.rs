use crate::{
    config::{AuthConfig, SessionConfig},
    services::auth::AuthService,
};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(auth: AuthService, auth_config: AuthConfig, session_config: SessionConfig) -> Self {
        Self {
            auth,
            auth_config,
            session_config,
        }
    }
}

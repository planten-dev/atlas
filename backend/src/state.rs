use crate::{
    config::{AuthConfig, SessionConfig},
    services::{auth::AuthService, authz::AuthzService},
};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
    pub authz: AuthzService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(
        auth: AuthService,
        authz: AuthzService,
        auth_config: AuthConfig,
        session_config: SessionConfig,
    ) -> Self {
        Self {
            auth,
            authz,
            auth_config,
            session_config,
        }
    }
}

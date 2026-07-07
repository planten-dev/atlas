use crate::{
    config::{AuthConfig, SessionConfig},
    services::{auth::AuthService, users::UserService},
};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
    pub users: UserService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(
        auth: AuthService,
        users: UserService,
        auth_config: AuthConfig,
        session_config: SessionConfig,
    ) -> Self {
        Self {
            auth,
            users,
            auth_config,
            session_config,
        }
    }
}

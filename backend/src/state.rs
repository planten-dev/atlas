use crate::{
    config::{AuthConfig, SessionConfig},
    services::{
        auth::AuthService, authz::AuthzService, products::ProductService, users::UserService,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
    pub authz: AuthzService,
    pub users: UserService,
    pub products: ProductService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(
        auth: AuthService,
        authz: AuthzService,
        users: UserService,
        products: ProductService,
        auth_config: AuthConfig,
        session_config: SessionConfig,
    ) -> Self {
        Self {
            auth,
            authz,
            users,
            products,
            auth_config,
            session_config,
        }
    }
}

use crate::{
    config::{AuthConfig, SessionConfig},
    services::{
        auth::AuthService, authz::AuthzService, products::ProductService, systems::SystemService,
        users::UserService,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
    pub authz: AuthzService,
    pub users: UserService,
    pub products: ProductService,
    pub systems: SystemService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(
        auth: AuthService,
        authz: AuthzService,
        users: UserService,
        products: ProductService,
        systems: SystemService,
        auth_config: AuthConfig,
        session_config: SessionConfig,
    ) -> Self {
        Self {
            auth,
            authz,
            users,
            products,
            systems,
            auth_config,
            session_config,
        }
    }
}

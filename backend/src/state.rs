use crate::{
    config::{AuthConfig, SessionConfig},
    services::{
        auth::AuthService, authz::AuthzService, customers::CustomerService, events::EventService,
        product_categories::ProductCategoryService, products::ProductService, stores::StoreService,
        systems::SystemService, users::UserService,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
    pub authz: AuthzService,
    pub users: UserService,
    pub product_categories: ProductCategoryService,
    pub products: ProductService,
    pub systems: SystemService,
    pub stores: StoreService,
    pub customers: CustomerService,
    pub events: EventService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(
        auth: AuthService,
        authz: AuthzService,
        users: UserService,
        product_categories: ProductCategoryService,
        products: ProductService,
        systems: SystemService,
        stores: StoreService,
        customers: CustomerService,
        events: EventService,
        auth_config: AuthConfig,
        session_config: SessionConfig,
    ) -> Self {
        Self {
            auth,
            authz,
            users,
            product_categories,
            products,
            systems,
            stores,
            customers,
            events,
            auth_config,
            session_config,
        }
    }
}

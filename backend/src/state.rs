use crate::{
    config::{AuthConfig, SessionConfig},
    services::{
        auth::AuthService, authz::AuthzService, customers::CustomerService, events::EventService,
        product_categories::ProductCategoryService, products::ProductService,
        sales_records::SalesRecordService, stores::StoreService, systems::SystemService,
        users::UserService,
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
    pub sales_records: SalesRecordService,
    pub events: EventService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

pub struct AppStateParts {
    pub auth: AuthService,
    pub authz: AuthzService,
    pub users: UserService,
    pub product_categories: ProductCategoryService,
    pub products: ProductService,
    pub systems: SystemService,
    pub stores: StoreService,
    pub customers: CustomerService,
    pub sales_records: SalesRecordService,
    pub events: EventService,
    pub auth_config: AuthConfig,
    pub session_config: SessionConfig,
}

impl AppState {
    pub fn new(parts: AppStateParts) -> Self {
        Self {
            auth: parts.auth,
            authz: parts.authz,
            users: parts.users,
            product_categories: parts.product_categories,
            products: parts.products,
            systems: parts.systems,
            stores: parts.stores,
            customers: parts.customers,
            sales_records: parts.sales_records,
            events: parts.events,
            auth_config: parts.auth_config,
            session_config: parts.session_config,
        }
    }
}

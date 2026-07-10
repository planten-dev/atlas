pub mod authz;
pub mod customers;
pub mod departments;
pub mod events;
pub mod product_categories;
pub mod products;
pub mod sales_performance;
pub mod sales_records;
pub mod sessions;
pub mod stores;
pub mod systems;
pub mod user_profiles;
pub mod users;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database operation failed")]
    Database(#[from] sea_orm::DbErr),
    #[error("{field} is required")]
    MissingRequiredField { field: &'static str },
    #[error("user is disabled")]
    DisabledUser,
}

pub mod authz;
pub mod departments;
pub mod products;
pub mod sessions;
pub mod systems;
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

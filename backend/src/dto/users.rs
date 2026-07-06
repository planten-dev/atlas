use crate::entities::users::{Model, UserStatus};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub phone: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginUserRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub phone: String,
    pub status: UserStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<Model> for UserResponse {
    fn from(user: Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
            phone: user.phone,
            status: user.status,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

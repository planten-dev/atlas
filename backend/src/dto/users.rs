use crate::entities::users::{Model, UserStatus};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUserRequest {
    pub username: String,
    pub phone: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginUserRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub status: Option<String>,
    pub keyword: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Serialize)]
pub struct ListUsersResponse {
    pub items: Vec<UserResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub phone: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserStatusRequest {
    pub status: UserStatus,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeUserPasswordRequest {
    pub current_password: String,
    pub new_password: String,
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

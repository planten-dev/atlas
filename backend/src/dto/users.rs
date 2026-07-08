use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{user_profiles, users};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserResponse {
    pub id: Uuid,
    pub dingtalk_user_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListUsersResponse {
    pub users: Vec<UserResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserProfileResponse {
    pub user_id: Uuid,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub mobile: Option<String>,
    pub hide_mobile: Option<bool>,
    pub telephone: Option<String>,
    pub job_number: Option<String>,
    pub title: Option<String>,
    pub email: Option<String>,
    pub org_email: Option<String>,
    pub work_place: Option<String>,
    pub remark: Option<String>,
    pub department_external_ids: Option<String>,
    pub is_admin: Option<bool>,
    pub is_boss: Option<bool>,
    pub is_active: Option<bool>,
    pub is_senior: Option<bool>,
    pub hired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<users::Model> for UserResponse {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            dingtalk_user_id: user.dingtalk_user_id,
            status: user.status,
            created_at: user.created_at,
            updated_at: user.updated_at,
            last_login_at: user.last_login_at,
        }
    }
}

impl From<user_profiles::Model> for UserProfileResponse {
    fn from(profile: user_profiles::Model) -> Self {
        Self {
            user_id: profile.user_id,
            name: profile.name,
            avatar_url: profile.avatar_url,
            mobile: profile.mobile,
            hide_mobile: profile.hide_mobile,
            telephone: profile.telephone,
            job_number: profile.job_number,
            title: profile.title,
            email: profile.email,
            org_email: profile.org_email,
            work_place: profile.work_place,
            remark: profile.remark,
            department_external_ids: profile.department_external_ids,
            is_admin: profile.is_admin,
            is_boss: profile.is_boss,
            is_active: profile.is_active,
            is_senior: profile.is_senior,
            hired_at: profile.hired_at,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListUsersQuery {
    pub status_filter: Option<String>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUserStatusRequest {
    pub target_status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Disabled,
}

impl UserStatus {
    pub fn parse(field: &'static str, value: &str) -> Result<Self, UserStatusParseError> {
        match value.trim() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(UserStatusParseError {
                field,
                value: value.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStatusParseError {
    pub field: &'static str,
    pub value: String,
}

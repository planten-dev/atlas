use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{permission_policies, roles};

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub code: String,
    pub name: String,
    pub kind: String,
    pub priority: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RoleResponse {
    pub fn from_model(role: roles::Model) -> Self {
        Self {
            id: role.id,
            code: role.code,
            name: role.name,
            kind: role.kind,
            priority: role.priority,
            created_at: role.created_at,
            updated_at: role.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RoleDetailResponse {
    #[serde(flatten)]
    pub role: RoleResponse,
    pub parent_role_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SetRoleParentsRequest {
    pub parent_role_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SetUserRolesRequest {
    pub role_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct UserRolesResponse {
    pub user_id: Uuid,
    pub roles: Vec<RoleResponse>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub object: String,
    pub action: String,
    pub effect: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyResponse {
    pub id: Uuid,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub object: String,
    pub action: String,
    pub effect: String,
    pub created_at: DateTime<Utc>,
}

impl PolicyResponse {
    pub fn from_model(policy: permission_policies::Model) -> Self {
        Self {
            id: policy.id,
            subject_kind: policy.subject_kind,
            subject_id: policy.subject_id,
            object: policy.object,
            action: policy.action,
            effect: policy.effect,
            created_at: policy.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListPoliciesQuery {
    pub subject_kind: Option<String>,
    pub subject_id: Option<Uuid>,
}

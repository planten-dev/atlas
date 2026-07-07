use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::systems;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SystemResponse {
    pub id: Uuid,
    pub name: String,
    pub department_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListSystemsResponse {
    pub systems: Vec<SystemResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl From<systems::Model> for SystemResponse {
    fn from(system: systems::Model) -> Self {
        Self {
            id: system.id,
            name: system.name,
            department_id: system.department_id,
            status: system.status,
            created_at: system.created_at,
            updated_at: system.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListSystemsQuery {
    pub status_filter: Option<String>,
    pub department_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSystemRequest {
    pub name: String,
    pub department_id: Uuid,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateSystemRequest {
    #[serde(default)]
    pub name: PatchField<String>,
    #[serde(default)]
    pub department_id: PatchField<Uuid>,
    #[serde(default)]
    pub status: PatchField<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchField<T> {
    Unset,
    Null,
    Value(T),
}

impl<T> Default for PatchField<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<'de, T> Deserialize<'de> for PatchField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemStatus {
    Active,
    Disabled,
}

impl SystemStatus {
    pub fn parse(field: &'static str, value: &str) -> Result<Self, SystemStatusParseError> {
        match value.trim() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(SystemStatusParseError {
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
pub struct SystemStatusParseError {
    pub field: &'static str,
    pub value: String,
}

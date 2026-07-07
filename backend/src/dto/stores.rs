use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::stores;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StoreResponse {
    pub id: Uuid,
    pub name: String,
    pub system_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListStoresResponse {
    pub stores: Vec<StoreResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl From<stores::Model> for StoreResponse {
    fn from(store: stores::Model) -> Self {
        Self {
            id: store.id,
            name: store.name,
            system_id: store.system_id,
            status: store.status,
            created_at: store.created_at,
            updated_at: store.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListStoresQuery {
    pub status_filter: Option<String>,
    pub system_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateStoreRequest {
    pub name: String,
    pub system_id: Uuid,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateStoreRequest {
    #[serde(default)]
    pub name: PatchField<String>,
    #[serde(default)]
    pub system_id: PatchField<Uuid>,
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
pub enum StoreStatus {
    Active,
    Disabled,
}

impl StoreStatus {
    pub fn parse(field: &'static str, value: &str) -> Result<Self, StoreStatusParseError> {
        match value.trim() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(StoreStatusParseError {
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
pub struct StoreStatusParseError {
    pub field: &'static str,
    pub value: String,
}

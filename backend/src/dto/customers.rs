use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::customers;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CustomerAttachment {
    pub file_id: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CustomerResponse {
    pub id: Uuid,
    pub name: String,
    pub creator_user_id: Uuid,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub remark: Option<String>,
    pub status: String,
    pub attachments: Vec<CustomerAttachment>,
    pub outstanding_amount: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListCustomersResponse {
    pub customers: Vec<CustomerResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl From<customers::Model> for CustomerResponse {
    fn from(customer: customers::Model) -> Self {
        Self {
            id: customer.id,
            name: customer.name,
            creator_user_id: customer.creator_user_id,
            system_id: customer.system_id,
            store_id: customer.store_id,
            remark: customer.remark,
            status: customer.status,
            attachments: attachments_from_json(customer.attachments.as_deref()),
            outstanding_amount: "0.00".to_string(),
            created_at: customer.created_at,
            updated_at: customer.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListCustomersQuery {
    pub status_filter: Option<String>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub creator_user_id: Option<Uuid>,
    pub name_keyword: Option<String>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCustomerRequest {
    pub name: String,
    #[serde(default)]
    pub system_id: Option<Uuid>,
    pub store_id: Uuid,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub attachments: Option<Vec<CustomerAttachment>>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateCustomerRequest {
    #[serde(default)]
    pub name: PatchField<String>,
    #[serde(default)]
    pub system_id: PatchField<Uuid>,
    #[serde(default)]
    pub store_id: PatchField<Uuid>,
    #[serde(default)]
    pub remark: PatchField<String>,
    #[serde(default)]
    pub attachments: PatchField<Vec<CustomerAttachment>>,
    #[serde(default)]
    pub status: PatchField<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PatchField<T> {
    #[default]
    Unset,
    Null,
    Value(T),
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
pub enum CustomerStatus {
    Active,
    Disabled,
}

impl CustomerStatus {
    pub fn parse(field: &'static str, value: &str) -> Result<Self, CustomerStatusParseError> {
        match value.trim() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(CustomerStatusParseError {
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
pub struct CustomerStatusParseError {
    pub field: &'static str,
    pub value: String,
}

fn attachments_from_json(value: Option<&str>) -> Vec<CustomerAttachment> {
    value
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default()
}

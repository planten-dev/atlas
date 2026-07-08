use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::product_category;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProductCategoryResponse {
    pub id: Uuid,
    pub category_name: String,
    pub requires_operation_count: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProductCategorySummary {
    pub category_id: Uuid,
    pub category_name: String,
    pub requires_operation_count: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListProductCategoriesResponse {
    pub categories: Vec<ProductCategoryResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl From<product_category::Model> for ProductCategoryResponse {
    fn from(category: product_category::Model) -> Self {
        Self {
            id: category.id,
            category_name: category.category_name,
            requires_operation_count: category.requires_operation_count,
            status: category.status,
            created_at: category.created_at,
            updated_at: category.updated_at,
        }
    }
}

impl From<&product_category::Model> for ProductCategorySummary {
    fn from(category: &product_category::Model) -> Self {
        Self {
            category_id: category.id,
            category_name: category.category_name.clone(),
            requires_operation_count: category.requires_operation_count,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListProductCategoriesQuery {
    pub status_filter: Option<String>,
    pub requires_operation_count_filter: Option<bool>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProductCategoryRequest {
    pub category_name: String,
    pub requires_operation_count: bool,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateProductCategoryRequest {
    #[serde(default)]
    pub category_name: PatchField<String>,
    #[serde(default)]
    pub requires_operation_count: PatchField<bool>,
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
pub enum ProductCategoryStatus {
    Active,
    Disabled,
}

impl ProductCategoryStatus {
    pub fn parse(
        field: &'static str,
        value: &str,
    ) -> Result<Self, ProductCategoryStatusParseError> {
        match value.trim() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(ProductCategoryStatusParseError {
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
pub struct ProductCategoryStatusParseError {
    pub field: &'static str,
    pub value: String,
}

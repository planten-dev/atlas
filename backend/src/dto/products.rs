use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::{
    dto::product_categories::ProductCategorySummary,
    entities::{product_category, products},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProductResponse {
    pub id: Uuid,
    pub name: String,
    pub category_id: Uuid,
    pub category_name: String,
    pub requires_operation_count: bool,
    pub series: Option<String>,
    pub brand_name: Option<String>,
    pub specification: Option<String>,
    pub unit: Option<String>,
    pub unit_price: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListProductsResponse {
    pub products: Vec<ProductResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl ProductResponse {
    pub fn from_model(product: products::Model, category: product_category::Model) -> Self {
        let category_summary = ProductCategorySummary::from(&category);
        Self {
            id: product.id,
            name: product.name,
            category_id: category_summary.category_id,
            category_name: category_summary.category_name,
            requires_operation_count: category_summary.requires_operation_count,
            series: product.series,
            brand_name: product.brand_name,
            specification: product.specification,
            unit: product.unit,
            unit_price: format_price(product.unit_price),
            status: product.status,
            created_at: product.created_at,
            updated_at: product.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListProductsQuery {
    pub status_filter: Option<String>,
    pub category_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProductRequest {
    pub name: String,
    pub category_id: Uuid,
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub brand_name: Option<String>,
    #[serde(default)]
    pub specification: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    pub unit_price: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateProductRequest {
    #[serde(default)]
    pub name: PatchField<String>,
    #[serde(default)]
    pub category_id: PatchField<Uuid>,
    #[serde(default)]
    pub series: PatchField<String>,
    #[serde(default)]
    pub brand_name: PatchField<String>,
    #[serde(default)]
    pub specification: PatchField<String>,
    #[serde(default)]
    pub unit: PatchField<String>,
    #[serde(default)]
    pub unit_price: PatchField<String>,
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
pub enum ProductStatus {
    Active,
    Disabled,
}

impl ProductStatus {
    pub fn parse(field: &'static str, value: &str) -> Result<Self, ProductStatusParseError> {
        match value.trim() {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(ProductStatusParseError {
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
pub struct ProductStatusParseError {
    pub field: &'static str,
    pub value: String,
}

pub fn format_price(price: Decimal) -> String {
    format!("{price:.2}")
}

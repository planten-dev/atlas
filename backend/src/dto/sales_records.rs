use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::{
    sales_record_operation_counts, sales_record_operation_usages, sales_records,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesRecordResponse {
    pub id: Uuid,
    pub record_group_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub sale_date: NaiveDate,
    pub deal_status: String,
    pub customer_type: String,
    pub deal_type: String,
    pub content_category_id: Uuid,
    pub handler_user_id: Uuid,
    pub paid_amount: String,
    pub unpaid_amount: String,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub collaboration_type: String,
    pub expert_user_id: Option<Uuid>,
    pub consultant_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub status: String,
    pub operation_count: Option<OperationCountResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListSalesRecordsResponse {
    pub sales_records: Vec<SalesRecordResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CreateSalesRecordBatchResponse {
    pub record_group_id: Uuid,
    pub sales_records: Vec<SalesRecordResponse>,
}

impl SalesRecordResponse {
    pub fn from_model(
        record: sales_records::Model,
        operation_count: Option<sales_record_operation_counts::Model>,
    ) -> Self {
        Self {
            id: record.id,
            record_group_id: record.record_group_id,
            customer_id: record.customer_id,
            sale_date: record.sale_date,
            deal_status: record.deal_status,
            customer_type: record.customer_type,
            deal_type: record.deal_type,
            content_category_id: record.content_category_id,
            handler_user_id: record.handler_user_id,
            paid_amount: format_money(record.paid_amount),
            unpaid_amount: format_money(record.unpaid_amount),
            system_id: record.system_id,
            store_id: record.store_id,
            collaboration_type: record.collaboration_type,
            expert_user_id: record.expert_user_id,
            consultant_user_id: record.consultant_user_id,
            doctor_user_id: record.doctor_user_id,
            status: record.status,
            operation_count: operation_count.map(OperationCountResponse::from),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OperationCountResponse {
    pub sales_record_id: Uuid,
    pub total_count: i32,
    pub used_count: i32,
    pub remaining_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListOperationCountsResponse {
    pub operation_counts: Vec<OperationCountResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl From<sales_record_operation_counts::Model> for OperationCountResponse {
    fn from(count: sales_record_operation_counts::Model) -> Self {
        Self {
            sales_record_id: count.sales_record_id,
            total_count: count.total_count,
            used_count: count.used_count,
            remaining_count: count.total_count - count.used_count,
            status: count.status,
            created_at: count.created_at,
            updated_at: count.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OperationUsageResponse {
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub operated_at: DateTime<Utc>,
    pub operator_user_id: Uuid,
    pub doctor_user_id: Option<Uuid>,
    pub operation_count: i32,
    pub remark: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListOperationUsagesResponse {
    pub operation_usages: Vec<OperationUsageResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

impl From<sales_record_operation_usages::Model> for OperationUsageResponse {
    fn from(usage: sales_record_operation_usages::Model) -> Self {
        Self {
            id: usage.id,
            sales_record_id: usage.sales_record_id,
            operated_at: usage.operated_at,
            operator_user_id: usage.operator_user_id,
            doctor_user_id: usage.doctor_user_id,
            operation_count: usage.operation_count,
            remark: usage.remark,
            status: usage.status,
            created_at: usage.created_at,
            updated_at: usage.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListSalesRecordsQuery {
    pub status_filter: Option<String>,
    pub record_group_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub handler_user_id: Option<Uuid>,
    pub content_category_id: Option<Uuid>,
    pub sale_date_from: Option<NaiveDate>,
    pub sale_date_to: Option<NaiveDate>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSalesRecordBatchRequest {
    pub records: Vec<CreateSalesRecordRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSalesRecordRequest {
    pub customer_id: Uuid,
    pub sale_date: NaiveDate,
    pub deal_status: String,
    pub customer_type: String,
    pub deal_type: String,
    pub content_category_id: Uuid,
    pub handler_user_id: Uuid,
    pub paid_amount: String,
    pub unpaid_amount: String,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub collaboration_type: String,
    #[serde(default)]
    pub expert_user_id: Option<Uuid>,
    #[serde(default)]
    pub consultant_user_id: Option<Uuid>,
    #[serde(default)]
    pub doctor_user_id: Option<Uuid>,
    #[serde(default)]
    pub operation_total_count: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateSalesRecordRequest {
    #[serde(default)]
    pub customer_id: PatchField<Uuid>,
    #[serde(default)]
    pub sale_date: PatchField<NaiveDate>,
    #[serde(default)]
    pub deal_status: PatchField<String>,
    #[serde(default)]
    pub customer_type: PatchField<String>,
    #[serde(default)]
    pub deal_type: PatchField<String>,
    #[serde(default)]
    pub content_category_id: PatchField<Uuid>,
    #[serde(default)]
    pub handler_user_id: PatchField<Uuid>,
    #[serde(default)]
    pub paid_amount: PatchField<String>,
    #[serde(default)]
    pub unpaid_amount: PatchField<String>,
    #[serde(default)]
    pub system_id: PatchField<Uuid>,
    #[serde(default)]
    pub store_id: PatchField<Uuid>,
    #[serde(default)]
    pub collaboration_type: PatchField<String>,
    #[serde(default)]
    pub expert_user_id: PatchField<Uuid>,
    #[serde(default)]
    pub consultant_user_id: PatchField<Uuid>,
    #[serde(default)]
    pub doctor_user_id: PatchField<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListOperationCountsQuery {
    pub status_filter: Option<String>,
    pub sales_record_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateOperationCountRequest {
    pub total_count: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListOperationUsagesQuery {
    pub status_filter: Option<String>,
    pub sales_record_id: Option<Uuid>,
    pub operator_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub operated_at_from: Option<DateTime<Utc>>,
    pub operated_at_to: Option<DateTime<Utc>>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateOperationUsageRequest {
    pub sales_record_id: Uuid,
    pub operated_at: DateTime<Utc>,
    pub operator_user_id: Uuid,
    #[serde(default)]
    pub doctor_user_id: Option<Uuid>,
    #[serde(default = "default_operation_count")]
    pub operation_count: i32,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateOperationUsageRequest {
    #[serde(default)]
    pub operated_at: PatchField<DateTime<Utc>>,
    #[serde(default)]
    pub operator_user_id: PatchField<Uuid>,
    #[serde(default)]
    pub doctor_user_id: PatchField<Uuid>,
    #[serde(default)]
    pub operation_count: PatchField<i32>,
    #[serde(default)]
    pub remark: PatchField<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumParseError {
    pub field: &'static str,
    pub value: String,
    pub expected: &'static str,
}

pub fn parse_status(field: &'static str, value: &str) -> Result<&'static str, EnumParseError> {
    parse_enum(field, value, &["active", "voided"], "active, voided")
}

pub fn parse_deal_status(field: &'static str, value: &str) -> Result<&'static str, EnumParseError> {
    parse_enum(
        field,
        value,
        &["closed", "not_closed"],
        "closed, not_closed",
    )
}

pub fn parse_customer_type(
    field: &'static str,
    value: &str,
) -> Result<&'static str, EnumParseError> {
    parse_enum(field, value, &["new", "returning"], "new, returning")
}

pub fn parse_deal_type(field: &'static str, value: &str) -> Result<&'static str, EnumParseError> {
    parse_enum(field, value, &["non_salon", "salon"], "non_salon, salon")
}

pub fn parse_collaboration_type(
    field: &'static str,
    value: &str,
) -> Result<&'static str, EnumParseError> {
    parse_enum(
        field,
        value,
        &["expert_consultation", "self_sale"],
        "expert_consultation, self_sale",
    )
}

fn parse_enum(
    field: &'static str,
    value: &str,
    allowed: &[&'static str],
    expected: &'static str,
) -> Result<&'static str, EnumParseError> {
    let trimmed = value.trim();
    allowed
        .iter()
        .copied()
        .find(|candidate| *candidate == trimmed)
        .ok_or_else(|| EnumParseError {
            field,
            value: value.to_string(),
            expected,
        })
}

fn default_operation_count() -> i32 {
    1
}

pub fn format_money(value: Decimal) -> String {
    format!("{value:.2}")
}

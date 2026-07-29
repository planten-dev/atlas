use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::{
    sales_record_allocations, sales_record_lines, sales_record_operation_counts,
    sales_record_operation_usages, sales_records,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesRecordResponse {
    pub id: Uuid,
    pub record_type: String,
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
    pub total_amount: String,
    pub received_amount: String,
    pub debt_change: String,
    pub performance_status: Option<String>,
    pub customer_type: Option<String>,
    pub deal_type: Option<String>,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub handler_user_id: Uuid,
    pub expert_user_id: Option<Uuid>,
    pub consultant_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub remark: Option<String>,
    pub status: String,
    pub created_by_user_id: Uuid,
    pub lines: Vec<SalesRecordLineResponse>,
    pub allocations: Vec<SalesRecordAllocationResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SalesRecordResponse {
    pub fn from_parts(
        record: sales_records::Model,
        lines: Vec<SalesRecordLineResponse>,
        allocations: Vec<SalesRecordAllocationResponse>,
    ) -> Self {
        let debt_change = record.total_amount - record.received_amount;
        Self {
            id: record.id,
            record_type: record.record_type,
            customer_id: record.customer_id,
            record_date: record.record_date,
            total_amount: format_money(record.total_amount),
            received_amount: format_money(record.received_amount),
            debt_change: format_money(debt_change),
            performance_status: record.performance_status,
            customer_type: record.customer_type,
            deal_type: record.deal_type,
            system_id: record.system_id,
            store_id: record.store_id,
            handler_user_id: record.handler_user_id,
            expert_user_id: record.expert_user_id,
            consultant_user_id: record.consultant_user_id,
            doctor_user_id: record.doctor_user_id,
            remark: record.remark,
            status: record.status,
            created_by_user_id: record.created_by_user_id,
            lines,
            allocations,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListSalesRecordsResponse {
    pub sales_records: Vec<SalesRecordResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesRecordLineResponse {
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub product_id: Uuid,
    pub item_name: String,
    pub operation_total_count: Option<i32>,
    pub remark: Option<String>,
    pub status: String,
    pub operation_count: Option<OperationCountResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl SalesRecordLineResponse {
    pub fn from_model(
        line: sales_record_lines::Model,
        count: Option<sales_record_operation_counts::Model>,
    ) -> Self {
        let record_id = line.sales_record_id;
        Self {
            id: line.id,
            sales_record_id: record_id,
            product_id: line.product_id,
            item_name: line.item_name,
            operation_total_count: line.operation_total_count,
            remark: line.remark,
            status: line.status,
            operation_count: count.map(|c| OperationCountResponse::from_model(c, record_id)),
            created_at: line.created_at,
            updated_at: line.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesRecordAllocationResponse {
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub guide_user_id: Uuid,
    pub allocation_ratio: String,
    pub allocated_amount: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl From<sales_record_allocations::Model> for SalesRecordAllocationResponse {
    fn from(v: sales_record_allocations::Model) -> Self {
        Self {
            id: v.id,
            sales_record_id: v.sales_record_id,
            guide_user_id: v.guide_user_id,
            allocation_ratio: format_ratio(v.allocation_ratio),
            allocated_amount: format_money(v.allocated_amount),
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OperationCountResponse {
    pub sales_record_line_id: Uuid,
    pub sales_record_id: Uuid,
    pub total_count: i32,
    pub used_count: i32,
    pub remaining_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl OperationCountResponse {
    pub fn from_model(c: sales_record_operation_counts::Model, sales_record_id: Uuid) -> Self {
        Self {
            sales_record_line_id: c.sales_record_line_id,
            sales_record_id,
            total_count: c.total_count,
            used_count: c.used_count,
            remaining_count: c.total_count - c.used_count,
            status: c.status,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListOperationCountsResponse {
    pub operation_counts: Vec<OperationCountResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OperationUsageResponse {
    pub id: Uuid,
    pub sales_record_line_id: Uuid,
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
impl OperationUsageResponse {
    pub fn from_model(v: sales_record_operation_usages::Model, sales_record_id: Uuid) -> Self {
        Self {
            id: v.id,
            sales_record_line_id: v.sales_record_line_id,
            sales_record_id,
            operated_at: v.operated_at,
            operator_user_id: v.operator_user_id,
            doctor_user_id: v.doctor_user_id,
            operation_count: v.operation_count,
            remark: v.remark,
            status: v.status,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListOperationUsagesResponse {
    pub operation_usages: Vec<OperationUsageResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListSalesRecordsQuery {
    pub status_filter: Option<String>,
    pub record_type: Option<String>,
    pub customer_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub handler_user_id: Option<Uuid>,
    pub record_date_from: Option<NaiveDate>,
    pub record_date_to: Option<NaiveDate>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRecordLineInput {
    pub product_id: Uuid,
    pub item_name: String,
    #[serde(default)]
    pub operation_total_count: Option<i32>,
    #[serde(default)]
    pub remark: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRecordAllocationInput {
    pub guide_user_id: Uuid,
    pub allocation_ratio: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateDealRecordRequest {
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
    pub total_amount: String,
    pub received_amount: String,
    pub customer_type: String,
    pub deal_type: String,
    pub handler_user_id: Uuid,
    #[serde(default)]
    pub expert_user_id: Option<Uuid>,
    #[serde(default)]
    pub consultant_user_id: Option<Uuid>,
    #[serde(default)]
    pub doctor_user_id: Option<Uuid>,
    #[serde(default)]
    pub remark: Option<String>,
    pub lines: Vec<SalesRecordLineInput>,
    pub allocations: Vec<SalesRecordAllocationInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePreServiceRecordRequest {
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
    pub total_amount: String,
    #[serde(default)]
    pub customer_type: Option<String>,
    #[serde(default)]
    pub deal_type: Option<String>,
    pub handler_user_id: Uuid,
    #[serde(default)]
    pub expert_user_id: Option<Uuid>,
    #[serde(default)]
    pub consultant_user_id: Option<Uuid>,
    #[serde(default)]
    pub doctor_user_id: Option<Uuid>,
    #[serde(default)]
    pub remark: Option<String>,
    pub lines: Vec<SalesRecordLineInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateDebtCollectionRecordRequest {
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
    pub received_amount: String,
    pub handler_user_id: Uuid,
    #[serde(default)]
    pub expert_user_id: Option<Uuid>,
    #[serde(default)]
    pub consultant_user_id: Option<Uuid>,
    #[serde(default)]
    pub doctor_user_id: Option<Uuid>,
    #[serde(default)]
    pub remark: Option<String>,
    pub allocations: Vec<SalesRecordAllocationInput>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListOperationCountsQuery {
    pub status_filter: Option<String>,
    pub sales_record_line_id: Option<Uuid>,
    pub sales_record_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateOperationCountRequest {
    pub total_count: i32,
}
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListOperationUsagesQuery {
    pub status_filter: Option<String>,
    pub sales_record_line_id: Option<Uuid>,
    pub sales_record_id: Option<Uuid>,
    pub operator_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub operated_at_from: Option<DateTime<Utc>>,
    pub operated_at_to: Option<DateTime<Utc>>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateOperationUsageRequest {
    pub sales_record_line_id: Uuid,
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PatchField<T> {
    #[default]
    Unset,
    Null,
    Value(T),
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for PatchField<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(d).map(|v| match v {
            Some(v) => Self::Value(v),
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
impl std::fmt::Display for EnumParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} must be one of {}", self.field, self.expected)
    }
}
impl std::error::Error for EnumParseError {}
fn parse_enum(
    field: &'static str,
    value: &str,
    allowed: &[&'static str],
    expected: &'static str,
) -> Result<&'static str, EnumParseError> {
    let v = value.trim();
    allowed
        .iter()
        .copied()
        .find(|x| *x == v)
        .ok_or_else(|| EnumParseError {
            field,
            value: value.into(),
            expected,
        })
}
pub fn parse_status(field: &'static str, value: &str) -> Result<&'static str, EnumParseError> {
    parse_enum(field, value, &["active", "voided"], "active, voided")
}
pub fn parse_record_type(field: &'static str, value: &str) -> Result<&'static str, EnumParseError> {
    parse_enum(
        field,
        value,
        &["deal", "pre_service", "debt_collection"],
        "deal, pre_service, debt_collection",
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
fn default_operation_count() -> i32 {
    1
}
pub fn format_money(v: Decimal) -> String {
    format!("{v:.2}")
}
pub fn format_ratio(v: Decimal) -> String {
    format!("{v:.2}")
}

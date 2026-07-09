use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::entities::{
    sales_payment_allocations, sales_payments, sales_record_lines, sales_record_operation_counts,
    sales_record_operation_usages, sales_records,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesRecordResponse {
    pub id: Uuid,
    pub record_type: String,
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
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
    pub receivable_amount: String,
    pub paid_amount: String,
    pub outstanding_amount: String,
    pub lines: Vec<SalesRecordLineResponse>,
    pub payments: Vec<SalesPaymentResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SalesRecordResponse {
    pub fn from_parts(
        record: sales_records::Model,
        lines: Vec<SalesRecordLineResponse>,
        payments: Vec<SalesPaymentResponse>,
    ) -> Self {
        let receivable_amount = lines
            .iter()
            .filter(|line| line.status == "active")
            .map(|line| line.receivable_decimal)
            .sum::<Decimal>();
        let paid_amount = payments
            .iter()
            .filter(|payment| payment.status == "active")
            .map(|payment| payment.paid_decimal)
            .sum::<Decimal>();
        let outstanding_amount = receivable_amount - paid_amount;

        Self {
            id: record.id,
            record_type: record.record_type,
            customer_id: record.customer_id,
            record_date: record.record_date,
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
            receivable_amount: format_money(receivable_amount),
            paid_amount: format_money(paid_amount),
            outstanding_amount: format_money(outstanding_amount),
            lines,
            payments,
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
    pub receivable_amount: String,
    #[serde(skip_serializing)]
    pub receivable_decimal: Decimal,
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
        operation_count: Option<sales_record_operation_counts::Model>,
    ) -> Self {
        Self {
            id: line.id,
            sales_record_id: line.sales_record_id,
            product_id: line.product_id,
            item_name: line.item_name,
            receivable_amount: format_money(line.receivable_amount),
            receivable_decimal: line.receivable_amount,
            operation_total_count: line.operation_total_count,
            remark: line.remark,
            status: line.status,
            operation_count: operation_count.map(OperationCountResponse::from),
            created_at: line.created_at,
            updated_at: line.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesPaymentResponse {
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub payment_type: String,
    pub paid_amount: String,
    #[serde(skip_serializing)]
    pub paid_decimal: Decimal,
    pub paid_at: DateTime<Utc>,
    pub performance_status: String,
    pub status: String,
    pub remark: Option<String>,
    pub created_by_user_id: Uuid,
    pub allocations: Vec<SalesPaymentAllocationResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SalesPaymentResponse {
    pub fn from_parts(
        payment: sales_payments::Model,
        allocations: Vec<SalesPaymentAllocationResponse>,
    ) -> Self {
        Self {
            id: payment.id,
            sales_record_id: payment.sales_record_id,
            payment_type: payment.payment_type,
            paid_amount: format_money(payment.paid_amount),
            paid_decimal: payment.paid_amount,
            paid_at: payment.paid_at,
            performance_status: payment.performance_status,
            status: payment.status,
            remark: payment.remark,
            created_by_user_id: payment.created_by_user_id,
            allocations,
            created_at: payment.created_at,
            updated_at: payment.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListSalesPaymentsResponse {
    pub sales_payments: Vec<SalesPaymentResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SalesPaymentAllocationResponse {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub guide_user_id: Uuid,
    pub allocation_ratio: String,
    pub allocated_amount: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<sales_payment_allocations::Model> for SalesPaymentAllocationResponse {
    fn from(allocation: sales_payment_allocations::Model) -> Self {
        Self {
            id: allocation.id,
            payment_id: allocation.payment_id,
            guide_user_id: allocation.guide_user_id,
            allocation_ratio: format_ratio(allocation.allocation_ratio),
            allocated_amount: format_money(allocation.allocated_amount),
            created_at: allocation.created_at,
            updated_at: allocation.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OperationCountResponse {
    pub sales_record_line_id: Uuid,
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
            sales_record_line_id: count.sales_record_line_id,
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
    pub sales_record_line_id: Uuid,
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
            sales_record_line_id: usage.sales_record_line_id,
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListSalesPaymentsQuery {
    pub status_filter: Option<String>,
    pub payment_type: Option<String>,
    pub sales_record_id: Option<Uuid>,
    pub paid_at_from: Option<DateTime<Utc>>,
    pub paid_at_to: Option<DateTime<Utc>>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSaleRecordRequest {
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
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
    pub payment: SalesPaymentInput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateServiceRecordRequest {
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRecordLineInput {
    pub product_id: Uuid,
    pub item_name: String,
    pub receivable_amount: String,
    #[serde(default)]
    pub operation_total_count: Option<i32>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesPaymentInput {
    pub paid_amount: String,
    pub paid_at: DateTime<Utc>,
    pub allocations: Vec<SalesPaymentAllocationInput>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesPaymentAllocationInput {
    pub guide_user_id: Uuid,
    pub allocation_ratio: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCollectionPaymentRequest {
    pub sales_record_id: Uuid,
    pub paid_amount: String,
    pub paid_at: DateTime<Utc>,
    pub allocations: Vec<SalesPaymentAllocationInput>,
    #[serde(default)]
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListOperationCountsQuery {
    pub status_filter: Option<String>,
    pub sales_record_line_id: Option<Uuid>,
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
    pub sales_record_line_id: Option<Uuid>,
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

pub fn parse_record_type(field: &'static str, value: &str) -> Result<&'static str, EnumParseError> {
    parse_enum(field, value, &["sale", "service"], "sale, service")
}

pub fn parse_payment_type(
    field: &'static str,
    value: &str,
) -> Result<&'static str, EnumParseError> {
    parse_enum(field, value, &["initial", "collection"], "initial, collection")
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

pub fn format_ratio(value: Decimal) -> String {
    format!("{value:.2}")
}

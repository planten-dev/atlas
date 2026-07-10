use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PerformanceMonthQuery {
    pub period_month: NaiveDate,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceSummaryQuery {
    pub period_month: NaiveDate,
    pub user_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceEntriesQuery {
    pub period_month: NaiveDate,
    pub user_id: Option<Uuid>,
    pub payment_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceBatchesQuery {
    pub period_month: Option<NaiveDate>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePerformanceBatchRequest {
    pub period_month: NaiveDate,
    pub payment_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingPerformancePaymentResponse {
    pub payment_id: Uuid,
    pub sales_record_id: Uuid,
    pub paid_amount: String,
    pub paid_at: DateTime<Utc>,
    pub expert_user_id: Option<Uuid>,
    pub expert_amount: String,
    pub guide_amount: String,
    pub total_amount: String,
    pub guide_count: usize,
    pub system_id: Uuid,
    pub store_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ListPendingPerformanceResponse {
    pub payments: Vec<PendingPerformancePaymentResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceBatchResponse {
    pub id: Uuid,
    pub period_month: NaiveDate,
    pub batch_type: String,
    pub payment_count: i32,
    pub expert_amount: String,
    pub guide_amount: String,
    pub total_amount: String,
    pub posted_by_user_id: Option<Uuid>,
    pub posted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ListPerformanceBatchesResponse {
    pub batches: Vec<PerformanceBatchResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceEntryResponse {
    pub id: Uuid,
    pub batch_id: Uuid,
    pub payment_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub allocation_ratio: Option<String>,
    pub sales_record_id: Uuid,
    pub user_id: Uuid,
    pub performance_role: String,
    pub entry_type: String,
    pub amount: String,
    pub period_month: NaiveDate,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub source_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ListPerformanceEntriesResponse {
    pub entries: Vec<PerformanceEntryResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceSummaryResponse {
    pub user_id: Uuid,
    pub expert_amount: String,
    pub guide_amount: String,
    pub reversal_amount: String,
    pub net_amount: String,
}

#[derive(Debug, Serialize)]
pub struct ListPerformanceSummaryResponse {
    pub summaries: Vec<PerformanceSummaryResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

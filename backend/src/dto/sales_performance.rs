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
    pub performance_date_from: Option<NaiveDate>,
    pub performance_date_to: Option<NaiveDate>,
    pub user_id: Option<Uuid>,
    pub performance_role: Option<String>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub entry_type: Option<String>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}
#[derive(Debug, Deserialize)]
pub struct PerformanceEntriesQuery {
    pub performance_date_from: Option<NaiveDate>,
    pub performance_date_to: Option<NaiveDate>,
    pub user_id: Option<Uuid>,
    pub performance_role: Option<String>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub entry_type: Option<String>,
    pub sales_record_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}
#[derive(Debug, Deserialize)]
pub struct PerformanceExportQuery {
    pub performance_date_from: Option<NaiveDate>,
    pub performance_date_to: Option<NaiveDate>,
    pub user_id: Option<Uuid>,
    pub performance_role: Option<String>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub entry_type: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct PerformanceBatchesQuery {
    pub period_month: Option<NaiveDate>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePerformanceBatchRequest {
    pub period_month: NaiveDate,
    pub sales_record_ids: Vec<Uuid>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PendingPerformanceRecordResponse {
    pub sales_record_id: Uuid,
    pub received_amount: String,
    pub record_date: NaiveDate,
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
    pub sales_records: Vec<PendingPerformanceRecordResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceBatchResponse {
    pub id: Uuid,
    pub period_month: NaiveDate,
    pub batch_type: String,
    pub record_count: i32,
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
    pub record_allocation_id: Option<Uuid>,
    pub allocation_ratio: Option<String>,
    pub sales_record_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub job_number: String,
    pub performance_role: String,
    pub entry_type: String,
    pub amount: String,
    pub period_month: NaiveDate,
    pub performance_date: NaiveDate,
    pub system_id: Uuid,
    pub system_name: String,
    pub store_id: Uuid,
    pub store_name: String,
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
    pub user_name: String,
    pub job_number: String,
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

use crate::{
    dto::sales_performance::*,
    dto::sales_records::format_money,
    entities::{sales_performance_entries, sales_records},
    repositories::{
        RepositoryError,
        sales_performance::{
            NewPerformanceBatch, NewPerformanceEntry, PerformanceFilters,
            SalesPerformanceRepository,
        },
    },
};
use chrono::{Datelike, NaiveDate, Utc};
use rust_xlsxwriter::Workbook;
use sea_orm::DatabaseTransaction;
use sea_orm::entity::prelude::Decimal;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;
const DEFAULT_SIZE: u64 = 50;
const MAX_SIZE: u64 = 200;
#[derive(Clone)]
pub struct SalesPerformanceService {
    repository: SalesPerformanceRepository,
}
impl SalesPerformanceService {
    pub fn new(repository: SalesPerformanceRepository) -> Self {
        Self { repository }
    }
    pub async fn pending(
        &self,
        q: PerformanceMonthQuery,
    ) -> Result<ListPendingPerformanceResponse, SalesPerformanceError> {
        let (p, s) = pagination(q.page_number, q.page_size)?;
        let (from, to) = month_bounds(q.period_month)?;
        let (records, total) = self.repository.list_pending_records(from, to, p, s).await?;
        let mut out = Vec::new();
        for r in records {
            let allocations = self
                .repository
                .find_allocations_in(&self.repository.db, r.id)
                .await?;
            let guide: Decimal = allocations.iter().map(|a| a.allocated_amount).sum();
            out.push(PendingPerformanceRecordResponse {
                sales_record_id: r.id,
                received_amount: format_money(r.received_amount),
                record_date: r.record_date,
                expert_user_id: r.expert_user_id,
                expert_amount: format_money(if r.expert_user_id.is_some() {
                    r.received_amount
                } else {
                    Decimal::ZERO
                }),
                guide_amount: format_money(guide),
                total_amount: format_money(
                    guide
                        + if r.expert_user_id.is_some() {
                            r.received_amount
                        } else {
                            Decimal::ZERO
                        },
                ),
                guide_count: allocations.len(),
                system_id: r.system_id,
                store_id: r.store_id,
            });
        }
        Ok(ListPendingPerformanceResponse {
            sales_records: out,
            page_number: p,
            page_size: s,
            total_count: total,
        })
    }
    pub async fn post_batch(
        &self,
        actor: Uuid,
        r: CreatePerformanceBatchRequest,
    ) -> Result<PerformanceBatchResponse, SalesPerformanceError> {
        if r.sales_record_ids.is_empty() || r.sales_record_ids.len() > MAX_SIZE as usize {
            return Err(SalesPerformanceError::SalesRecordIdsInvalid);
        }
        if r.sales_record_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != r.sales_record_ids.len()
        {
            return Err(SalesPerformanceError::DuplicateSalesRecordId);
        }
        let (from, to) = month_bounds(r.period_month)?;
        let tx = self.repository.begin().await?;
        let records = self
            .repository
            .find_records_for_update(&tx, r.sales_record_ids.clone())
            .await?;
        if records.len() != r.sales_record_ids.len() {
            return Err(SalesPerformanceError::SalesRecordNotFound);
        }
        let mut drafts = Vec::new();
        let mut expert_total = Decimal::ZERO;
        let mut guide_total = Decimal::ZERO;
        for record in records {
            if record.status != "active" {
                return Err(SalesPerformanceError::SalesRecordInactive);
            }
            if record.performance_status.as_deref() != Some("pending") {
                return Err(SalesPerformanceError::SalesRecordNotPending);
            }
            if record.record_date < from || record.record_date >= to {
                return Err(SalesPerformanceError::SalesRecordOutsideMonth);
            }
            let allocations = self.repository.find_allocations_in(&tx, record.id).await?;
            let allocated: Decimal = allocations.iter().map(|a| a.allocated_amount).sum();
            if allocations.is_empty() || allocated != record.received_amount {
                return Err(SalesPerformanceError::AllocationTotalInvalid);
            }
            guide_total += allocated;
            if record.expert_user_id.is_some() {
                expert_total += record.received_amount
            }
            drafts.push((record, allocations));
        }
        let now = Utc::now();
        let batch = self
            .repository
            .insert_batch(
                &tx,
                NewPerformanceBatch {
                    period_month: from,
                    batch_type: "posting".into(),
                    record_count: drafts.len() as i32,
                    expert_amount: expert_total,
                    guide_amount: guide_total,
                    total_amount: expert_total + guide_total,
                    posted_by_user_id: Some(actor),
                },
                now,
            )
            .await?;
        for (record, allocations) in drafts {
            if let Some(user_id) = record.expert_user_id {
                self.repository
                    .insert_entry(
                        &tx,
                        entry(
                            &record,
                            batch.id,
                            user_id,
                            "expert",
                            record.received_amount,
                            None,
                            None,
                            "earning",
                            from,
                            record.record_date,
                            None,
                        ),
                        now,
                    )
                    .await?;
            }
            for a in allocations {
                self.repository
                    .insert_entry(
                        &tx,
                        entry(
                            &record,
                            batch.id,
                            a.guide_user_id,
                            "guide",
                            a.allocated_amount,
                            Some(a.id),
                            Some(a.allocation_ratio),
                            "earning",
                            from,
                            record.record_date,
                            None,
                        ),
                        now,
                    )
                    .await?;
            }
            self.repository
                .update_record_status(&tx, &record, "posted", now)
                .await?;
        }
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(batch_response(batch))
    }
    pub async fn batches(
        &self,
        q: PerformanceBatchesQuery,
    ) -> Result<ListPerformanceBatchesResponse, SalesPerformanceError> {
        let (p, s) = pagination(q.page_number, q.page_size)?;
        let (rows, total) = self.repository.list_batches(q.period_month, p, s).await?;
        Ok(ListPerformanceBatchesResponse {
            batches: rows.into_iter().map(batch_response).collect(),
            page_number: p,
            page_size: s,
            total_count: total,
        })
    }
    pub async fn entries(
        &self,
        q: PerformanceEntriesQuery,
    ) -> Result<ListPerformanceEntriesResponse, SalesPerformanceError> {
        let (p, s) = pagination(q.page_number, q.page_size)?;
        let f = filters(
            q.performance_date_from,
            q.performance_date_to,
            q.user_id,
            q.performance_role,
            q.system_id,
            q.store_id,
            q.entry_type,
            q.sales_record_id,
        )?;
        let (rows, total) = self.repository.list_entries(f, p, s).await?;
        Ok(ListPerformanceEntriesResponse {
            entries: rows.into_iter().map(entry_response).collect(),
            page_number: p,
            page_size: s,
            total_count: total,
        })
    }
    pub async fn summary(
        &self,
        q: PerformanceSummaryQuery,
    ) -> Result<ListPerformanceSummaryResponse, SalesPerformanceError> {
        let (p, s) = pagination(q.page_number, q.page_size)?;
        let f = filters(
            q.performance_date_from,
            q.performance_date_to,
            q.user_id,
            q.performance_role,
            q.system_id,
            q.store_id,
            q.entry_type,
            None,
        )?;
        let rows = self.repository.all_entries(f).await?;
        let mut map: HashMap<Uuid, (Decimal, Decimal, Decimal)> = HashMap::new();
        for e in rows {
            let v = map.entry(e.user_id).or_default();
            if e.entry_type == "reversal" {
                v.2 += e.amount
            } else if e.performance_role == "expert" {
                v.0 += e.amount
            } else {
                v.1 += e.amount
            }
        }
        let mut all = map
            .into_iter()
            .map(
                |(id, (expert, guide, reversal))| PerformanceSummaryResponse {
                    user_id: id,
                    user_name: String::new(),
                    job_number: String::new(),
                    expert_amount: format_money(expert),
                    guide_amount: format_money(guide),
                    reversal_amount: format_money(reversal),
                    net_amount: format_money(expert + guide + reversal),
                },
            )
            .collect::<Vec<_>>();
        let total = all.len() as u64;
        let start = ((p - 1) * s) as usize;
        let end = (start + s as usize).min(all.len());
        let summaries = if start >= all.len() {
            vec![]
        } else {
            all.drain(start..end).collect()
        };
        Ok(ListPerformanceSummaryResponse {
            summaries,
            page_number: p,
            page_size: s,
            total_count: total,
        })
    }
    pub async fn export(
        &self,
        q: PerformanceExportQuery,
    ) -> Result<(Vec<u8>, String), SalesPerformanceError> {
        let f = filters(
            q.performance_date_from,
            q.performance_date_to,
            q.user_id,
            q.performance_role,
            q.system_id,
            q.store_id,
            q.entry_type,
            None,
        )?;
        let rows = self.repository.all_entries(f).await?;
        let mut wb = Workbook::new();
        let ws = wb.add_worksheet();
        for (c, h) in ["日期", "销售记录", "人员", "角色", "类型", "金额"]
            .into_iter()
            .enumerate()
        {
            ws.write_string(0, c as u16, h).map_err(export_err)?;
        }
        for (i, e) in rows.iter().enumerate() {
            let row = (i + 1) as u32;
            ws.write_string(row, 0, e.performance_date.to_string())
                .map_err(export_err)?;
            ws.write_string(row, 1, e.sales_record_id.to_string())
                .map_err(export_err)?;
            ws.write_string(row, 2, e.user_id.to_string())
                .map_err(export_err)?;
            ws.write_string(row, 3, &e.performance_role)
                .map_err(export_err)?;
            ws.write_string(row, 4, &e.entry_type).map_err(export_err)?;
            ws.write_string(row, 5, format_money(e.amount))
                .map_err(export_err)?;
        }
        Ok((
            wb.save_to_buffer().map_err(export_err)?,
            "sales-performance.xlsx".into(),
        ))
    }
}

pub async fn reverse_record_in_transaction(
    tx: &DatabaseTransaction,
    record: &sales_records::Model,
    now: chrono::DateTime<Utc>,
) -> Result<(), SalesPerformanceError> {
    let repo = SalesPerformanceRepository::for_review_transaction();
    let originals = repo.earning_entries_for_record(tx, record.id).await?;
    if originals.is_empty() {
        return Err(SalesPerformanceError::PostedEntriesMissing);
    }
    let month = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
    let expert: Decimal = originals
        .iter()
        .filter(|e| e.performance_role == "expert")
        .map(|e| -e.amount)
        .sum();
    let guide: Decimal = originals
        .iter()
        .filter(|e| e.performance_role == "guide")
        .map(|e| -e.amount)
        .sum();
    let batch = repo
        .insert_batch(
            tx,
            NewPerformanceBatch {
                period_month: month,
                batch_type: "reversal".into(),
                record_count: 1,
                expert_amount: expert,
                guide_amount: guide,
                total_amount: expert + guide,
                posted_by_user_id: None,
            },
            now,
        )
        .await?;
    for e in originals {
        repo.insert_entry(
            tx,
            NewPerformanceEntry {
                batch_id: batch.id,
                record_allocation_id: e.record_allocation_id,
                allocation_ratio: e.allocation_ratio,
                sales_record_id: e.sales_record_id,
                user_id: e.user_id,
                performance_role: e.performance_role,
                entry_type: "reversal".into(),
                amount: -e.amount,
                period_month: month,
                performance_date: now.date_naive(),
                system_id: e.system_id,
                store_id: e.store_id,
                source_entry_id: Some(e.id),
            },
            now,
        )
        .await?;
    }
    repo.update_record_status(tx, record, "reversed", now)
        .await?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn entry(
    r: &sales_records::Model,
    batch_id: Uuid,
    user_id: Uuid,
    role: &str,
    amount: Decimal,
    allocation: Option<Uuid>,
    ratio: Option<Decimal>,
    kind: &str,
    month: NaiveDate,
    date: NaiveDate,
    source: Option<Uuid>,
) -> NewPerformanceEntry {
    NewPerformanceEntry {
        batch_id,
        record_allocation_id: allocation,
        allocation_ratio: ratio,
        sales_record_id: r.id,
        user_id,
        performance_role: role.into(),
        entry_type: kind.into(),
        amount,
        period_month: month,
        performance_date: date,
        system_id: r.system_id,
        store_id: r.store_id,
        source_entry_id: source,
    }
}
fn batch_response(
    b: crate::entities::sales_performance_batches::Model,
) -> PerformanceBatchResponse {
    PerformanceBatchResponse {
        id: b.id,
        period_month: b.period_month,
        batch_type: b.batch_type,
        record_count: b.record_count,
        expert_amount: format_money(b.expert_amount),
        guide_amount: format_money(b.guide_amount),
        total_amount: format_money(b.total_amount),
        posted_by_user_id: b.posted_by_user_id,
        posted_at: b.posted_at,
        created_at: b.created_at,
    }
}
fn entry_response(e: sales_performance_entries::Model) -> PerformanceEntryResponse {
    PerformanceEntryResponse {
        id: e.id,
        batch_id: e.batch_id,
        record_allocation_id: e.record_allocation_id,
        allocation_ratio: e.allocation_ratio.map(format_money),
        sales_record_id: e.sales_record_id,
        user_id: e.user_id,
        user_name: String::new(),
        job_number: String::new(),
        performance_role: e.performance_role,
        entry_type: e.entry_type,
        amount: format_money(e.amount),
        period_month: e.period_month,
        performance_date: e.performance_date,
        system_id: e.system_id,
        system_name: String::new(),
        store_id: e.store_id,
        store_name: String::new(),
        source_entry_id: e.source_entry_id,
        created_at: e.created_at,
    }
}
#[allow(clippy::too_many_arguments)]
fn filters(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    user: Option<Uuid>,
    role: Option<String>,
    system: Option<Uuid>,
    store: Option<Uuid>,
    kind: Option<String>,
    record: Option<Uuid>,
) -> Result<PerformanceFilters, SalesPerformanceError> {
    Ok(PerformanceFilters {
        from: from.unwrap_or(NaiveDate::MIN),
        to: to.unwrap_or(NaiveDate::MAX),
        user_id: user,
        performance_role: role,
        system_id: system,
        store_id: store,
        entry_type: kind,
        sales_record_id: record,
    })
}
fn pagination(p: Option<u64>, s: Option<u64>) -> Result<(u64, u64), SalesPerformanceError> {
    let p = p.unwrap_or(1);
    let s = s.unwrap_or(DEFAULT_SIZE);
    if p < 1 || !(1..=MAX_SIZE).contains(&s) {
        return Err(SalesPerformanceError::InvalidPagination);
    }
    Ok((p, s))
}
fn month_bounds(v: NaiveDate) -> Result<(NaiveDate, NaiveDate), SalesPerformanceError> {
    let from = NaiveDate::from_ymd_opt(v.year(), v.month(), 1)
        .ok_or(SalesPerformanceError::InvalidMonth)?;
    let (to_y, to_m) = if v.month() == 12 {
        (v.year() + 1, 1)
    } else {
        (v.year(), v.month() + 1)
    };
    Ok((
        from,
        NaiveDate::from_ymd_opt(to_y, to_m, 1).ok_or(SalesPerformanceError::InvalidMonth)?,
    ))
}
fn export_err(e: impl std::fmt::Display) -> SalesPerformanceError {
    SalesPerformanceError::ExportFailed(e.to_string())
}
#[derive(Debug, Error)]
pub enum SalesPerformanceError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("sales record ids must contain between 1 and 200 ids")]
    SalesRecordIdsInvalid,
    #[error("sales record ids contain duplicates")]
    DuplicateSalesRecordId,
    #[error("sales record not found")]
    SalesRecordNotFound,
    #[error("sales record is not pending")]
    SalesRecordNotPending,
    #[error("sales record is outside month")]
    SalesRecordOutsideMonth,
    #[error("sales record inactive")]
    SalesRecordInactive,
    #[error("allocation total invalid")]
    AllocationTotalInvalid,
    #[error("posted entries missing")]
    PostedEntriesMissing,
    #[error("invalid month")]
    InvalidMonth,
    #[error("invalid pagination")]
    InvalidPagination,
    #[error("export failed: {0}")]
    ExportFailed(String),
}
impl SalesPerformanceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(RepositoryError::Database(_)) => "database_error",
            Self::SalesRecordNotFound => "sales_record_not_found",
            Self::SalesRecordNotPending => "sales_record_not_pending",
            Self::SalesRecordOutsideMonth => "sales_record_outside_month",
            Self::AllocationTotalInvalid => "performance_allocation_total_invalid",
            Self::ExportFailed(_) => "performance_export_failed",
            _ => "validation_error",
        }
    }
}

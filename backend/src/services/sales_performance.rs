use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, TimeZone, Utc};
use sea_orm::DatabaseTransaction;
use sea_orm::entity::prelude::Decimal;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    dto::{
        sales_performance::{
            CreatePerformanceBatchRequest, ListPendingPerformanceResponse,
            ListPerformanceBatchesResponse, ListPerformanceEntriesResponse,
            ListPerformanceSummaryResponse, PendingPerformancePaymentResponse,
            PerformanceBatchResponse, PerformanceBatchesQuery, PerformanceEntriesQuery,
            PerformanceEntryResponse, PerformanceMonthQuery, PerformanceSummaryQuery,
            PerformanceSummaryResponse,
        },
        sales_records::format_money,
    },
    entities::{sales_payments, sales_performance_batches, sales_performance_entries},
    repositories::{
        RepositoryError,
        sales_performance::{NewPerformanceBatch, NewPerformanceEntry, SalesPerformanceRepository},
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;
const SHANGHAI_OFFSET_SECONDS: i32 = 8 * 60 * 60;

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
        query: PerformanceMonthQuery,
    ) -> Result<ListPendingPerformanceResponse, SalesPerformanceError> {
        let (page_number, page_size) = pagination(query.page_number, query.page_size)?;
        let (from, to) = month_bounds(query.period_month)?;
        let (payments, total_count) = self
            .repository
            .list_pending_payments(from, to, page_number, page_size)
            .await?;
        let mut responses = Vec::with_capacity(payments.len());
        for payment in payments {
            let record = self
                .repository
                .find_record_in(&self.repository.db, payment.sales_record_id)
                .await?
                .ok_or(SalesPerformanceError::SalesRecordNotFound)?;
            let allocations = self
                .repository
                .find_allocations_in(&self.repository.db, payment.id)
                .await?;
            let guide_amount = allocations
                .iter()
                .map(|a| a.allocated_amount)
                .sum::<Decimal>();
            if guide_amount != payment.paid_amount {
                warn!(payment_id = %payment.id, "pending payment allocation total is invalid");
                return Err(SalesPerformanceError::AllocationTotalInvalid);
            }
            let expert_amount = if record.expert_user_id.is_some() {
                payment.paid_amount
            } else {
                Decimal::ZERO
            };
            responses.push(PendingPerformancePaymentResponse {
                payment_id: payment.id,
                sales_record_id: payment.sales_record_id,
                paid_amount: format_money(payment.paid_amount),
                paid_at: payment.paid_at,
                expert_user_id: record.expert_user_id,
                expert_amount: format_money(expert_amount),
                guide_amount: format_money(guide_amount),
                total_amount: format_money(expert_amount + guide_amount),
                guide_count: allocations.len(),
                system_id: record.system_id,
                store_id: record.store_id,
            });
        }
        Ok(ListPendingPerformanceResponse {
            payments: responses,
            page_number,
            page_size,
            total_count,
        })
    }

    pub async fn post_batch(
        &self,
        posted_by_user_id: Uuid,
        request: CreatePerformanceBatchRequest,
    ) -> Result<PerformanceBatchResponse, SalesPerformanceError> {
        let (from, to) = month_bounds(request.period_month)?;
        if request.payment_ids.is_empty() || request.payment_ids.len() > MAX_PAGE_SIZE as usize {
            return Err(SalesPerformanceError::PaymentIdsInvalid);
        }
        let unique = request.payment_ids.iter().copied().collect::<HashSet<_>>();
        if unique.len() != request.payment_ids.len() {
            return Err(SalesPerformanceError::DuplicatePaymentId);
        }

        let payment_hints = self
            .repository
            .find_payments(request.payment_ids.clone())
            .await?;
        if payment_hints.len() != request.payment_ids.len() {
            return Err(SalesPerformanceError::PaymentNotFound);
        }
        let mut record_ids = payment_hints
            .iter()
            .map(|payment| payment.sales_record_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        record_ids.sort_unstable();

        let now = Utc::now();
        let tx = self.repository.begin().await?;
        let locked_records = self.repository.lock_records(&tx, record_ids).await?;
        let records_by_id = locked_records
            .into_iter()
            .map(|record| (record.id, record))
            .collect::<HashMap<_, _>>();
        let payments = self
            .repository
            .find_payments_for_update(&tx, request.payment_ids.clone())
            .await?;

        struct Draft {
            payment: sales_payments::Model,
            record: crate::entities::sales_records::Model,
            allocations: Vec<crate::entities::sales_payment_allocations::Model>,
        }
        let mut drafts = Vec::with_capacity(payments.len());
        let mut expert_amount = Decimal::ZERO;
        let mut guide_amount = Decimal::ZERO;
        for payment in payments {
            if payment.status != "active" || payment.performance_status != "pending" {
                return Err(SalesPerformanceError::PaymentNotPending);
            }
            if payment.paid_at < from || payment.paid_at >= to {
                return Err(SalesPerformanceError::PaymentOutsideMonth);
            }
            let record = records_by_id
                .get(&payment.sales_record_id)
                .cloned()
                .ok_or(SalesPerformanceError::SalesRecordNotFound)?;
            if record.status != "active" || record.record_type != "sale" {
                return Err(SalesPerformanceError::SalesRecordInactive);
            }
            let allocations = self.repository.find_allocations_in(&tx, payment.id).await?;
            let allocated = allocations
                .iter()
                .map(|a| a.allocated_amount)
                .sum::<Decimal>();
            if allocations.is_empty() || allocated != payment.paid_amount {
                return Err(SalesPerformanceError::AllocationTotalInvalid);
            }
            guide_amount += allocated;
            if record.expert_user_id.is_some() {
                expert_amount += payment.paid_amount;
            }
            drafts.push(Draft {
                payment,
                record,
                allocations,
            });
        }

        let batch = self
            .repository
            .insert_batch(
                &tx,
                NewPerformanceBatch {
                    period_month: request.period_month,
                    batch_type: "posting".to_string(),
                    payment_count: drafts.len() as i32,
                    expert_amount,
                    guide_amount,
                    total_amount: expert_amount + guide_amount,
                    posted_by_user_id: Some(posted_by_user_id),
                },
                now,
            )
            .await?;

        for draft in drafts {
            if let Some(expert_user_id) = draft.record.expert_user_id {
                self.repository
                    .insert_entry(
                        &tx,
                        NewPerformanceEntry {
                            batch_id: batch.id,
                            payment_id: draft.payment.id,
                            allocation_id: None,
                            allocation_ratio: None,
                            sales_record_id: draft.record.id,
                            user_id: expert_user_id,
                            performance_role: "expert".to_string(),
                            entry_type: "earning".to_string(),
                            amount: draft.payment.paid_amount,
                            period_month: request.period_month,
                            system_id: draft.record.system_id,
                            store_id: draft.record.store_id,
                            source_entry_id: None,
                        },
                        now,
                    )
                    .await?;
            }
            for allocation in draft.allocations {
                self.repository
                    .insert_entry(
                        &tx,
                        NewPerformanceEntry {
                            batch_id: batch.id,
                            payment_id: draft.payment.id,
                            allocation_id: Some(allocation.id),
                            allocation_ratio: Some(allocation.allocation_ratio),
                            sales_record_id: draft.record.id,
                            user_id: allocation.guide_user_id,
                            performance_role: "guide".to_string(),
                            entry_type: "earning".to_string(),
                            amount: allocation.allocated_amount,
                            period_month: request.period_month,
                            system_id: draft.record.system_id,
                            store_id: draft.record.store_id,
                            source_entry_id: None,
                        },
                        now,
                    )
                    .await?;
            }
            self.repository
                .update_payment_performance_status(&tx, &draft.payment, "posted", now)
                .await?;
        }
        tx.commit().await.map_err(RepositoryError::from)?;
        info!(batch_id = %batch.id, payment_count = batch.payment_count, "posted sales performance batch");
        Ok(batch_response(batch))
    }

    pub async fn batches(
        &self,
        query: PerformanceBatchesQuery,
    ) -> Result<ListPerformanceBatchesResponse, SalesPerformanceError> {
        let (page_number, page_size) = pagination(query.page_number, query.page_size)?;
        if let Some(month) = query.period_month {
            validate_month(month)?;
        }
        let (batches, total_count) = self
            .repository
            .list_batches(query.period_month, page_number, page_size)
            .await?;
        Ok(ListPerformanceBatchesResponse {
            batches: batches.into_iter().map(batch_response).collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    pub async fn entries(
        &self,
        query: PerformanceEntriesQuery,
    ) -> Result<ListPerformanceEntriesResponse, SalesPerformanceError> {
        validate_month(query.period_month)?;
        let (page_number, page_size) = pagination(query.page_number, query.page_size)?;
        let entries = self
            .repository
            .list_entries(query.period_month, query.user_id, query.payment_id)
            .await?;
        let total_count = entries.len() as u64;
        let start = page_number.saturating_sub(1).saturating_mul(page_size) as usize;
        let page = entries
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .map(entry_response)
            .collect();
        Ok(ListPerformanceEntriesResponse {
            entries: page,
            page_number,
            page_size,
            total_count,
        })
    }

    pub async fn summary(
        &self,
        query: PerformanceSummaryQuery,
    ) -> Result<ListPerformanceSummaryResponse, SalesPerformanceError> {
        validate_month(query.period_month)?;
        let (page_number, page_size) = pagination(query.page_number, query.page_size)?;
        let entries = self
            .repository
            .list_entries(query.period_month, query.user_id, None)
            .await?;
        let mut grouped: HashMap<Uuid, (Decimal, Decimal, Decimal, Decimal)> = HashMap::new();
        for entry in entries {
            let amounts = grouped.entry(entry.user_id).or_insert((
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
            ));
            if entry.entry_type == "earning" && entry.performance_role == "expert" {
                amounts.0 += entry.amount;
            }
            if entry.entry_type == "earning" && entry.performance_role == "guide" {
                amounts.1 += entry.amount;
            }
            if entry.entry_type == "reversal" {
                amounts.2 += -entry.amount;
            }
            amounts.3 += entry.amount;
        }
        let mut summary_values = grouped.into_iter().collect::<Vec<_>>();
        summary_values.sort_by(|(user_a, amounts_a), (user_b, amounts_b)| {
            amounts_b.3.cmp(&amounts_a.3).then(user_a.cmp(user_b))
        });
        let total_count = summary_values.len() as u64;
        let start = page_number.saturating_sub(1).saturating_mul(page_size) as usize;
        let page = summary_values
            .into_iter()
            .skip(start)
            .take(page_size as usize)
            .map(|(user_id, amounts)| PerformanceSummaryResponse {
                user_id,
                expert_amount: format_money(amounts.0),
                guide_amount: format_money(amounts.1),
                reversal_amount: format_money(amounts.2),
                net_amount: format_money(amounts.3),
            })
            .collect();
        Ok(ListPerformanceSummaryResponse {
            summaries: page,
            page_number,
            page_size,
            total_count,
        })
    }
}

pub async fn reverse_payment_in_transaction(
    tx: &DatabaseTransaction,
    payment: &sales_payments::Model,
    record: &crate::entities::sales_records::Model,
    now: DateTime<Utc>,
) -> Result<(), SalesPerformanceError> {
    let repo = SalesPerformanceRepository::for_review_transaction();
    match payment.performance_status.as_str() {
        "pending" => {
            repo.update_payment_performance_status(tx, payment, "cancelled", now)
                .await?;
        }
        "posted" => {
            let originals = repo.earning_entries_for_payment(tx, payment.id).await?;
            if originals.is_empty() {
                return Err(SalesPerformanceError::PostedEntriesMissing);
            }
            let month = shanghai_month(now);
            let expert_amount = originals
                .iter()
                .filter(|e| e.performance_role == "expert")
                .map(|e| -e.amount)
                .sum();
            let guide_amount = originals
                .iter()
                .filter(|e| e.performance_role == "guide")
                .map(|e| -e.amount)
                .sum();
            let batch = repo
                .insert_batch(
                    tx,
                    NewPerformanceBatch {
                        period_month: month,
                        batch_type: "reversal".to_string(),
                        payment_count: 1,
                        expert_amount,
                        guide_amount,
                        total_amount: expert_amount + guide_amount,
                        posted_by_user_id: None,
                    },
                    now,
                )
                .await?;
            for original in originals {
                repo.insert_entry(
                    tx,
                    NewPerformanceEntry {
                        batch_id: batch.id,
                        payment_id: original.payment_id,
                        allocation_id: original.allocation_id,
                        allocation_ratio: original.allocation_ratio,
                        sales_record_id: original.sales_record_id,
                        user_id: original.user_id,
                        performance_role: original.performance_role,
                        entry_type: "reversal".to_string(),
                        amount: -original.amount,
                        period_month: month,
                        system_id: record.system_id,
                        store_id: record.store_id,
                        source_entry_id: Some(original.id),
                    },
                    now,
                )
                .await?;
            }
            repo.update_payment_performance_status(tx, payment, "reversed", now)
                .await?;
            info!(payment_id = %payment.id, batch_id = %batch.id, "created automatic performance reversal");
        }
        "cancelled" | "reversed" => {}
        _ => return Err(SalesPerformanceError::PaymentStatusInvalid),
    }
    Ok(())
}

fn batch_response(batch: sales_performance_batches::Model) -> PerformanceBatchResponse {
    PerformanceBatchResponse {
        id: batch.id,
        period_month: batch.period_month,
        batch_type: batch.batch_type,
        payment_count: batch.payment_count,
        expert_amount: format_money(batch.expert_amount),
        guide_amount: format_money(batch.guide_amount),
        total_amount: format_money(batch.total_amount),
        posted_by_user_id: batch.posted_by_user_id,
        posted_at: batch.posted_at,
        created_at: batch.created_at,
    }
}

fn entry_response(entry: sales_performance_entries::Model) -> PerformanceEntryResponse {
    PerformanceEntryResponse {
        id: entry.id,
        batch_id: entry.batch_id,
        payment_id: entry.payment_id,
        allocation_id: entry.allocation_id,
        sales_record_id: entry.sales_record_id,
        allocation_ratio: entry
            .allocation_ratio
            .map(crate::dto::sales_records::format_ratio),
        user_id: entry.user_id,
        performance_role: entry.performance_role,
        entry_type: entry.entry_type,
        amount: format_money(entry.amount),
        period_month: entry.period_month,
        system_id: entry.system_id,
        store_id: entry.store_id,
        source_entry_id: entry.source_entry_id,
        created_at: entry.created_at,
    }
}

fn validate_month(month: NaiveDate) -> Result<(), SalesPerformanceError> {
    if month.day() != 1 {
        return Err(SalesPerformanceError::PeriodMonthInvalid);
    }
    Ok(())
}

fn month_bounds(month: NaiveDate) -> Result<(DateTime<Utc>, DateTime<Utc>), SalesPerformanceError> {
    validate_month(month)?;
    let next = if month.month() == 12 {
        NaiveDate::from_ymd_opt(month.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(month.year(), month.month() + 1, 1)
    }
    .ok_or(SalesPerformanceError::PeriodMonthInvalid)?;
    let offset = FixedOffset::east_opt(SHANGHAI_OFFSET_SECONDS)
        .ok_or(SalesPerformanceError::PeriodMonthInvalid)?;
    let start = offset
        .from_local_datetime(&month.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or(SalesPerformanceError::PeriodMonthInvalid)?
        .with_timezone(&Utc);
    let end = offset
        .from_local_datetime(&next.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or(SalesPerformanceError::PeriodMonthInvalid)?
        .with_timezone(&Utc);
    Ok((start, end))
}

fn shanghai_month(now: DateTime<Utc>) -> NaiveDate {
    let offset = FixedOffset::east_opt(SHANGHAI_OFFSET_SECONDS).expect("valid Shanghai offset");
    let local = now.with_timezone(&offset);
    NaiveDate::from_ymd_opt(local.year(), local.month(), 1).expect("valid month")
}

fn pagination(page: Option<u64>, size: Option<u64>) -> Result<(u64, u64), SalesPerformanceError> {
    let page = page.unwrap_or(DEFAULT_PAGE_NUMBER);
    let size = size.unwrap_or(DEFAULT_PAGE_SIZE);
    if page == 0 || size == 0 || size > MAX_PAGE_SIZE {
        return Err(SalesPerformanceError::PaginationInvalid);
    }
    Ok((page, size))
}

#[derive(Debug, Error)]
pub enum SalesPerformanceError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("period_month must be the first day of a month")]
    PeriodMonthInvalid,
    #[error("page parameters are invalid")]
    PaginationInvalid,
    #[error("payment_ids must contain between 1 and 200 ids")]
    PaymentIdsInvalid,
    #[error("payment_ids contains duplicates")]
    DuplicatePaymentId,
    #[error("payment was not found")]
    PaymentNotFound,
    #[error("payment is not active and pending performance posting")]
    PaymentNotPending,
    #[error("payment is outside the requested month")]
    PaymentOutsideMonth,
    #[error("sales record was not found")]
    SalesRecordNotFound,
    #[error("sales record is not an active sale")]
    SalesRecordInactive,
    #[error("payment allocations do not total the payment amount")]
    AllocationTotalInvalid,
    #[error("posted performance entries are missing")]
    PostedEntriesMissing,
    #[error("payment performance status is invalid")]
    PaymentStatusInvalid,
}

impl SalesPerformanceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(RepositoryError::Database(_)) => "database_error",
            Self::Repository(_) => "validation_error",
            Self::PeriodMonthInvalid => "period_month_invalid",
            Self::PaginationInvalid => "pagination_invalid",
            Self::PaymentIdsInvalid => "payment_ids_invalid",
            Self::DuplicatePaymentId => "duplicate_payment_id",
            Self::PaymentNotFound => "sales_payment_not_found",
            Self::PaymentNotPending => "performance_payment_not_pending",
            Self::PaymentOutsideMonth => "performance_payment_outside_month",
            Self::SalesRecordNotFound => "sales_record_not_found",
            Self::SalesRecordInactive => "performance_sales_record_inactive",
            Self::AllocationTotalInvalid => "performance_allocation_total_invalid",
            Self::PostedEntriesMissing => "performance_entries_missing",
            Self::PaymentStatusInvalid => "performance_status_invalid",
        }
    }
}

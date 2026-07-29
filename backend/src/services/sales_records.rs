use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseTransaction;
use sea_orm::entity::prelude::Decimal;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    dto::sales_records::*,
    entities::sales_record_operation_usages,
    repositories::{
        RepositoryError, customers::CustomerRepository,
        product_categories::ProductCategoryRepository, products::ProductRepository,
        sales_performance::SalesPerformanceRepository, sales_records::*, stores::StoreRepository,
        systems::SystemRepository, users::UserRepository,
    },
    services::{
        review::{ApplyError, ReviewableResource},
        sales_performance::SalesPerformanceService,
    },
};

const DEFAULT_PAGE: u64 = 1;
const DEFAULT_SIZE: u64 = 50;
const MAX_SIZE: u64 = 200;
const MAX_ITEM: usize = 128;
const MAX_REMARK: usize = 2000;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SalesRecordReviewDoc {
    Create(SalesRecordCreateDoc),
    Status {
        status: String,
    },
    OperationCount {
        sales_record_line_id: Uuid,
        total_count: i32,
        used_count: i32,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesRecordCreateDoc {
    pub record_type: String,
    pub customer_id: Uuid,
    pub record_date: chrono::NaiveDate,
    pub total_amount: String,
    pub received_amount: String,
    pub customer_type: Option<String>,
    pub deal_type: Option<String>,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub handler_user_id: Uuid,
    pub expert_user_id: Option<Uuid>,
    pub consultant_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub remark: Option<String>,
    pub created_by_user_id: Uuid,
    pub lines: Vec<SalesRecordLineDoc>,
    pub allocations: Vec<SalesRecordAllocationDoc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesRecordLineDoc {
    pub product_id: Uuid,
    pub item_name: String,
    pub operation_total_count: Option<i32>,
    pub remark: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesRecordAllocationDoc {
    pub guide_user_id: Uuid,
    pub allocation_ratio: String,
    pub allocated_amount: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SalesOperationUsageReviewDoc {
    Create(SalesOperationUsageDoc),
    State(SalesOperationUsageDoc),
    Delete,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOperationUsageDoc {
    pub sales_record_line_id: Uuid,
    pub operated_at: DateTime<Utc>,
    pub operator_user_id: Uuid,
    pub doctor_user_id: Option<Uuid>,
    pub operation_count: i32,
    pub remark: Option<String>,
    pub status: String,
}

#[derive(Clone)]
pub struct SalesRecordService {
    sales_records: SalesRecordRepository,
    customers: CustomerRepository,
    systems: SystemRepository,
    stores: StoreRepository,
    products: ProductRepository,
    categories: ProductCategoryRepository,
    users: UserRepository,
}
impl SalesRecordService {
    pub fn new(
        sales_records: SalesRecordRepository,
        customers: CustomerRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        categories: ProductCategoryRepository,
        users: UserRepository,
    ) -> Self {
        let products = ProductRepository::new(sales_records.db.clone());
        Self {
            sales_records,
            customers,
            systems,
            stores,
            products,
            categories,
            users,
        }
    }
    pub fn performance_service(&self) -> SalesPerformanceService {
        SalesPerformanceService::new(SalesPerformanceRepository::new(
            self.sales_records.db.clone(),
        ))
    }
    pub async fn customer_outstanding(&self, id: Uuid) -> Result<Decimal, SalesRecordError> {
        Ok(self
            .sales_records
            .customer_outstanding(&self.sales_records.db, id, None)
            .await?)
    }

    pub async fn prepare_deal_review(
        &self,
        actor: Uuid,
        r: CreateDealRecordRequest,
    ) -> Result<SalesRecordReviewDoc, SalesRecordError> {
        let total = parse_positive_money("total_amount", r.total_amount)?;
        let received = parse_positive_money("received_amount", r.received_amount)?;
        if received > total {
            return Err(SalesRecordError::ReceivedExceedsTotal);
        }
        let lines = self.prepare_lines(r.lines).await?;
        let allocations = self.prepare_allocations(received, r.allocations).await?;
        self.prepare_common(
            actor,
            "deal",
            r.customer_id,
            r.record_date,
            total,
            received,
            Some(parse_customer_type("customer_type", &r.customer_type)?.into()),
            Some(parse_deal_type("deal_type", &r.deal_type)?.into()),
            r.handler_user_id,
            r.expert_user_id,
            r.consultant_user_id,
            r.doctor_user_id,
            r.remark,
            lines,
            allocations,
        )
        .await
    }
    pub async fn prepare_pre_service_review(
        &self,
        actor: Uuid,
        r: CreatePreServiceRecordRequest,
    ) -> Result<SalesRecordReviewDoc, SalesRecordError> {
        let total = parse_positive_money("total_amount", r.total_amount)?;
        let lines = self.prepare_lines(r.lines).await?;
        self.prepare_common(
            actor,
            "pre_service",
            r.customer_id,
            r.record_date,
            total,
            Decimal::ZERO,
            optional_enum(r.customer_type, parse_customer_type)?,
            optional_enum(r.deal_type, parse_deal_type)?,
            r.handler_user_id,
            r.expert_user_id,
            r.consultant_user_id,
            r.doctor_user_id,
            r.remark,
            lines,
            vec![],
        )
        .await
    }
    pub async fn prepare_debt_collection_review(
        &self,
        actor: Uuid,
        r: CreateDebtCollectionRecordRequest,
    ) -> Result<SalesRecordReviewDoc, SalesRecordError> {
        let received = parse_positive_money("received_amount", r.received_amount)?;
        let outstanding = self.customer_outstanding(r.customer_id).await?;
        if received > outstanding {
            return Err(SalesRecordError::CollectionExceedsOutstanding);
        }
        let allocations = self.prepare_allocations(received, r.allocations).await?;
        self.prepare_common(
            actor,
            "debt_collection",
            r.customer_id,
            r.record_date,
            Decimal::ZERO,
            received,
            None,
            None,
            r.handler_user_id,
            r.expert_user_id,
            r.consultant_user_id,
            r.doctor_user_id,
            r.remark,
            vec![],
            allocations,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn prepare_common(
        &self,
        actor: Uuid,
        kind: &str,
        customer_id: Uuid,
        record_date: chrono::NaiveDate,
        total: Decimal,
        received: Decimal,
        customer_type: Option<String>,
        deal_type: Option<String>,
        handler: Uuid,
        expert: Option<Uuid>,
        consultant: Option<Uuid>,
        doctor: Option<Uuid>,
        remark: Option<String>,
        lines: Vec<SalesRecordLineDoc>,
        allocations: Vec<SalesRecordAllocationDoc>,
    ) -> Result<SalesRecordReviewDoc, SalesRecordError> {
        self.ensure_user(actor).await?;
        for id in [Some(handler), expert, consultant, doctor]
            .into_iter()
            .flatten()
        {
            self.ensure_user(id).await?
        }
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(SalesRecordError::CustomerNotFound)?;
        if customer.status != "active" {
            return Err(SalesRecordError::CustomerDisabled);
        }
        let system = self
            .systems
            .find_by_id(customer.system_id)
            .await?
            .ok_or(SalesRecordError::SystemNotFound)?;
        let store = self
            .stores
            .find_by_id(customer.store_id)
            .await?
            .ok_or(SalesRecordError::StoreNotFound)?;
        if system.status != "active" || store.status != "active" {
            return Err(SalesRecordError::CustomerScopeDisabled);
        }
        Ok(SalesRecordReviewDoc::Create(SalesRecordCreateDoc {
            record_type: kind.into(),
            customer_id,
            record_date,
            total_amount: format_money(total),
            received_amount: format_money(received),
            customer_type,
            deal_type,
            system_id: customer.system_id,
            store_id: customer.store_id,
            handler_user_id: handler,
            expert_user_id: expert,
            consultant_user_id: consultant,
            doctor_user_id: doctor,
            remark: limited_optional(remark, MAX_REMARK)?,
            created_by_user_id: actor,
            lines,
            allocations,
        }))
    }
    async fn prepare_lines(
        &self,
        items: Vec<SalesRecordLineInput>,
    ) -> Result<Vec<SalesRecordLineDoc>, SalesRecordError> {
        if items.is_empty() {
            return Err(SalesRecordError::LinesRequired);
        }
        let mut out = Vec::new();
        for i in items {
            let p = self
                .products
                .find_by_id(i.product_id)
                .await?
                .ok_or(SalesRecordError::ProductNotFound)?;
            if p.status != "active" {
                return Err(SalesRecordError::ProductDisabled);
            }
            let c = self
                .categories
                .find_by_id(p.category_id)
                .await?
                .ok_or(SalesRecordError::CategoryNotFound)?;
            if c.status != "active" {
                return Err(SalesRecordError::CategoryDisabled);
            }
            match (c.requires_operation_count, i.operation_total_count) {
                (true, Some(v)) if v > 0 => {}
                (true, _) => return Err(SalesRecordError::OperationCountRequired),
                (false, Some(_)) => return Err(SalesRecordError::OperationCountNotAllowed),
                (false, None) => {}
            }
            out.push(SalesRecordLineDoc {
                product_id: i.product_id,
                item_name: required_text("item_name", i.item_name, MAX_ITEM)?,
                operation_total_count: i.operation_total_count,
                remark: limited_optional(i.remark, MAX_REMARK)?,
            });
        }
        Ok(out)
    }
    async fn prepare_allocations(
        &self,
        amount: Decimal,
        items: Vec<SalesRecordAllocationInput>,
    ) -> Result<Vec<SalesRecordAllocationDoc>, SalesRecordError> {
        if items.is_empty() {
            return Err(SalesRecordError::AllocationsRequired);
        }
        let mut seen = HashSet::new();
        let mut ratios = Decimal::ZERO;
        let mut parsed = Vec::new();
        for i in items {
            if !seen.insert(i.guide_user_id) {
                return Err(SalesRecordError::DuplicateGuide);
            }
            self.ensure_user(i.guide_user_id).await?;
            let ratio = parse_ratio(i.allocation_ratio)?;
            ratios += ratio;
            parsed.push((i.guide_user_id, ratio));
        }
        if ratios != Decimal::new(10000, 2) {
            return Err(SalesRecordError::AllocationTotalInvalid);
        }
        let mut allocated = Decimal::ZERO;
        let last = parsed.len() - 1;
        Ok(parsed
            .into_iter()
            .enumerate()
            .map(|(n, (id, ratio))| {
                let value = if n == last {
                    amount - allocated
                } else {
                    let v = (amount * ratio / Decimal::new(100, 0)).round_dp(2);
                    allocated += v;
                    v
                };
                SalesRecordAllocationDoc {
                    guide_user_id: id,
                    allocation_ratio: format_ratio(ratio),
                    allocated_amount: format_money(value),
                }
            })
            .collect())
    }
    async fn ensure_user(&self, id: Uuid) -> Result<(), SalesRecordError> {
        let u = self
            .users
            .find_by_id(id)
            .await?
            .ok_or(SalesRecordError::UserNotFound)?;
        if u.status != "active" {
            return Err(SalesRecordError::UserDisabled);
        }
        Ok(())
    }
    pub async fn sales_record_expert(&self, id: Uuid) -> Result<Option<Uuid>, SalesRecordError> {
        Ok(self
            .sales_records
            .find_sales_record_by_id(id)
            .await?
            .ok_or(SalesRecordError::NotFound)?
            .expert_user_id)
    }
    pub async fn prepare_void_record_review(
        &self,
        id: Uuid,
    ) -> Result<(SalesRecordReviewDoc, SalesRecordReviewDoc, Option<Uuid>), SalesRecordError> {
        let r = self
            .sales_records
            .find_sales_record_by_id(id)
            .await?
            .ok_or(SalesRecordError::NotFound)?;
        if r.status == "voided" {
            return Err(SalesRecordError::Voided);
        }
        Ok((
            SalesRecordReviewDoc::Status { status: r.status },
            SalesRecordReviewDoc::Status {
                status: "voided".into(),
            },
            r.expert_user_id,
        ))
    }
    pub async fn prepare_count_review(
        &self,
        line_id: Uuid,
        r: UpdateOperationCountRequest,
    ) -> Result<
        (
            Uuid,
            SalesRecordReviewDoc,
            SalesRecordReviewDoc,
            Option<Uuid>,
        ),
        SalesRecordError,
    > {
        if r.total_count < 1 {
            return Err(SalesRecordError::InvalidCount);
        }
        let line = self
            .sales_records
            .find_line_by_id(line_id)
            .await?
            .ok_or(SalesRecordError::LineNotFound)?;
        let c = self
            .sales_records
            .find_operation_count(line_id)
            .await?
            .ok_or(SalesRecordError::CountNotFound)?;
        if r.total_count < c.used_count {
            return Err(SalesRecordError::CountBelowUsed);
        }
        let expert = self.sales_record_expert(line.sales_record_id).await?;
        Ok((
            line.sales_record_id,
            SalesRecordReviewDoc::OperationCount {
                sales_record_line_id: line_id,
                total_count: c.total_count,
                used_count: c.used_count,
            },
            SalesRecordReviewDoc::OperationCount {
                sales_record_line_id: line_id,
                total_count: r.total_count,
                used_count: c.used_count,
            },
            expert,
        ))
    }
    pub async fn list_sales_records(
        &self,
        q: ListSalesRecordsQuery,
    ) -> Result<ListSalesRecordsResponse, SalesRecordError> {
        let (page, size) = pagination(q.page_number, q.page_size)?;
        let status = q
            .status_filter
            .as_deref()
            .map(|v| parse_status("status_filter", v))
            .transpose()?;
        let kind = q
            .record_type
            .as_deref()
            .map(|v| parse_record_type("record_type", v))
            .transpose()?;
        let (rows, total) = self
            .sales_records
            .list_sales_records(
                SalesRecordFilters {
                    status_filter: status,
                    record_type: kind,
                    customer_id: q.customer_id,
                    system_id: q.system_id,
                    store_id: q.store_id,
                    handler_user_id: q.handler_user_id,
                    record_date_from: q.record_date_from,
                    record_date_to: q.record_date_to,
                },
                page,
                size,
            )
            .await?;
        Ok(ListSalesRecordsResponse {
            sales_records: self.responses(rows).await?,
            page_number: page,
            page_size: size,
            total_count: total,
        })
    }
    pub async fn sales_record_detail(
        &self,
        id: Uuid,
    ) -> Result<SalesRecordResponse, SalesRecordError> {
        let r = self
            .sales_records
            .find_sales_record_by_id(id)
            .await?
            .ok_or(SalesRecordError::NotFound)?;
        Ok(self.responses(vec![r]).await?.remove(0))
    }
    async fn responses(
        &self,
        rows: Vec<crate::entities::sales_records::Model>,
    ) -> Result<Vec<SalesRecordResponse>, SalesRecordError> {
        let ids = rows.iter().map(|r| r.id).collect::<Vec<_>>();
        let lines = self
            .sales_records
            .find_lines_by_sales_record_ids(ids.clone())
            .await?;
        let counts = self
            .sales_records
            .find_operation_counts_by_line_ids_in(
                &self.sales_records.db,
                lines.iter().map(|l| l.id).collect(),
            )
            .await?;
        let allocations = self
            .sales_records
            .find_allocations_by_record_ids(&self.sales_records.db, ids)
            .await?;
        let mut lm: HashMap<Uuid, Vec<_>> = HashMap::new();
        let cm = counts
            .into_iter()
            .map(|c| (c.sales_record_line_id, c))
            .collect::<HashMap<_, _>>();
        for l in lines {
            let count = cm.get(&l.id).cloned();
            lm.entry(l.sales_record_id)
                .or_default()
                .push(SalesRecordLineResponse::from_model(l, count));
        }
        let mut am: HashMap<Uuid, Vec<_>> = HashMap::new();
        for a in allocations {
            am.entry(a.sales_record_id).or_default().push(a.into());
        }
        Ok(rows
            .into_iter()
            .map(|r| {
                let id = r.id;
                SalesRecordResponse::from_parts(
                    r,
                    lm.remove(&id).unwrap_or_default(),
                    am.remove(&id).unwrap_or_default(),
                )
            })
            .collect())
    }

    pub async fn list_operation_counts(
        &self,
        q: ListOperationCountsQuery,
    ) -> Result<ListOperationCountsResponse, SalesRecordError> {
        let (p, s) = pagination(q.page_number, q.page_size)?;
        let status = q
            .status_filter
            .as_deref()
            .map(|v| parse_status("status_filter", v))
            .transpose()?;
        let (rows, total) = self
            .sales_records
            .list_operation_counts(
                OperationCountFilters {
                    status_filter: status,
                    sales_record_line_id: q.sales_record_line_id,
                    sales_record_id: q.sales_record_id,
                },
                p,
                s,
            )
            .await?;
        let mut out = Vec::new();
        for c in rows {
            let line = self
                .sales_records
                .find_line_by_id(c.sales_record_line_id)
                .await?
                .ok_or(SalesRecordError::LineNotFound)?;
            out.push(OperationCountResponse::from_model(c, line.sales_record_id));
        }
        Ok(ListOperationCountsResponse {
            operation_counts: out,
            page_number: p,
            page_size: s,
            total_count: total,
        })
    }
    pub async fn operation_count_detail(
        &self,
        id: Uuid,
    ) -> Result<OperationCountResponse, SalesRecordError> {
        let c = self
            .sales_records
            .find_operation_count(id)
            .await?
            .ok_or(SalesRecordError::CountNotFound)?;
        let line = self
            .sales_records
            .find_line_by_id(id)
            .await?
            .ok_or(SalesRecordError::LineNotFound)?;
        Ok(OperationCountResponse::from_model(c, line.sales_record_id))
    }
    pub async fn list_operation_usages(
        &self,
        q: ListOperationUsagesQuery,
    ) -> Result<ListOperationUsagesResponse, SalesRecordError> {
        let (p, s) = pagination(q.page_number, q.page_size)?;
        let status = q
            .status_filter
            .as_deref()
            .map(|v| parse_status("status_filter", v))
            .transpose()?;
        let (rows, total) = self
            .sales_records
            .list_operation_usages(
                OperationUsageFilters {
                    status_filter: status,
                    sales_record_line_id: q.sales_record_line_id,
                    sales_record_id: q.sales_record_id,
                    operator_user_id: q.operator_user_id,
                    doctor_user_id: q.doctor_user_id,
                    operated_at_from: q.operated_at_from,
                    operated_at_to: q.operated_at_to,
                },
                p,
                s,
            )
            .await?;
        let mut out = Vec::new();
        for u in rows {
            let line = self
                .sales_records
                .find_line_by_id(u.sales_record_line_id)
                .await?
                .ok_or(SalesRecordError::LineNotFound)?;
            out.push(OperationUsageResponse::from_model(u, line.sales_record_id));
        }
        Ok(ListOperationUsagesResponse {
            operation_usages: out,
            page_number: p,
            page_size: s,
            total_count: total,
        })
    }
    pub async fn operation_usage_detail(
        &self,
        id: Uuid,
    ) -> Result<OperationUsageResponse, SalesRecordError> {
        let u = self
            .sales_records
            .find_operation_usage_by_id(id)
            .await?
            .ok_or(SalesRecordError::UsageNotFound)?;
        let line = self
            .sales_records
            .find_line_by_id(u.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::LineNotFound)?;
        Ok(OperationUsageResponse::from_model(u, line.sales_record_id))
    }
    pub async fn prepare_create_usage_review(
        &self,
        r: CreateOperationUsageRequest,
    ) -> Result<(SalesOperationUsageReviewDoc, Option<Uuid>), SalesRecordError> {
        if r.operation_count < 1 {
            return Err(SalesRecordError::InvalidCount);
        }
        self.ensure_user(r.operator_user_id).await?;
        if let Some(id) = r.doctor_user_id {
            self.ensure_user(id).await?
        }
        let line = self
            .sales_records
            .find_line_by_id(r.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::LineNotFound)?;
        let expert = self.sales_record_expert(line.sales_record_id).await?;
        Ok((
            SalesOperationUsageReviewDoc::Create(SalesOperationUsageDoc {
                sales_record_line_id: r.sales_record_line_id,
                operated_at: r.operated_at,
                operator_user_id: r.operator_user_id,
                doctor_user_id: r.doctor_user_id,
                operation_count: r.operation_count,
                remark: limited_optional(r.remark, MAX_REMARK)?,
                status: "active".into(),
            }),
            expert,
        ))
    }
    pub async fn prepare_update_usage_review(
        &self,
        id: Uuid,
        r: UpdateOperationUsageRequest,
    ) -> Result<
        (
            SalesOperationUsageReviewDoc,
            SalesOperationUsageReviewDoc,
            Option<Uuid>,
        ),
        SalesRecordError,
    > {
        let u = self
            .sales_records
            .find_operation_usage_by_id(id)
            .await?
            .ok_or(SalesRecordError::UsageNotFound)?;
        if u.status == "voided" {
            return Err(SalesRecordError::UsageVoided);
        }
        let old = usage_doc(&u);
        let operated_at = required_patch(r.operated_at, u.operated_at, "operated_at")?;
        let operator = required_patch(r.operator_user_id, u.operator_user_id, "operator_user_id")?;
        self.ensure_user(operator).await?;
        let doctor = optional_patch(r.doctor_user_id, u.doctor_user_id);
        if let Some(id) = doctor {
            self.ensure_user(id).await?
        }
        let count = required_patch(r.operation_count, u.operation_count, "operation_count")?;
        if count < 1 {
            return Err(SalesRecordError::InvalidCount);
        }
        let remark = optional_text_patch(r.remark, u.remark.clone())?;
        let new = SalesOperationUsageDoc {
            sales_record_line_id: u.sales_record_line_id,
            operated_at,
            operator_user_id: operator,
            doctor_user_id: doctor,
            operation_count: count,
            remark,
            status: u.status,
        };
        let expert = self
            .sales_record_expert(
                self.sales_records
                    .find_line_by_id(new.sales_record_line_id)
                    .await?
                    .ok_or(SalesRecordError::LineNotFound)?
                    .sales_record_id,
            )
            .await?;
        Ok((
            SalesOperationUsageReviewDoc::State(old),
            SalesOperationUsageReviewDoc::State(new),
            expert,
        ))
    }
    pub async fn prepare_void_usage_review(
        &self,
        id: Uuid,
    ) -> Result<
        (
            SalesOperationUsageReviewDoc,
            SalesOperationUsageReviewDoc,
            Option<Uuid>,
        ),
        SalesRecordError,
    > {
        let u = self
            .sales_records
            .find_operation_usage_by_id(id)
            .await?
            .ok_or(SalesRecordError::UsageNotFound)?;
        if u.status == "voided" {
            return Err(SalesRecordError::UsageVoided);
        }
        let old = usage_doc(&u);
        let mut new = old.clone();
        new.status = "voided".into();
        let line = self
            .sales_records
            .find_line_by_id(u.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::LineNotFound)?;
        Ok((
            SalesOperationUsageReviewDoc::State(old),
            SalesOperationUsageReviewDoc::State(new),
            self.sales_record_expert(line.sales_record_id).await?,
        ))
    }
    pub async fn prepare_delete_usage_review(
        &self,
        id: Uuid,
    ) -> Result<(SalesOperationUsageReviewDoc, Option<Uuid>), SalesRecordError> {
        let u = self
            .sales_records
            .find_operation_usage_by_id(id)
            .await?
            .ok_or(SalesRecordError::UsageNotFound)?;
        let line = self
            .sales_records
            .find_line_by_id(u.sales_record_line_id)
            .await?
            .ok_or(SalesRecordError::LineNotFound)?;
        Ok((
            SalesOperationUsageReviewDoc::State(usage_doc(&u)),
            self.sales_record_expert(line.sales_record_id).await?,
        ))
    }
}

#[async_trait]
impl ReviewableResource for SalesRecordReviewDoc {
    const RESOURCE_TYPE: &'static str = "sales:records";
    const APPROVAL_PERMISSION: &'static str = "sales:records:approve";
    async fn apply_insert(
        self,
        tx: &DatabaseTransaction,
        now: DateTime<Utc>,
    ) -> Result<Uuid, ApplyError> {
        let Self::Create(d) = self else {
            return Err(ApplyError::ResourceMissing);
        };
        let repo = SalesRecordRepository::for_review_transaction();
        repo.lock_customer(tx, d.customer_id)
            .await
            .map_err(apply_repo)?
            .ok_or(ApplyError::ResourceMissing)?;
        let total = review_decimal(&d.total_amount)?;
        let received = review_decimal(&d.received_amount)?;
        if d.record_type == "debt_collection"
            && received
                > repo
                    .customer_outstanding(tx, d.customer_id, None)
                    .await
                    .map_err(apply_repo)?
        {
            return Err(ApplyError::ResourceMissing);
        }
        let performance_status = if received > Decimal::ZERO {
            Some("pending".into())
        } else {
            None
        };
        let record = repo
            .insert_sales_record(
                tx,
                NewSalesRecord {
                    record_type: d.record_type,
                    customer_id: d.customer_id,
                    record_date: d.record_date,
                    total_amount: total,
                    received_amount: received,
                    performance_status,
                    customer_type: d.customer_type,
                    deal_type: d.deal_type,
                    system_id: d.system_id,
                    store_id: d.store_id,
                    handler_user_id: d.handler_user_id,
                    expert_user_id: d.expert_user_id,
                    consultant_user_id: d.consultant_user_id,
                    doctor_user_id: d.doctor_user_id,
                    remark: d.remark,
                    status: "active".into(),
                    created_by_user_id: d.created_by_user_id,
                },
                now,
            )
            .await
            .map_err(apply_repo)?;
        for l in d.lines {
            let count = l.operation_total_count;
            let line = repo
                .insert_sales_record_line(
                    tx,
                    NewSalesRecordLine {
                        sales_record_id: record.id,
                        product_id: l.product_id,
                        item_name: l.item_name,
                        operation_total_count: count,
                        remark: l.remark,
                        status: "active".into(),
                    },
                    now,
                )
                .await
                .map_err(apply_repo)?;
            if let Some(total_count) = count {
                repo.insert_operation_count(
                    tx,
                    NewOperationCount {
                        sales_record_line_id: line.id,
                        total_count,
                        used_count: 0,
                        status: "active".into(),
                    },
                    now,
                )
                .await
                .map_err(apply_repo)?;
            }
        }
        for a in d.allocations {
            repo.insert_allocation(
                tx,
                NewSalesRecordAllocation {
                    sales_record_id: record.id,
                    guide_user_id: a.guide_user_id,
                    allocation_ratio: review_decimal(&a.allocation_ratio)?,
                    allocated_amount: review_decimal(&a.allocated_amount)?,
                },
                now,
            )
            .await
            .map_err(apply_repo)?;
        }
        Ok(record.id)
    }
    async fn apply_update(
        self,
        tx: &DatabaseTransaction,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError> {
        let repo = SalesRecordRepository::for_review_transaction();
        match self {
            Self::Status { status } => {
                if status != "voided" {
                    return Err(ApplyError::ResourceMissing);
                }
                let r = repo
                    .find_sales_record_by_id_for_update(tx, id)
                    .await
                    .map_err(apply_repo)?
                    .ok_or(ApplyError::ResourceMissing)?;
                repo.lock_customer(tx, r.customer_id)
                    .await
                    .map_err(apply_repo)?
                    .ok_or(ApplyError::ResourceMissing)?;
                let remaining = repo
                    .customer_outstanding(tx, r.customer_id, Some(id))
                    .await
                    .map_err(apply_repo)?;
                if remaining < Decimal::ZERO {
                    return Err(ApplyError::ResourceMissing);
                }
                let lines = repo
                    .find_lines_by_sales_record_id_in(tx, id)
                    .await
                    .map_err(apply_repo)?;
                let ids = lines.iter().map(|l| l.id).collect::<Vec<_>>();
                if repo
                    .count_active_operation_usages_for_lines(tx, ids.clone())
                    .await
                    .map_err(apply_repo)?
                    > 0
                {
                    return Err(ApplyError::ResourceMissing);
                }
                if r.performance_status.as_deref() == Some("posted") {
                    crate::services::sales_performance::reverse_record_in_transaction(tx, &r, now)
                        .await
                        .map_err(|_| ApplyError::ResourceMissing)?;
                } else if r.performance_status.as_deref() == Some("pending") {
                    repo.update_performance_status(tx, &r, "cancelled", now)
                        .await
                        .map_err(apply_repo)?;
                }
                repo.update_sales_record_status(tx, &r, "voided", now)
                    .await
                    .map_err(apply_repo)?;
                for l in lines {
                    repo.update_line_status(tx, &l, "voided", now)
                        .await
                        .map_err(apply_repo)?;
                }
                for c in repo
                    .find_operation_counts_by_line_ids_in(tx, ids)
                    .await
                    .map_err(apply_repo)?
                {
                    repo.update_operation_count(
                        tx,
                        &c,
                        OperationCountChanges {
                            status: Some("voided".into()),
                            ..Default::default()
                        },
                        now,
                    )
                    .await
                    .map_err(apply_repo)?;
                }
                Ok(())
            }
            Self::OperationCount {
                sales_record_line_id,
                total_count,
                ..
            } => {
                let line = repo
                    .find_line_by_id_for_update(tx, sales_record_line_id)
                    .await
                    .map_err(apply_repo)?
                    .ok_or(ApplyError::ResourceMissing)?;
                if line.sales_record_id != id {
                    return Err(ApplyError::ResourceMissing);
                }
                let c = repo
                    .find_operation_count_for_update(tx, sales_record_line_id)
                    .await
                    .map_err(apply_repo)?
                    .ok_or(ApplyError::ResourceMissing)?;
                if total_count < c.used_count {
                    return Err(ApplyError::ResourceMissing);
                }
                repo.update_operation_count(
                    tx,
                    &c,
                    OperationCountChanges {
                        total_count: Some(total_count),
                        ..Default::default()
                    },
                    now,
                )
                .await
                .map_err(apply_repo)?;
                Ok(())
            }
            Self::Create(_) => Err(ApplyError::ResourceMissing),
        }
    }
    async fn apply_delete(
        _: &DatabaseTransaction,
        _: Uuid,
        _: DateTime<Utc>,
    ) -> Result<(), ApplyError> {
        Err(ApplyError::ResourceMissing)
    }
}

async fn usage_delta(
    repo: &SalesRecordRepository,
    tx: &DatabaseTransaction,
    line_id: Uuid,
    delta: i32,
    now: DateTime<Utc>,
) -> Result<(), ApplyError> {
    if delta == 0 {
        return Ok(());
    }
    let c = repo
        .find_operation_count_for_update(tx, line_id)
        .await
        .map_err(apply_repo)?
        .ok_or(ApplyError::ResourceMissing)?;
    let used = c
        .used_count
        .checked_add(delta)
        .ok_or(ApplyError::ResourceMissing)?;
    if c.status != "active" || used < 0 || used > c.total_count {
        return Err(ApplyError::ResourceMissing);
    }
    repo.update_operation_count(
        tx,
        &c,
        OperationCountChanges {
            used_count: Some(used),
            ..Default::default()
        },
        now,
    )
    .await
    .map_err(apply_repo)?;
    Ok(())
}
#[async_trait]
impl ReviewableResource for SalesOperationUsageReviewDoc {
    const RESOURCE_TYPE: &'static str = "sales:operation-usages";
    const APPROVAL_PERMISSION: &'static str = "sales:operation-usages:approve";
    async fn apply_insert(
        self,
        tx: &DatabaseTransaction,
        now: DateTime<Utc>,
    ) -> Result<Uuid, ApplyError> {
        let Self::Create(d) = self else {
            return Err(ApplyError::ResourceMissing);
        };
        let repo = SalesRecordRepository::for_review_transaction();
        let line = repo
            .find_line_by_id_for_update(tx, d.sales_record_line_id)
            .await
            .map_err(apply_repo)?
            .ok_or(ApplyError::ResourceMissing)?;
        let record = repo
            .find_sales_record_by_id_for_update(tx, line.sales_record_id)
            .await
            .map_err(apply_repo)?
            .ok_or(ApplyError::ResourceMissing)?;
        if line.status != "active" || record.status != "active" {
            return Err(ApplyError::ResourceMissing);
        }
        usage_delta(&repo, tx, line.id, d.operation_count, now).await?;
        Ok(repo
            .insert_operation_usage(
                tx,
                NewOperationUsage {
                    sales_record_line_id: d.sales_record_line_id,
                    operated_at: d.operated_at,
                    operator_user_id: d.operator_user_id,
                    doctor_user_id: d.doctor_user_id,
                    operation_count: d.operation_count,
                    remark: d.remark,
                    status: "active".into(),
                },
                now,
            )
            .await
            .map_err(apply_repo)?
            .id)
    }
    async fn apply_update(
        self,
        tx: &DatabaseTransaction,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError> {
        let Self::State(d) = self else {
            return Err(ApplyError::ResourceMissing);
        };
        let repo = SalesRecordRepository::for_review_transaction();
        let u = repo
            .find_operation_usage_by_id_for_update(tx, id)
            .await
            .map_err(apply_repo)?
            .ok_or(ApplyError::ResourceMissing)?;
        let delta = (if d.status == "active" {
            d.operation_count
        } else {
            0
        }) - (if u.status == "active" {
            u.operation_count
        } else {
            0
        });
        usage_delta(&repo, tx, u.sales_record_line_id, delta, now).await?;
        repo.update_operation_usage(
            tx,
            &u,
            OperationUsageChanges {
                operated_at: Some(d.operated_at),
                operator_user_id: Some(d.operator_user_id),
                doctor_user_id: Some(d.doctor_user_id),
                operation_count: Some(d.operation_count),
                remark: Some(d.remark),
                status: Some(d.status),
            },
            now,
        )
        .await
        .map_err(apply_repo)?;
        Ok(())
    }
    async fn apply_delete(
        tx: &DatabaseTransaction,
        id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError> {
        let repo = SalesRecordRepository::for_review_transaction();
        let u = repo
            .find_operation_usage_by_id_for_update(tx, id)
            .await
            .map_err(apply_repo)?
            .ok_or(ApplyError::ResourceMissing)?;
        if u.status == "active" {
            usage_delta(&repo, tx, u.sales_record_line_id, -u.operation_count, now).await?
        }
        if !repo
            .delete_operation_usage_by_id(tx, id)
            .await
            .map_err(apply_repo)?
        {
            return Err(ApplyError::ResourceMissing);
        }
        Ok(())
    }
}

fn apply_repo(e: RepositoryError) -> ApplyError {
    match e {
        RepositoryError::Database(e) => ApplyError::Database(e),
        _ => ApplyError::ResourceMissing,
    }
}
fn review_decimal(v: &str) -> Result<Decimal, ApplyError> {
    Decimal::from_str(v).map_err(|_| ApplyError::ResourceMissing)
}
fn usage_doc(u: &sales_record_operation_usages::Model) -> SalesOperationUsageDoc {
    SalesOperationUsageDoc {
        sales_record_line_id: u.sales_record_line_id,
        operated_at: u.operated_at,
        operator_user_id: u.operator_user_id,
        doctor_user_id: u.doctor_user_id,
        operation_count: u.operation_count,
        remark: u.remark.clone(),
        status: u.status.clone(),
    }
}
fn pagination(p: Option<u64>, s: Option<u64>) -> Result<(u64, u64), SalesRecordError> {
    let p = p.unwrap_or(DEFAULT_PAGE);
    let s = s.unwrap_or(DEFAULT_SIZE);
    if p < 1 || !(1..=MAX_SIZE).contains(&s) {
        return Err(SalesRecordError::InvalidPagination);
    }
    Ok((p, s))
}
fn required_text(field: &'static str, v: String, max: usize) -> Result<String, SalesRecordError> {
    let v = v.trim();
    if v.is_empty() {
        return Err(SalesRecordError::Required(field));
    }
    if v.chars().count() > max {
        return Err(SalesRecordError::TooLong(field));
    }
    Ok(v.into())
}
fn limited_optional(v: Option<String>, max: usize) -> Result<Option<String>, SalesRecordError> {
    match v {
        None => Ok(None),
        Some(v) => {
            let v = v.trim();
            if v.is_empty() {
                Ok(None)
            } else if v.chars().count() > max {
                Err(SalesRecordError::TooLong("remark"))
            } else {
                Ok(Some(v.into()))
            }
        }
    }
}
fn parse_money(field: &'static str, v: String) -> Result<Decimal, SalesRecordError> {
    let d = Decimal::from_str(v.trim()).map_err(|_| SalesRecordError::InvalidMoney(field))?;
    if d.scale() > 2 || d < Decimal::ZERO || d > Decimal::new(999_999_999_999, 2) {
        return Err(SalesRecordError::InvalidMoney(field));
    }
    Ok(d.round_dp(2))
}
fn parse_positive_money(field: &'static str, v: String) -> Result<Decimal, SalesRecordError> {
    let d = parse_money(field, v)?;
    if d <= Decimal::ZERO {
        return Err(SalesRecordError::InvalidMoney(field));
    }
    Ok(d)
}
fn parse_ratio(v: String) -> Result<Decimal, SalesRecordError> {
    let d = Decimal::from_str(v.trim()).map_err(|_| SalesRecordError::InvalidRatio)?;
    if d.scale() > 2 || d <= Decimal::ZERO || d > Decimal::new(10000, 2) {
        return Err(SalesRecordError::InvalidRatio);
    }
    Ok(d.round_dp(2))
}
fn optional_enum(
    v: Option<String>,
    f: fn(&'static str, &str) -> Result<&'static str, EnumParseError>,
) -> Result<Option<String>, SalesRecordError> {
    match v {
        None => Ok(None),
        Some(v) if v.trim().is_empty() => Ok(None),
        Some(v) => Ok(Some(f("value", &v)?.into())),
    }
}
fn required_patch<T: Clone>(
    p: PatchField<T>,
    old: T,
    field: &'static str,
) -> Result<T, SalesRecordError> {
    match p {
        PatchField::Unset => Ok(old),
        PatchField::Value(v) => Ok(v),
        PatchField::Null => Err(SalesRecordError::Required(field)),
    }
}
fn optional_patch<T: Clone>(p: PatchField<T>, old: Option<T>) -> Option<T> {
    match p {
        PatchField::Unset => old,
        PatchField::Null => None,
        PatchField::Value(v) => Some(v),
    }
}
fn optional_text_patch(
    p: PatchField<String>,
    old: Option<String>,
) -> Result<Option<String>, SalesRecordError> {
    match p {
        PatchField::Unset => Ok(old),
        PatchField::Null => Ok(None),
        PatchField::Value(v) => limited_optional(Some(v), MAX_REMARK),
    }
}

#[derive(Debug, Error)]
pub enum SalesRecordError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("sales record not found")]
    NotFound,
    #[error("line not found")]
    LineNotFound,
    #[error("count not found")]
    CountNotFound,
    #[error("usage not found")]
    UsageNotFound,
    #[error("customer not found")]
    CustomerNotFound,
    #[error("customer disabled")]
    CustomerDisabled,
    #[error("customer scope not found")]
    SystemNotFound,
    #[error("store not found")]
    StoreNotFound,
    #[error("customer scope disabled")]
    CustomerScopeDisabled,
    #[error("product not found")]
    ProductNotFound,
    #[error("product disabled")]
    ProductDisabled,
    #[error("category not found")]
    CategoryNotFound,
    #[error("category disabled")]
    CategoryDisabled,
    #[error("user not found")]
    UserNotFound,
    #[error("user disabled")]
    UserDisabled,
    #[error("lines required")]
    LinesRequired,
    #[error("allocations required")]
    AllocationsRequired,
    #[error("duplicate guide")]
    DuplicateGuide,
    #[error("allocation ratio total must be 100")]
    AllocationTotalInvalid,
    #[error("invalid ratio")]
    InvalidRatio,
    #[error("operation count required")]
    OperationCountRequired,
    #[error("operation count not allowed")]
    OperationCountNotAllowed,
    #[error("received amount exceeds total")]
    ReceivedExceedsTotal,
    #[error("collection exceeds outstanding")]
    CollectionExceedsOutstanding,
    #[error("record voided")]
    Voided,
    #[error("usage voided")]
    UsageVoided,
    #[error("count below used")]
    CountBelowUsed,
    #[error("invalid count")]
    InvalidCount,
    #[error("invalid pagination")]
    InvalidPagination,
    #[error("{0} required")]
    Required(&'static str),
    #[error("{0} too long")]
    TooLong(&'static str),
    #[error("invalid money: {0}")]
    InvalidMoney(&'static str),
    #[error("invalid enum value")]
    InvalidEnum(#[from] EnumParseError),
}
impl SalesRecordError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "sales_record_not_found",
            Self::LineNotFound => "sales_record_line_not_found",
            Self::CountNotFound => "operation_count_not_found",
            Self::UsageNotFound => "operation_usage_not_found",
            Self::CollectionExceedsOutstanding => "collection_exceeds_outstanding",
            Self::ReceivedExceedsTotal => "received_exceeds_total",
            Self::Voided => "sales_record_voided",
            Self::UsageVoided => "operation_usage_voided",
            Self::Repository(RepositoryError::Database(_)) => "database_error",
            _ => "validation_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        dto::sales_performance::{CreatePerformanceBatchRequest, PerformanceEntriesQuery},
        repositories::{
            customers::{CustomerRepository, NewCustomer},
            product_categories::ProductCategoryRepository,
            products::{NewProduct, ProductRepository},
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        },
    };
    use chrono::NaiveDate;
    use std::path::PathBuf;

    struct Harness {
        users: UserRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
        products: ProductRepository,
        categories: ProductCategoryRepository,
        service: SalesRecordService,
    }

    impl Harness {
        async fn new() -> Self {
            let db = db::connect_and_migrate(&DatabaseConfig {
                kind: DatabaseKind::SqliteMemory,
                url: "postgres://unused".into(),
                sqlite_file: PathBuf::from("unused.sqlite"),
            })
            .await
            .expect("database should initialize");
            let users = UserRepository::new(db.clone());
            let systems = SystemRepository::new(db.clone());
            let stores = StoreRepository::new(db.clone());
            let customers = CustomerRepository::new(db.clone());
            let products = ProductRepository::new(db.clone());
            let categories = ProductCategoryRepository::new(db.clone());
            let service = SalesRecordService::new(
                SalesRecordRepository::new(db),
                customers.clone(),
                systems.clone(),
                stores.clone(),
                categories.clone(),
                users.clone(),
            );
            Self {
                users,
                systems,
                stores,
                customers,
                products,
                categories,
                service,
            }
        }

        async fn user(&self, name: &str) -> Uuid {
            self.users
                .find_or_create_for_login(name, Utc::now())
                .await
                .expect("user should create")
                .id
        }

        async fn customer(&self, actor: Uuid) -> Uuid {
            let system = self
                .systems
                .create_system(
                    NewSystem {
                        name: format!("system-{}", Uuid::new_v4()),
                        status: "active".into(),
                    },
                    Utc::now(),
                )
                .await
                .expect("system should create");
            let store = self
                .stores
                .create_store(
                    NewStore {
                        name: format!("store-{}", Uuid::new_v4()),
                        system_id: system.id,
                        status: "active".into(),
                    },
                    Utc::now(),
                )
                .await
                .expect("store should create");
            self.customers
                .create_customer(
                    NewCustomer {
                        name: "customer".into(),
                        creator_user_id: actor,
                        system_id: system.id,
                        store_id: store.id,
                        remark: None,
                        status: "active".into(),
                        attachments: None,
                    },
                    Utc::now(),
                )
                .await
                .expect("customer should create")
                .id
        }

        async fn product(&self, requires_count: bool) -> Uuid {
            let category = self
                .categories
                .list_categories(Some("active"), Some(requires_count), 1, 50)
                .await
                .expect("categories should list")
                .0
                .into_iter()
                .next()
                .expect("seed category should exist");
            self.products
                .create_product(
                    NewProduct {
                        name: format!("product-{}", Uuid::new_v4()),
                        category_id: category.id,
                        series: None,
                        brand_name: None,
                        specification: None,
                        unit: Some("次".into()),
                        unit_price: Decimal::new(10000, 2),
                        status: "active".into(),
                    },
                    Utc::now(),
                )
                .await
                .expect("product should create")
                .id
        }

        async fn apply_insert(&self, doc: SalesRecordReviewDoc) -> Uuid {
            let tx = self
                .service
                .sales_records
                .begin()
                .await
                .expect("transaction should begin");
            let id = doc
                .apply_insert(&tx, Utc::now())
                .await
                .expect("review should apply");
            tx.commit().await.expect("transaction should commit");
            id
        }

        async fn apply_update(
            &self,
            id: Uuid,
            doc: SalesRecordReviewDoc,
        ) -> Result<(), ApplyError> {
            let tx = self
                .service
                .sales_records
                .begin()
                .await
                .expect("transaction should begin");
            let result = doc.apply_update(&tx, id, Utc::now()).await;
            if result.is_ok() {
                tx.commit().await.expect("transaction should commit");
            }
            result
        }
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 8).expect("valid date")
    }

    fn line(product_id: Uuid, count: Option<i32>) -> SalesRecordLineInput {
        SalesRecordLineInput {
            product_id,
            item_name: "护理项目".into(),
            operation_total_count: count,
            remark: None,
        }
    }

    fn allocation(guide_user_id: Uuid, ratio: &str) -> SalesRecordAllocationInput {
        SalesRecordAllocationInput {
            guide_user_id,
            allocation_ratio: ratio.into(),
        }
    }

    #[tokio::test]
    async fn three_record_types_apply_and_update_customer_outstanding() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let guide = h.user("guide").await;
        let customer_id = h.customer(actor).await;
        let product_id = h.product(true).await;

        let deal_id = h
            .apply_insert(
                h.service
                    .prepare_deal_review(
                        actor,
                        CreateDealRecordRequest {
                            customer_id,
                            record_date: date(),
                            total_amount: "300.00".into(),
                            received_amount: "100.00".into(),
                            customer_type: "new".into(),
                            deal_type: "non_salon".into(),
                            handler_user_id: actor,
                            expert_user_id: None,
                            consultant_user_id: None,
                            doctor_user_id: None,
                            remark: None,
                            lines: vec![line(product_id, Some(3))],
                            allocations: vec![allocation(guide, "100.00")],
                        },
                    )
                    .await
                    .expect("deal should prepare"),
            )
            .await;
        let deal = h
            .service
            .sales_record_detail(deal_id)
            .await
            .expect("deal should load");
        assert_eq!(deal.record_type, "deal");
        assert_eq!(deal.debt_change, "200.00");
        assert_eq!(
            deal.lines[0]
                .operation_count
                .as_ref()
                .map(|v| v.total_count),
            Some(3)
        );
        assert_eq!(deal.allocations[0].allocated_amount, "100.00");

        let pre_service_id = h
            .apply_insert(
                h.service
                    .prepare_pre_service_review(
                        actor,
                        CreatePreServiceRecordRequest {
                            customer_id,
                            record_date: date(),
                            total_amount: "200.00".into(),
                            customer_type: None,
                            deal_type: None,
                            handler_user_id: actor,
                            expert_user_id: None,
                            consultant_user_id: None,
                            doctor_user_id: None,
                            remark: None,
                            lines: vec![line(product_id, Some(2))],
                        },
                    )
                    .await
                    .expect("pre-service should prepare"),
            )
            .await;
        let pre_service = h
            .service
            .sales_record_detail(pre_service_id)
            .await
            .expect("pre-service should load");
        assert_eq!(pre_service.record_type, "pre_service");
        assert_eq!(pre_service.received_amount, "0.00");
        assert!(pre_service.performance_status.is_none());
        assert_eq!(
            h.service.customer_outstanding(customer_id).await.unwrap(),
            Decimal::new(40000, 2)
        );

        let collection_id = h
            .apply_insert(
                h.service
                    .prepare_debt_collection_review(
                        actor,
                        CreateDebtCollectionRecordRequest {
                            customer_id,
                            record_date: date(),
                            received_amount: "150.00".into(),
                            handler_user_id: actor,
                            expert_user_id: None,
                            consultant_user_id: None,
                            doctor_user_id: None,
                            remark: None,
                            allocations: vec![allocation(guide, "100.00")],
                        },
                    )
                    .await
                    .expect("collection should prepare"),
            )
            .await;
        let collection = h
            .service
            .sales_record_detail(collection_id)
            .await
            .expect("collection should load");
        assert_eq!(collection.record_type, "debt_collection");
        assert_eq!(collection.total_amount, "0.00");
        assert!(collection.lines.is_empty());
        assert_eq!(collection.debt_change, "-150.00");
        assert_eq!(
            h.service.customer_outstanding(customer_id).await.unwrap(),
            Decimal::new(25000, 2)
        );
    }

    #[tokio::test]
    async fn validates_amount_lines_counts_and_allocations() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let guide = h.user("guide").await;
        let customer_id = h.customer(actor).await;
        let product_id = h.product(true).await;
        let base = CreateDealRecordRequest {
            customer_id,
            record_date: date(),
            total_amount: "100.00".into(),
            received_amount: "50.00".into(),
            customer_type: "new".into(),
            deal_type: "salon".into(),
            handler_user_id: actor,
            expert_user_id: None,
            consultant_user_id: None,
            doctor_user_id: None,
            remark: None,
            lines: vec![line(product_id, Some(1))],
            allocations: vec![allocation(guide, "100.00")],
        };

        assert!(matches!(
            h.service
                .prepare_deal_review(
                    actor,
                    CreateDealRecordRequest {
                        received_amount: "101.00".into(),
                        ..base.clone()
                    }
                )
                .await,
            Err(SalesRecordError::ReceivedExceedsTotal)
        ));
        assert!(matches!(
            h.service
                .prepare_deal_review(
                    actor,
                    CreateDealRecordRequest {
                        lines: vec![],
                        ..base.clone()
                    }
                )
                .await,
            Err(SalesRecordError::LinesRequired)
        ));
        assert!(matches!(
            h.service
                .prepare_deal_review(
                    actor,
                    CreateDealRecordRequest {
                        lines: vec![line(product_id, None)],
                        ..base.clone()
                    }
                )
                .await,
            Err(SalesRecordError::OperationCountRequired)
        ));
        assert!(matches!(
            h.service
                .prepare_deal_review(
                    actor,
                    CreateDealRecordRequest {
                        allocations: vec![allocation(guide, "90.00")],
                        ..base.clone()
                    }
                )
                .await,
            Err(SalesRecordError::AllocationTotalInvalid)
        ));
        assert!(matches!(
            h.service
                .prepare_deal_review(
                    actor,
                    CreateDealRecordRequest {
                        allocations: vec![allocation(guide, "50.00"), allocation(guide, "50.00")],
                        ..base
                    }
                )
                .await,
            Err(SalesRecordError::DuplicateGuide)
        ));
    }

    #[tokio::test]
    async fn collection_is_rechecked_on_apply_and_void_cannot_make_balance_negative() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let guide = h.user("guide").await;
        let customer_id = h.customer(actor).await;
        let product_id = h.product(false).await;
        let deal_id = h
            .apply_insert(
                h.service
                    .prepare_deal_review(
                        actor,
                        CreateDealRecordRequest {
                            customer_id,
                            record_date: date(),
                            total_amount: "100.00".into(),
                            received_amount: "10.00".into(),
                            customer_type: "returning".into(),
                            deal_type: "non_salon".into(),
                            handler_user_id: actor,
                            expert_user_id: None,
                            consultant_user_id: None,
                            doctor_user_id: None,
                            remark: None,
                            lines: vec![line(product_id, None)],
                            allocations: vec![allocation(guide, "100.00")],
                        },
                    )
                    .await
                    .expect("deal should prepare"),
            )
            .await;

        let collection_doc = h
            .service
            .prepare_debt_collection_review(
                actor,
                CreateDebtCollectionRecordRequest {
                    customer_id,
                    record_date: date(),
                    received_amount: "60.00".into(),
                    handler_user_id: actor,
                    expert_user_id: None,
                    consultant_user_id: None,
                    doctor_user_id: None,
                    remark: None,
                    allocations: vec![allocation(guide, "100.00")],
                },
            )
            .await
            .expect("collection should prepare");
        h.apply_insert(collection_doc).await;

        let stale_doc = SalesRecordReviewDoc::Create(SalesRecordCreateDoc {
            record_type: "debt_collection".into(),
            customer_id,
            record_date: date(),
            total_amount: "0.00".into(),
            received_amount: "60.00".into(),
            customer_type: None,
            deal_type: None,
            system_id: h
                .service
                .sales_record_detail(deal_id)
                .await
                .unwrap()
                .system_id,
            store_id: h
                .service
                .sales_record_detail(deal_id)
                .await
                .unwrap()
                .store_id,
            handler_user_id: actor,
            expert_user_id: None,
            consultant_user_id: None,
            doctor_user_id: None,
            remark: None,
            created_by_user_id: actor,
            lines: vec![],
            allocations: vec![SalesRecordAllocationDoc {
                guide_user_id: guide,
                allocation_ratio: "100.00".into(),
                allocated_amount: "60.00".into(),
            }],
        });
        let tx = h.service.sales_records.begin().await.unwrap();
        assert!(stale_doc.apply_insert(&tx, Utc::now()).await.is_err());
        drop(tx);

        assert!(
            h.apply_update(
                deal_id,
                SalesRecordReviewDoc::Status {
                    status: "voided".into()
                }
            )
            .await
            .is_err()
        );
        assert_eq!(
            h.service.sales_record_detail(deal_id).await.unwrap().status,
            "active"
        );
    }

    #[tokio::test]
    async fn posted_deal_creates_expert_and_guide_performance_then_voids_with_reversal() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let guide = h.user("guide").await;
        let expert = h.user("expert").await;
        let customer_id = h.customer(actor).await;
        let product_id = h.product(false).await;
        let deal_id = h
            .apply_insert(
                h.service
                    .prepare_deal_review(
                        actor,
                        CreateDealRecordRequest {
                            customer_id,
                            record_date: date(),
                            total_amount: "100.00".into(),
                            received_amount: "100.00".into(),
                            customer_type: "new".into(),
                            deal_type: "salon".into(),
                            handler_user_id: actor,
                            expert_user_id: Some(expert),
                            consultant_user_id: None,
                            doctor_user_id: None,
                            remark: None,
                            lines: vec![line(product_id, None)],
                            allocations: vec![allocation(guide, "100.00")],
                        },
                    )
                    .await
                    .expect("deal should prepare"),
            )
            .await;

        let batch = h
            .service
            .performance_service()
            .post_batch(
                actor,
                CreatePerformanceBatchRequest {
                    period_month: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                    sales_record_ids: vec![deal_id],
                },
            )
            .await
            .expect("performance should post");
        assert_eq!(batch.expert_amount, "100.00");
        assert_eq!(batch.guide_amount, "100.00");
        assert_eq!(batch.total_amount, "200.00");

        h.apply_update(
            deal_id,
            SalesRecordReviewDoc::Status {
                status: "voided".into(),
            },
        )
        .await
        .expect("deal should void");
        let entries = h
            .service
            .performance_service()
            .entries(PerformanceEntriesQuery {
                performance_date_from: Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
                performance_date_to: Some(NaiveDate::from_ymd_opt(2026, 7, 31).unwrap()),
                user_id: None,
                performance_role: None,
                system_id: None,
                store_id: None,
                entry_type: None,
                sales_record_id: Some(deal_id),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("entries should list");
        assert_eq!(entries.total_count, 4);
        assert_eq!(
            entries
                .entries
                .iter()
                .filter(|entry| entry.entry_type == "earning")
                .count(),
            2
        );
        assert_eq!(
            entries
                .entries
                .iter()
                .filter(|entry| entry.entry_type == "reversal")
                .count(),
            2
        );
    }
}

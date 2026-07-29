use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    entities::{
        customers, sales_record_allocations, sales_record_lines, sales_record_operation_counts,
        sales_record_operation_usages, sales_records,
    },
    repositories::RepositoryError,
};

#[derive(Clone)]
pub struct SalesRecordRepository {
    pub(crate) db: DatabaseConnection,
}
pub struct NewSalesRecord {
    pub record_type: String,
    pub customer_id: Uuid,
    pub record_date: NaiveDate,
    pub total_amount: Decimal,
    pub received_amount: Decimal,
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
}
pub struct NewSalesRecordLine {
    pub sales_record_id: Uuid,
    pub product_id: Uuid,
    pub item_name: String,
    pub operation_total_count: Option<i32>,
    pub remark: Option<String>,
    pub status: String,
}
pub struct NewSalesRecordAllocation {
    pub sales_record_id: Uuid,
    pub guide_user_id: Uuid,
    pub allocation_ratio: Decimal,
    pub allocated_amount: Decimal,
}
pub struct NewOperationCount {
    pub sales_record_line_id: Uuid,
    pub total_count: i32,
    pub used_count: i32,
    pub status: String,
}
pub struct NewOperationUsage {
    pub sales_record_line_id: Uuid,
    pub operated_at: DateTime<Utc>,
    pub operator_user_id: Uuid,
    pub doctor_user_id: Option<Uuid>,
    pub operation_count: i32,
    pub remark: Option<String>,
    pub status: String,
}
#[derive(Default)]
pub struct OperationCountChanges {
    pub total_count: Option<i32>,
    pub used_count: Option<i32>,
    pub status: Option<String>,
}
#[derive(Default)]
pub struct OperationUsageChanges {
    pub operated_at: Option<DateTime<Utc>>,
    pub operator_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Option<Uuid>>,
    pub operation_count: Option<i32>,
    pub remark: Option<Option<String>>,
    pub status: Option<String>,
}
pub struct SalesRecordFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub record_type: Option<&'a str>,
    pub customer_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub handler_user_id: Option<Uuid>,
    pub record_date_from: Option<NaiveDate>,
    pub record_date_to: Option<NaiveDate>,
}
pub struct OperationCountFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub sales_record_line_id: Option<Uuid>,
    pub sales_record_id: Option<Uuid>,
}
pub struct OperationUsageFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub sales_record_line_id: Option<Uuid>,
    pub sales_record_id: Option<Uuid>,
    pub operator_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub operated_at_from: Option<DateTime<Utc>>,
    pub operated_at_to: Option<DateTime<Utc>>,
}

impl SalesRecordRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
    pub(crate) fn for_review_transaction() -> Self {
        Self {
            db: DatabaseConnection::Disconnected,
        }
    }
    pub async fn begin(&self) -> Result<DatabaseTransaction, RepositoryError> {
        Ok(self.db.begin().await?)
    }
    pub async fn lock_customer<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
    ) -> Result<Option<customers::Model>, RepositoryError> {
        Ok(customers::Entity::find_by_id(id)
            .lock_exclusive()
            .one(c)
            .await?)
    }
    pub async fn customer_outstanding<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
        excluding: Option<Uuid>,
    ) -> Result<Decimal, RepositoryError> {
        let mut q = sales_records::Entity::find()
            .filter(sales_records::Column::CustomerId.eq(id))
            .filter(sales_records::Column::Status.eq("active"));
        if let Some(id) = excluding {
            q = q.filter(sales_records::Column::Id.ne(id));
        }
        let rows = q.all(c).await?;
        Ok(rows
            .into_iter()
            .map(|r| r.total_amount - r.received_amount)
            .sum())
    }
    pub async fn insert_sales_record<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewSalesRecord,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        Ok(sales_records::ActiveModel {
            id: Set(Uuid::new_v4()),
            record_type: Set(v.record_type),
            customer_id: Set(v.customer_id),
            record_date: Set(v.record_date),
            total_amount: Set(v.total_amount),
            received_amount: Set(v.received_amount),
            performance_status: Set(v.performance_status),
            customer_type: Set(v.customer_type),
            deal_type: Set(v.deal_type),
            system_id: Set(v.system_id),
            store_id: Set(v.store_id),
            handler_user_id: Set(v.handler_user_id),
            expert_user_id: Set(v.expert_user_id),
            consultant_user_id: Set(v.consultant_user_id),
            doctor_user_id: Set(v.doctor_user_id),
            remark: Set(v.remark),
            status: Set(v.status),
            created_by_user_id: Set(v.created_by_user_id),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn insert_sales_record_line<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewSalesRecordLine,
        now: DateTime<Utc>,
    ) -> Result<sales_record_lines::Model, RepositoryError> {
        Ok(sales_record_lines::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_id: Set(v.sales_record_id),
            product_id: Set(v.product_id),
            item_name: Set(v.item_name),
            operation_total_count: Set(v.operation_total_count),
            remark: Set(v.remark),
            status: Set(v.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn insert_allocation<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewSalesRecordAllocation,
        now: DateTime<Utc>,
    ) -> Result<sales_record_allocations::Model, RepositoryError> {
        Ok(sales_record_allocations::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_id: Set(v.sales_record_id),
            guide_user_id: Set(v.guide_user_id),
            allocation_ratio: Set(v.allocation_ratio),
            allocated_amount: Set(v.allocated_amount),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn find_sales_record_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<sales_records::Model>, RepositoryError> {
        Ok(sales_records::Entity::find_by_id(id).one(&self.db).await?)
    }
    pub async fn find_sales_record_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        id: Uuid,
    ) -> Result<Option<sales_records::Model>, RepositoryError> {
        Ok(sales_records::Entity::find_by_id(id)
            .lock_exclusive()
            .one(tx)
            .await?)
    }
    pub async fn list_sales_records(
        &self,
        f: SalesRecordFilters<'_>,
        page: u64,
        size: u64,
    ) -> Result<(Vec<sales_records::Model>, u64), RepositoryError> {
        let mut q = sales_records::Entity::find()
            .order_by_desc(sales_records::Column::RecordDate)
            .order_by_desc(sales_records::Column::CreatedAt);
        if let Some(v) = f.status_filter {
            q = q.filter(sales_records::Column::Status.eq(v));
        }
        if let Some(v) = f.record_type {
            q = q.filter(sales_records::Column::RecordType.eq(v));
        }
        if let Some(v) = f.customer_id {
            q = q.filter(sales_records::Column::CustomerId.eq(v));
        }
        if let Some(v) = f.system_id {
            q = q.filter(sales_records::Column::SystemId.eq(v));
        }
        if let Some(v) = f.store_id {
            q = q.filter(sales_records::Column::StoreId.eq(v));
        }
        if let Some(v) = f.handler_user_id {
            q = q.filter(sales_records::Column::HandlerUserId.eq(v));
        }
        if let Some(v) = f.record_date_from {
            q = q.filter(sales_records::Column::RecordDate.gte(v));
        }
        if let Some(v) = f.record_date_to {
            q = q.filter(sales_records::Column::RecordDate.lte(v));
        }
        let p = q.paginate(&self.db, size);
        Ok((
            p.fetch_page(page.saturating_sub(1)).await?,
            p.num_items().await?,
        ))
    }
    pub async fn update_sales_record_status<C: ConnectionTrait>(
        &self,
        c: &C,
        v: &sales_records::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        let mut a: sales_records::ActiveModel = v.clone().into();
        a.status = Set(status.into());
        a.updated_at = Set(now);
        Ok(a.update(c).await?)
    }
    pub async fn update_performance_status<C: ConnectionTrait>(
        &self,
        c: &C,
        v: &sales_records::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        let mut a: sales_records::ActiveModel = v.clone().into();
        a.performance_status = Set(Some(status.into()));
        a.updated_at = Set(now);
        Ok(a.update(c).await?)
    }
    pub async fn find_lines_by_sales_record_ids(
        &self,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_lines::Model>, RepositoryError> {
        Ok(sales_record_lines::Entity::find()
            .filter(sales_record_lines::Column::SalesRecordId.is_in(ids))
            .order_by_asc(sales_record_lines::Column::CreatedAt)
            .all(&self.db)
            .await?)
    }
    pub async fn find_lines_by_sales_record_id_in<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
    ) -> Result<Vec<sales_record_lines::Model>, RepositoryError> {
        Ok(sales_record_lines::Entity::find()
            .filter(sales_record_lines::Column::SalesRecordId.eq(id))
            .all(c)
            .await?)
    }
    pub async fn find_line_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<sales_record_lines::Model>, RepositoryError> {
        Ok(sales_record_lines::Entity::find_by_id(id)
            .one(&self.db)
            .await?)
    }
    pub async fn find_line_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        id: Uuid,
    ) -> Result<Option<sales_record_lines::Model>, RepositoryError> {
        Ok(sales_record_lines::Entity::find_by_id(id)
            .lock_exclusive()
            .one(tx)
            .await?)
    }
    pub async fn update_line_status<C: ConnectionTrait>(
        &self,
        c: &C,
        v: &sales_record_lines::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut a: sales_record_lines::ActiveModel = v.clone().into();
        a.status = Set(status.into());
        a.updated_at = Set(now);
        a.update(c).await?;
        Ok(())
    }
    pub async fn find_allocations_by_record_ids<C: ConnectionTrait>(
        &self,
        c: &C,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_allocations::Model>, RepositoryError> {
        Ok(sales_record_allocations::Entity::find()
            .filter(sales_record_allocations::Column::SalesRecordId.is_in(ids))
            .order_by_asc(sales_record_allocations::Column::CreatedAt)
            .all(c)
            .await?)
    }
    pub async fn insert_operation_count<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewOperationCount,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_counts::Model, RepositoryError> {
        Ok(sales_record_operation_counts::ActiveModel {
            sales_record_line_id: Set(v.sales_record_line_id),
            total_count: Set(v.total_count),
            used_count: Set(v.used_count),
            status: Set(v.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn find_operation_count(
        &self,
        id: Uuid,
    ) -> Result<Option<sales_record_operation_counts::Model>, RepositoryError> {
        Ok(sales_record_operation_counts::Entity::find_by_id(id)
            .one(&self.db)
            .await?)
    }
    pub async fn find_operation_count_for_update(
        &self,
        tx: &DatabaseTransaction,
        id: Uuid,
    ) -> Result<Option<sales_record_operation_counts::Model>, RepositoryError> {
        Ok(sales_record_operation_counts::Entity::find_by_id(id)
            .lock_exclusive()
            .one(tx)
            .await?)
    }
    pub async fn find_operation_counts_by_line_ids_in<C: ConnectionTrait>(
        &self,
        c: &C,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_operation_counts::Model>, RepositoryError> {
        Ok(sales_record_operation_counts::Entity::find()
            .filter(sales_record_operation_counts::Column::SalesRecordLineId.is_in(ids))
            .all(c)
            .await?)
    }
    pub async fn list_operation_counts(
        &self,
        f: OperationCountFilters<'_>,
        page: u64,
        size: u64,
    ) -> Result<(Vec<sales_record_operation_counts::Model>, u64), RepositoryError> {
        let mut q = sales_record_operation_counts::Entity::find();
        if let Some(v) = f.status_filter {
            q = q.filter(sales_record_operation_counts::Column::Status.eq(v));
        }
        if let Some(v) = f.sales_record_line_id {
            q = q.filter(sales_record_operation_counts::Column::SalesRecordLineId.eq(v));
        }
        if let Some(v) = f.sales_record_id {
            q = q
                .inner_join(sales_record_lines::Entity)
                .filter(sales_record_lines::Column::SalesRecordId.eq(v));
        }
        let p = q.paginate(&self.db, size);
        Ok((p.fetch_page(page - 1).await?, p.num_items().await?))
    }
    pub async fn update_operation_count<C: ConnectionTrait>(
        &self,
        c: &C,
        v: &sales_record_operation_counts::Model,
        ch: OperationCountChanges,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_counts::Model, RepositoryError> {
        let mut a: sales_record_operation_counts::ActiveModel = v.clone().into();
        if let Some(x) = ch.total_count {
            a.total_count = Set(x)
        }
        if let Some(x) = ch.used_count {
            a.used_count = Set(x)
        }
        if let Some(x) = ch.status {
            a.status = Set(x)
        }
        a.updated_at = Set(now);
        Ok(a.update(c).await?)
    }
    pub async fn insert_operation_usage<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewOperationUsage,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_usages::Model, RepositoryError> {
        Ok(sales_record_operation_usages::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_line_id: Set(v.sales_record_line_id),
            operated_at: Set(v.operated_at),
            operator_user_id: Set(v.operator_user_id),
            doctor_user_id: Set(v.doctor_user_id),
            operation_count: Set(v.operation_count),
            remark: Set(v.remark),
            status: Set(v.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn find_operation_usage_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<sales_record_operation_usages::Model>, RepositoryError> {
        Ok(sales_record_operation_usages::Entity::find_by_id(id)
            .one(&self.db)
            .await?)
    }
    pub async fn find_operation_usage_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        id: Uuid,
    ) -> Result<Option<sales_record_operation_usages::Model>, RepositoryError> {
        Ok(sales_record_operation_usages::Entity::find_by_id(id)
            .lock_exclusive()
            .one(tx)
            .await?)
    }
    pub async fn list_operation_usages(
        &self,
        f: OperationUsageFilters<'_>,
        page: u64,
        size: u64,
    ) -> Result<(Vec<sales_record_operation_usages::Model>, u64), RepositoryError> {
        let mut q = sales_record_operation_usages::Entity::find();
        if let Some(v) = f.status_filter {
            q = q.filter(sales_record_operation_usages::Column::Status.eq(v));
        }
        if let Some(v) = f.sales_record_line_id {
            q = q.filter(sales_record_operation_usages::Column::SalesRecordLineId.eq(v));
        }
        if let Some(v) = f.sales_record_id {
            q = q
                .inner_join(sales_record_lines::Entity)
                .filter(sales_record_lines::Column::SalesRecordId.eq(v));
        }
        if let Some(v) = f.operator_user_id {
            q = q.filter(sales_record_operation_usages::Column::OperatorUserId.eq(v));
        }
        if let Some(v) = f.doctor_user_id {
            q = q.filter(sales_record_operation_usages::Column::DoctorUserId.eq(v));
        }
        if let Some(v) = f.operated_at_from {
            q = q.filter(sales_record_operation_usages::Column::OperatedAt.gte(v));
        }
        if let Some(v) = f.operated_at_to {
            q = q.filter(sales_record_operation_usages::Column::OperatedAt.lte(v));
        }
        let p = q.paginate(&self.db, size);
        Ok((p.fetch_page(page - 1).await?, p.num_items().await?))
    }
    pub async fn count_active_operation_usages_for_lines<C: ConnectionTrait>(
        &self,
        c: &C,
        ids: Vec<Uuid>,
    ) -> Result<u64, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }
        Ok(sales_record_operation_usages::Entity::find()
            .filter(sales_record_operation_usages::Column::SalesRecordLineId.is_in(ids))
            .filter(sales_record_operation_usages::Column::Status.eq("active"))
            .count(c)
            .await?)
    }
    pub async fn update_operation_usage<C: ConnectionTrait>(
        &self,
        c: &C,
        v: &sales_record_operation_usages::Model,
        ch: OperationUsageChanges,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_usages::Model, RepositoryError> {
        let mut a: sales_record_operation_usages::ActiveModel = v.clone().into();
        if let Some(x) = ch.operated_at {
            a.operated_at = Set(x)
        }
        if let Some(x) = ch.operator_user_id {
            a.operator_user_id = Set(x)
        }
        if let Some(x) = ch.doctor_user_id {
            a.doctor_user_id = Set(x)
        }
        if let Some(x) = ch.operation_count {
            a.operation_count = Set(x)
        }
        if let Some(x) = ch.remark {
            a.remark = Set(x)
        }
        if let Some(x) = ch.status {
            a.status = Set(x)
        }
        a.updated_at = Set(now);
        Ok(a.update(c).await?)
    }
    pub async fn delete_operation_usage_by_id<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
    ) -> Result<bool, RepositoryError> {
        Ok(sales_record_operation_usages::Entity::delete_by_id(id)
            .exec(c)
            .await?
            .rows_affected
            > 0)
    }
}

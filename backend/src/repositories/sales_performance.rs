use crate::{
    entities::{
        sales_performance_batches, sales_performance_entries, sales_record_allocations,
        sales_records,
    },
    repositories::RepositoryError,
};
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;
#[derive(Clone)]
pub struct SalesPerformanceRepository {
    pub(crate) db: DatabaseConnection,
}
pub struct NewPerformanceBatch {
    pub period_month: NaiveDate,
    pub batch_type: String,
    pub record_count: i32,
    pub expert_amount: Decimal,
    pub guide_amount: Decimal,
    pub total_amount: Decimal,
    pub posted_by_user_id: Option<Uuid>,
}
pub struct NewPerformanceEntry {
    pub batch_id: Uuid,
    pub record_allocation_id: Option<Uuid>,
    pub allocation_ratio: Option<Decimal>,
    pub sales_record_id: Uuid,
    pub user_id: Uuid,
    pub performance_role: String,
    pub entry_type: String,
    pub amount: Decimal,
    pub period_month: NaiveDate,
    pub performance_date: NaiveDate,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub source_entry_id: Option<Uuid>,
}
#[derive(Clone)]
pub struct PerformanceFilters {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub user_id: Option<Uuid>,
    pub performance_role: Option<String>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub entry_type: Option<String>,
    pub sales_record_id: Option<Uuid>,
}
impl SalesPerformanceRepository {
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
    pub async fn list_pending_records(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        page: u64,
        size: u64,
    ) -> Result<(Vec<sales_records::Model>, u64), RepositoryError> {
        let q = sales_records::Entity::find()
            .filter(sales_records::Column::Status.eq("active"))
            .filter(sales_records::Column::PerformanceStatus.eq("pending"))
            .filter(sales_records::Column::RecordDate.gte(from))
            .filter(sales_records::Column::RecordDate.lt(to))
            .order_by_asc(sales_records::Column::RecordDate);
        let p = q.paginate(&self.db, size);
        Ok((p.fetch_page(page - 1).await?, p.num_items().await?))
    }
    pub async fn find_records(
        &self,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_records::Model>, RepositoryError> {
        Ok(sales_records::Entity::find()
            .filter(sales_records::Column::Id.is_in(ids))
            .all(&self.db)
            .await?)
    }
    pub async fn find_records_for_update(
        &self,
        tx: &DatabaseTransaction,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_records::Model>, RepositoryError> {
        Ok(sales_records::Entity::find()
            .filter(sales_records::Column::Id.is_in(ids))
            .lock_exclusive()
            .all(tx)
            .await?)
    }
    pub async fn find_record_in<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
    ) -> Result<Option<sales_records::Model>, RepositoryError> {
        Ok(sales_records::Entity::find_by_id(id).one(c).await?)
    }
    pub async fn find_allocations_in<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
    ) -> Result<Vec<sales_record_allocations::Model>, RepositoryError> {
        Ok(sales_record_allocations::Entity::find()
            .filter(sales_record_allocations::Column::SalesRecordId.eq(id))
            .all(c)
            .await?)
    }
    pub async fn insert_batch<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewPerformanceBatch,
        now: DateTime<Utc>,
    ) -> Result<sales_performance_batches::Model, RepositoryError> {
        Ok(sales_performance_batches::ActiveModel {
            id: Set(Uuid::new_v4()),
            period_month: Set(v.period_month),
            batch_type: Set(v.batch_type),
            record_count: Set(v.record_count),
            expert_amount: Set(v.expert_amount),
            guide_amount: Set(v.guide_amount),
            total_amount: Set(v.total_amount),
            posted_by_user_id: Set(v.posted_by_user_id),
            posted_at: Set(now),
            created_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn insert_entry<C: ConnectionTrait>(
        &self,
        c: &C,
        v: NewPerformanceEntry,
        now: DateTime<Utc>,
    ) -> Result<sales_performance_entries::Model, RepositoryError> {
        Ok(sales_performance_entries::ActiveModel {
            id: Set(Uuid::new_v4()),
            batch_id: Set(v.batch_id),
            record_allocation_id: Set(v.record_allocation_id),
            allocation_ratio: Set(v.allocation_ratio),
            sales_record_id: Set(v.sales_record_id),
            user_id: Set(v.user_id),
            performance_role: Set(v.performance_role),
            entry_type: Set(v.entry_type),
            amount: Set(v.amount),
            period_month: Set(v.period_month),
            performance_date: Set(v.performance_date),
            system_id: Set(v.system_id),
            store_id: Set(v.store_id),
            source_entry_id: Set(v.source_entry_id),
            created_at: Set(now),
        }
        .insert(c)
        .await?)
    }
    pub async fn update_record_status<C: ConnectionTrait>(
        &self,
        c: &C,
        r: &sales_records::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut a: sales_records::ActiveModel = r.clone().into();
        a.performance_status = Set(Some(status.into()));
        a.updated_at = Set(now);
        a.update(c).await?;
        Ok(())
    }
    pub async fn earning_entries_for_record<C: ConnectionTrait>(
        &self,
        c: &C,
        id: Uuid,
    ) -> Result<Vec<sales_performance_entries::Model>, RepositoryError> {
        Ok(sales_performance_entries::Entity::find()
            .filter(sales_performance_entries::Column::SalesRecordId.eq(id))
            .filter(sales_performance_entries::Column::EntryType.eq("earning"))
            .all(c)
            .await?)
    }
    pub async fn list_batches(
        &self,
        month: Option<NaiveDate>,
        page: u64,
        size: u64,
    ) -> Result<(Vec<sales_performance_batches::Model>, u64), RepositoryError> {
        let mut q = sales_performance_batches::Entity::find()
            .order_by_desc(sales_performance_batches::Column::PostedAt);
        if let Some(m) = month {
            q = q.filter(sales_performance_batches::Column::PeriodMonth.eq(m));
        }
        let p = q.paginate(&self.db, size);
        Ok((p.fetch_page(page - 1).await?, p.num_items().await?))
    }
    fn entry_query(f: &PerformanceFilters) -> sea_orm::Select<sales_performance_entries::Entity> {
        let mut q = sales_performance_entries::Entity::find()
            .filter(sales_performance_entries::Column::PerformanceDate.gte(f.from))
            .filter(sales_performance_entries::Column::PerformanceDate.lte(f.to));
        if let Some(v) = f.user_id {
            q = q.filter(sales_performance_entries::Column::UserId.eq(v));
        }
        if let Some(ref v) = f.performance_role {
            q = q.filter(sales_performance_entries::Column::PerformanceRole.eq(v));
        }
        if let Some(v) = f.system_id {
            q = q.filter(sales_performance_entries::Column::SystemId.eq(v));
        }
        if let Some(v) = f.store_id {
            q = q.filter(sales_performance_entries::Column::StoreId.eq(v));
        }
        if let Some(ref v) = f.entry_type {
            q = q.filter(sales_performance_entries::Column::EntryType.eq(v));
        }
        if let Some(v) = f.sales_record_id {
            q = q.filter(sales_performance_entries::Column::SalesRecordId.eq(v));
        }
        q
    }
    pub async fn list_entries(
        &self,
        f: PerformanceFilters,
        page: u64,
        size: u64,
    ) -> Result<(Vec<sales_performance_entries::Model>, u64), RepositoryError> {
        let p = Self::entry_query(&f)
            .order_by_desc(sales_performance_entries::Column::PerformanceDate)
            .paginate(&self.db, size);
        Ok((p.fetch_page(page - 1).await?, p.num_items().await?))
    }
    pub async fn all_entries(
        &self,
        f: PerformanceFilters,
    ) -> Result<Vec<sales_performance_entries::Model>, RepositoryError> {
        Ok(Self::entry_query(&f).all(&self.db).await?)
    }
}

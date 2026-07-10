use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    entities::{
        sales_payment_allocations, sales_payments, sales_performance_batches,
        sales_performance_entries, sales_records,
    },
    repositories::RepositoryError,
};

#[derive(Clone)]
pub struct SalesPerformanceRepository {
    pub(crate) db: DatabaseConnection,
}

pub struct NewPerformanceBatch {
    pub period_month: NaiveDate,
    pub batch_type: String,
    pub payment_count: i32,
    pub expert_amount: Decimal,
    pub guide_amount: Decimal,
    pub total_amount: Decimal,
    pub posted_by_user_id: Option<Uuid>,
}

pub struct NewPerformanceEntry {
    pub batch_id: Uuid,
    pub payment_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub allocation_ratio: Option<Decimal>,
    pub sales_record_id: Uuid,
    pub user_id: Uuid,
    pub performance_role: String,
    pub entry_type: String,
    pub amount: Decimal,
    pub period_month: NaiveDate,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub source_entry_id: Option<Uuid>,
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

    pub async fn list_pending_payments(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<sales_payments::Model>, u64), RepositoryError> {
        let query = sales_payments::Entity::find()
            .filter(sales_payments::Column::Status.eq("active"))
            .filter(sales_payments::Column::PerformanceStatus.eq("pending"))
            .filter(sales_payments::Column::PaidAt.gte(from))
            .filter(sales_payments::Column::PaidAt.lt(to))
            .order_by_asc(sales_payments::Column::PaidAt)
            .order_by_asc(sales_payments::Column::Id);
        let paginator = query.paginate(&self.db, page_size);
        let total = paginator.num_items().await?;
        Ok((
            paginator.fetch_page(page_number.saturating_sub(1)).await?,
            total,
        ))
    }

    pub async fn find_payments_for_update(
        &self,
        tx: &DatabaseTransaction,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_payments::Model>, RepositoryError> {
        Ok(sales_payments::Entity::find()
            .filter(sales_payments::Column::Id.is_in(ids))
            .lock_exclusive()
            .all(tx)
            .await?)
    }

    pub async fn find_payments(
        &self,
        ids: Vec<Uuid>,
    ) -> Result<Vec<sales_payments::Model>, RepositoryError> {
        Ok(sales_payments::Entity::find()
            .filter(sales_payments::Column::Id.is_in(ids))
            .all(&self.db)
            .await?)
    }

    pub async fn lock_records(
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
        conn: &C,
        id: Uuid,
    ) -> Result<Option<sales_records::Model>, RepositoryError> {
        Ok(sales_records::Entity::find_by_id(id).one(conn).await?)
    }

    pub async fn find_allocations_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        payment_id: Uuid,
    ) -> Result<Vec<sales_payment_allocations::Model>, RepositoryError> {
        Ok(sales_payment_allocations::Entity::find()
            .filter(sales_payment_allocations::Column::PaymentId.eq(payment_id))
            .order_by_asc(sales_payment_allocations::Column::CreatedAt)
            .all(conn)
            .await?)
    }

    pub async fn insert_batch<C: ConnectionTrait>(
        &self,
        conn: &C,
        batch: NewPerformanceBatch,
        now: DateTime<Utc>,
    ) -> Result<sales_performance_batches::Model, RepositoryError> {
        Ok(sales_performance_batches::ActiveModel {
            id: Set(Uuid::new_v4()),
            period_month: Set(batch.period_month),
            batch_type: Set(batch.batch_type),
            payment_count: Set(batch.payment_count),
            expert_amount: Set(batch.expert_amount),
            guide_amount: Set(batch.guide_amount),
            total_amount: Set(batch.total_amount),
            posted_by_user_id: Set(batch.posted_by_user_id),
            posted_at: Set(now),
            created_at: Set(now),
        }
        .insert(conn)
        .await?)
    }

    pub async fn insert_entry<C: ConnectionTrait>(
        &self,
        conn: &C,
        entry: NewPerformanceEntry,
        now: DateTime<Utc>,
    ) -> Result<sales_performance_entries::Model, RepositoryError> {
        Ok(sales_performance_entries::ActiveModel {
            id: Set(Uuid::new_v4()),
            batch_id: Set(entry.batch_id),
            payment_id: Set(entry.payment_id),
            allocation_id: Set(entry.allocation_id),
            allocation_ratio: Set(entry.allocation_ratio),
            sales_record_id: Set(entry.sales_record_id),
            user_id: Set(entry.user_id),
            performance_role: Set(entry.performance_role),
            entry_type: Set(entry.entry_type),
            amount: Set(entry.amount),
            period_month: Set(entry.period_month),
            system_id: Set(entry.system_id),
            store_id: Set(entry.store_id),
            source_entry_id: Set(entry.source_entry_id),
            created_at: Set(now),
        }
        .insert(conn)
        .await?)
    }

    pub async fn update_payment_performance_status<C: ConnectionTrait>(
        &self,
        conn: &C,
        payment: &sales_payments::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<sales_payments::Model, RepositoryError> {
        let mut active: sales_payments::ActiveModel = payment.clone().into();
        active.performance_status = Set(status.to_string());
        active.updated_at = Set(now);
        Ok(active.update(conn).await?)
    }

    pub async fn earning_entries_for_payment<C: ConnectionTrait>(
        &self,
        conn: &C,
        payment_id: Uuid,
    ) -> Result<Vec<sales_performance_entries::Model>, RepositoryError> {
        Ok(sales_performance_entries::Entity::find()
            .filter(sales_performance_entries::Column::PaymentId.eq(payment_id))
            .filter(sales_performance_entries::Column::EntryType.eq("earning"))
            .all(conn)
            .await?)
    }

    pub async fn list_batches(
        &self,
        month: Option<NaiveDate>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<sales_performance_batches::Model>, u64), RepositoryError> {
        let mut query = sales_performance_batches::Entity::find()
            .order_by_desc(sales_performance_batches::Column::PostedAt);
        if let Some(month) = month {
            query = query.filter(sales_performance_batches::Column::PeriodMonth.eq(month));
        }
        let paginator = query.paginate(&self.db, page_size);
        let total = paginator.num_items().await?;
        Ok((
            paginator.fetch_page(page_number.saturating_sub(1)).await?,
            total,
        ))
    }

    pub async fn list_entries(
        &self,
        month: NaiveDate,
        user_id: Option<Uuid>,
        payment_id: Option<Uuid>,
    ) -> Result<Vec<sales_performance_entries::Model>, RepositoryError> {
        let mut query = sales_performance_entries::Entity::find()
            .filter(sales_performance_entries::Column::PeriodMonth.eq(month))
            .order_by_desc(sales_performance_entries::Column::CreatedAt)
            .order_by_asc(sales_performance_entries::Column::Id);
        if let Some(user_id) = user_id {
            query = query.filter(sales_performance_entries::Column::UserId.eq(user_id));
        }
        if let Some(payment_id) = payment_id {
            query = query.filter(sales_performance_entries::Column::PaymentId.eq(payment_id));
        }
        Ok(query.all(&self.db).await?)
    }
}

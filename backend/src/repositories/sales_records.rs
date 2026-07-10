use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    entities::{
        sales_payment_allocations, sales_payments, sales_record_lines,
        sales_record_operation_counts, sales_record_operation_usages, sales_records,
    },
    repositories::RepositoryError,
};

#[derive(Clone)]
pub struct SalesRecordRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewSalesRecord {
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
}

#[derive(Debug, Clone)]
pub struct NewSalesRecordLine {
    pub sales_record_id: Uuid,
    pub product_id: Uuid,
    pub item_name: String,
    pub receivable_amount: Decimal,
    pub operation_total_count: Option<i32>,
    pub remark: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct NewSalesPayment {
    pub sales_record_id: Uuid,
    pub payment_type: String,
    pub paid_amount: Decimal,
    pub paid_at: DateTime<Utc>,
    pub performance_status: String,
    pub status: String,
    pub remark: Option<String>,
    pub created_by_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct NewSalesPaymentAllocation {
    pub payment_id: Uuid,
    pub guide_user_id: Uuid,
    pub allocation_ratio: Decimal,
    pub allocated_amount: Decimal,
}

#[derive(Debug, Clone, Copy, Default)]
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

#[derive(Debug, Clone, Copy, Default)]
pub struct SalesPaymentFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub payment_type: Option<&'a str>,
    pub sales_record_id: Option<Uuid>,
    pub paid_at_from: Option<DateTime<Utc>>,
    pub paid_at_to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewOperationCount {
    pub sales_record_line_id: Uuid,
    pub total_count: i32,
    pub used_count: i32,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct OperationCountChanges {
    pub total_count: Option<i32>,
    pub used_count: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OperationCountFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub sales_record_line_id: Option<Uuid>,
    pub sales_record_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct NewOperationUsage {
    pub sales_record_line_id: Uuid,
    pub operated_at: DateTime<Utc>,
    pub operator_user_id: Uuid,
    pub doctor_user_id: Option<Uuid>,
    pub operation_count: i32,
    pub remark: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct OperationUsageChanges {
    pub operated_at: Option<DateTime<Utc>>,
    pub operator_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Option<Uuid>>,
    pub operation_count: Option<i32>,
    pub remark: Option<Option<String>>,
    pub status: Option<String>,
}

impl OperationUsageChanges {
    pub fn is_empty(&self) -> bool {
        self.operated_at.is_none()
            && self.operator_user_id.is_none()
            && self.doctor_user_id.is_none()
            && self.operation_count.is_none()
            && self.remark.is_none()
            && self.status.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default)]
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

    /// Repository facade for review appliers that are given an existing
    /// transaction by the event system. Only methods accepting an explicit
    /// connection/transaction may be used on this value.
    pub(crate) fn for_review_transaction() -> Self {
        Self {
            db: DatabaseConnection::Disconnected,
        }
    }

    pub async fn begin(&self) -> Result<DatabaseTransaction, RepositoryError> {
        Ok(self.db.begin().await?)
    }

    #[tracing::instrument(level = "info", skip(self, conn, record), fields(customer_id = %record.customer_id, record_type = %record.record_type))]
    pub async fn insert_sales_record<C: ConnectionTrait>(
        &self,
        conn: &C,
        record: NewSalesRecord,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        validate_required("record_type", &record.record_type)?;
        validate_required("status", &record.status)?;

        let record = sales_records::ActiveModel {
            id: Set(Uuid::new_v4()),
            record_type: Set(record.record_type),
            customer_id: Set(record.customer_id),
            record_date: Set(record.record_date),
            customer_type: Set(record.customer_type),
            deal_type: Set(record.deal_type),
            system_id: Set(record.system_id),
            store_id: Set(record.store_id),
            handler_user_id: Set(record.handler_user_id),
            expert_user_id: Set(record.expert_user_id),
            consultant_user_id: Set(record.consultant_user_id),
            doctor_user_id: Set(record.doctor_user_id),
            remark: Set(record.remark),
            status: Set(record.status),
            created_by_user_id: Set(record.created_by_user_id),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(sales_record_id = %record.id, "inserted sales record");
        Ok(record)
    }

    #[tracing::instrument(level = "info", skip(self, conn, line), fields(sales_record_id = %line.sales_record_id, product_id = %line.product_id))]
    pub async fn insert_sales_record_line<C: ConnectionTrait>(
        &self,
        conn: &C,
        line: NewSalesRecordLine,
        now: DateTime<Utc>,
    ) -> Result<sales_record_lines::Model, RepositoryError> {
        validate_required("item_name", &line.item_name)?;
        validate_required("status", &line.status)?;

        let line = sales_record_lines::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_id: Set(line.sales_record_id),
            product_id: Set(line.product_id),
            item_name: Set(line.item_name),
            receivable_amount: Set(line.receivable_amount),
            operation_total_count: Set(line.operation_total_count),
            remark: Set(line.remark),
            status: Set(line.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(sales_record_line_id = %line.id, "inserted sales record line");
        Ok(line)
    }

    #[tracing::instrument(level = "info", skip(self, conn, payment), fields(sales_record_id = %payment.sales_record_id, payment_type = %payment.payment_type))]
    pub async fn insert_sales_payment<C: ConnectionTrait>(
        &self,
        conn: &C,
        payment: NewSalesPayment,
        now: DateTime<Utc>,
    ) -> Result<sales_payments::Model, RepositoryError> {
        validate_required("payment_type", &payment.payment_type)?;
        validate_required("performance_status", &payment.performance_status)?;
        validate_required("status", &payment.status)?;

        let payment = sales_payments::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_id: Set(payment.sales_record_id),
            payment_type: Set(payment.payment_type),
            paid_amount: Set(payment.paid_amount),
            paid_at: Set(payment.paid_at),
            performance_status: Set(payment.performance_status),
            status: Set(payment.status),
            remark: Set(payment.remark),
            created_by_user_id: Set(payment.created_by_user_id),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(payment_id = %payment.id, "inserted sales payment");
        Ok(payment)
    }

    #[tracing::instrument(level = "info", skip(self, conn, allocation), fields(payment_id = %allocation.payment_id, guide_user_id = %allocation.guide_user_id))]
    pub async fn insert_sales_payment_allocation<C: ConnectionTrait>(
        &self,
        conn: &C,
        allocation: NewSalesPaymentAllocation,
        now: DateTime<Utc>,
    ) -> Result<sales_payment_allocations::Model, RepositoryError> {
        let allocation = sales_payment_allocations::ActiveModel {
            id: Set(Uuid::new_v4()),
            payment_id: Set(allocation.payment_id),
            guide_user_id: Set(allocation.guide_user_id),
            allocation_ratio: Set(allocation.allocation_ratio),
            allocated_amount: Set(allocation.allocated_amount),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(allocation_id = %allocation.id, "inserted sales payment allocation");
        Ok(allocation)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_sales_record_by_id(
        &self,
        sales_record_id: Uuid,
    ) -> Result<Option<sales_records::Model>, RepositoryError> {
        let record = sales_records::Entity::find_by_id(sales_record_id)
            .one(&self.db)
            .await?;
        debug!(found = record.is_some(), %sales_record_id, "looked up sales record by id");
        Ok(record)
    }

    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_sales_record_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        sales_record_id: Uuid,
    ) -> Result<Option<sales_records::Model>, RepositoryError> {
        let record = sales_records::Entity::find_by_id(sales_record_id)
            .lock_exclusive()
            .one(tx)
            .await?;
        debug!(found = record.is_some(), %sales_record_id, "locked sales record by id");
        Ok(record)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_sales_records(
        &self,
        filters: SalesRecordFilters<'_>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<sales_records::Model>, u64), RepositoryError> {
        let mut query = sales_records::Entity::find()
            .order_by_desc(sales_records::Column::RecordDate)
            .order_by_desc(sales_records::Column::CreatedAt)
            .order_by_asc(sales_records::Column::Id);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(sales_records::Column::Status.eq(status_filter.trim()));
        }
        if let Some(record_type) = filters.record_type {
            validate_required("record_type", record_type)?;
            query = query.filter(sales_records::Column::RecordType.eq(record_type.trim()));
        }
        if let Some(customer_id) = filters.customer_id {
            query = query.filter(sales_records::Column::CustomerId.eq(customer_id));
        }
        if let Some(system_id) = filters.system_id {
            query = query.filter(sales_records::Column::SystemId.eq(system_id));
        }
        if let Some(store_id) = filters.store_id {
            query = query.filter(sales_records::Column::StoreId.eq(store_id));
        }
        if let Some(handler_user_id) = filters.handler_user_id {
            query = query.filter(sales_records::Column::HandlerUserId.eq(handler_user_id));
        }
        if let Some(record_date_from) = filters.record_date_from {
            query = query.filter(sales_records::Column::RecordDate.gte(record_date_from));
        }
        if let Some(record_date_to) = filters.record_date_to {
            query = query.filter(sales_records::Column::RecordDate.lte(record_date_to));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let records = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = records.len(),
            total_count, page_number, page_size, "listed sales records"
        );
        Ok((records, total_count))
    }

    #[tracing::instrument(level = "info", skip(self, conn, record), fields(sales_record_id = %record.id, status = %status))]
    pub async fn update_sales_record_status<C: ConnectionTrait>(
        &self,
        conn: &C,
        record: &sales_records::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        validate_required("status", status)?;
        let mut active: sales_records::ActiveModel = record.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let record = active.update(conn).await?;
        info!(sales_record_id = %record.id, status = %record.status, "updated sales record status");
        Ok(record)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_lines_by_sales_record_ids(
        &self,
        sales_record_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_lines::Model>, RepositoryError> {
        if sales_record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let lines = sales_record_lines::Entity::find()
            .filter(sales_record_lines::Column::SalesRecordId.is_in(sales_record_ids))
            .order_by_asc(sales_record_lines::Column::CreatedAt)
            .order_by_asc(sales_record_lines::Column::Id)
            .all(&self.db)
            .await?;
        debug!(
            count = lines.len(),
            "looked up sales record lines by record ids"
        );
        Ok(lines)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_lines_by_sales_record_id(
        &self,
        sales_record_id: Uuid,
    ) -> Result<Vec<sales_record_lines::Model>, RepositoryError> {
        self.find_lines_by_sales_record_ids(vec![sales_record_id])
            .await
    }

    #[tracing::instrument(level = "debug", skip(self, conn))]
    pub async fn find_lines_by_sales_record_id_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_id: Uuid,
    ) -> Result<Vec<sales_record_lines::Model>, RepositoryError> {
        let lines = sales_record_lines::Entity::find()
            .filter(sales_record_lines::Column::SalesRecordId.eq(sales_record_id))
            .order_by_asc(sales_record_lines::Column::CreatedAt)
            .order_by_asc(sales_record_lines::Column::Id)
            .all(conn)
            .await?;
        debug!(%sales_record_id, count = lines.len(), "looked up sales record lines in connection");
        Ok(lines)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_line_by_id(
        &self,
        sales_record_line_id: Uuid,
    ) -> Result<Option<sales_record_lines::Model>, RepositoryError> {
        let line = sales_record_lines::Entity::find_by_id(sales_record_line_id)
            .one(&self.db)
            .await?;
        debug!(found = line.is_some(), %sales_record_line_id, "looked up sales record line by id");
        Ok(line)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_lines_by_ids(
        &self,
        sales_record_line_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_lines::Model>, RepositoryError> {
        if sales_record_line_ids.is_empty() {
            return Ok(Vec::new());
        }
        let lines = sales_record_lines::Entity::find()
            .filter(sales_record_lines::Column::Id.is_in(sales_record_line_ids))
            .all(&self.db)
            .await?;
        debug!(count = lines.len(), "looked up sales record lines by ids");
        Ok(lines)
    }

    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_line_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        sales_record_line_id: Uuid,
    ) -> Result<Option<sales_record_lines::Model>, RepositoryError> {
        let line = sales_record_lines::Entity::find_by_id(sales_record_line_id)
            .lock_exclusive()
            .one(tx)
            .await?;
        debug!(found = line.is_some(), %sales_record_line_id, "locked sales record line by id");
        Ok(line)
    }

    #[tracing::instrument(level = "info", skip(self, conn, line), fields(sales_record_line_id = %line.id, status = %status))]
    pub async fn update_line_status<C: ConnectionTrait>(
        &self,
        conn: &C,
        line: &sales_record_lines::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<sales_record_lines::Model, RepositoryError> {
        validate_required("status", status)?;
        let mut active: sales_record_lines::ActiveModel = line.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        Ok(active.update(conn).await?)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_payments_by_sales_record_ids(
        &self,
        sales_record_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_payments::Model>, RepositoryError> {
        if sales_record_ids.is_empty() {
            return Ok(Vec::new());
        }
        let payments = sales_payments::Entity::find()
            .filter(sales_payments::Column::SalesRecordId.is_in(sales_record_ids))
            .order_by_asc(sales_payments::Column::PaidAt)
            .order_by_asc(sales_payments::Column::CreatedAt)
            .order_by_asc(sales_payments::Column::Id)
            .all(&self.db)
            .await?;
        debug!(
            count = payments.len(),
            "looked up sales payments by record ids"
        );
        Ok(payments)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_payments_by_sales_record_id(
        &self,
        sales_record_id: Uuid,
    ) -> Result<Vec<sales_payments::Model>, RepositoryError> {
        self.find_payments_by_sales_record_ids(vec![sales_record_id])
            .await
    }

    #[tracing::instrument(level = "debug", skip(self, conn))]
    pub async fn find_payments_by_sales_record_id_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_id: Uuid,
    ) -> Result<Vec<sales_payments::Model>, RepositoryError> {
        let payments = sales_payments::Entity::find()
            .filter(sales_payments::Column::SalesRecordId.eq(sales_record_id))
            .order_by_asc(sales_payments::Column::PaidAt)
            .order_by_asc(sales_payments::Column::CreatedAt)
            .order_by_asc(sales_payments::Column::Id)
            .all(conn)
            .await?;
        debug!(%sales_record_id, count = payments.len(), "looked up sales payments in connection");
        Ok(payments)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_payment_by_id(
        &self,
        payment_id: Uuid,
    ) -> Result<Option<sales_payments::Model>, RepositoryError> {
        let payment = sales_payments::Entity::find_by_id(payment_id)
            .one(&self.db)
            .await?;
        debug!(found = payment.is_some(), %payment_id, "looked up sales payment by id");
        Ok(payment)
    }

    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_payment_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        payment_id: Uuid,
    ) -> Result<Option<sales_payments::Model>, RepositoryError> {
        let payment = sales_payments::Entity::find_by_id(payment_id)
            .lock_exclusive()
            .one(tx)
            .await?;
        debug!(found = payment.is_some(), %payment_id, "locked sales payment by id");
        Ok(payment)
    }

    pub async fn find_payment_by_id_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        payment_id: Uuid,
    ) -> Result<Option<sales_payments::Model>, RepositoryError> {
        Ok(sales_payments::Entity::find_by_id(payment_id)
            .one(conn)
            .await?)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_sales_payments(
        &self,
        filters: SalesPaymentFilters<'_>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<sales_payments::Model>, u64), RepositoryError> {
        let mut query = sales_payments::Entity::find()
            .order_by_desc(sales_payments::Column::PaidAt)
            .order_by_desc(sales_payments::Column::CreatedAt)
            .order_by_asc(sales_payments::Column::Id);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(sales_payments::Column::Status.eq(status_filter.trim()));
        }
        if let Some(payment_type) = filters.payment_type {
            validate_required("payment_type", payment_type)?;
            query = query.filter(sales_payments::Column::PaymentType.eq(payment_type.trim()));
        }
        if let Some(sales_record_id) = filters.sales_record_id {
            query = query.filter(sales_payments::Column::SalesRecordId.eq(sales_record_id));
        }
        if let Some(paid_at_from) = filters.paid_at_from {
            query = query.filter(sales_payments::Column::PaidAt.gte(paid_at_from));
        }
        if let Some(paid_at_to) = filters.paid_at_to {
            query = query.filter(sales_payments::Column::PaidAt.lte(paid_at_to));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let payments = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = payments.len(),
            total_count, page_number, page_size, "listed sales payments"
        );
        Ok((payments, total_count))
    }

    #[tracing::instrument(level = "info", skip(self, conn, payment), fields(payment_id = %payment.id, status = %status))]
    pub async fn update_payment_status<C: ConnectionTrait>(
        &self,
        conn: &C,
        payment: &sales_payments::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<sales_payments::Model, RepositoryError> {
        validate_required("status", status)?;
        let mut active: sales_payments::ActiveModel = payment.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let payment = active.update(conn).await?;
        info!(payment_id = %payment.id, status = %payment.status, "updated payment status");
        Ok(payment)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_allocations_by_payment_ids(
        &self,
        payment_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_payment_allocations::Model>, RepositoryError> {
        if payment_ids.is_empty() {
            return Ok(Vec::new());
        }
        let allocations = sales_payment_allocations::Entity::find()
            .filter(sales_payment_allocations::Column::PaymentId.is_in(payment_ids))
            .order_by_asc(sales_payment_allocations::Column::CreatedAt)
            .order_by_asc(sales_payment_allocations::Column::Id)
            .all(&self.db)
            .await?;
        debug!(
            count = allocations.len(),
            "looked up sales payment allocations by payment ids"
        );
        Ok(allocations)
    }

    #[tracing::instrument(level = "info", skip(self, conn, count), fields(sales_record_line_id = %count.sales_record_line_id))]
    pub async fn insert_operation_count<C: ConnectionTrait>(
        &self,
        conn: &C,
        count: NewOperationCount,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_counts::Model, RepositoryError> {
        validate_required("status", &count.status)?;
        let count = sales_record_operation_counts::ActiveModel {
            sales_record_line_id: Set(count.sales_record_line_id),
            total_count: Set(count.total_count),
            used_count: Set(count.used_count),
            status: Set(count.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;
        info!(sales_record_line_id = %count.sales_record_line_id, "inserted operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_operation_count(
        &self,
        sales_record_line_id: Uuid,
    ) -> Result<Option<sales_record_operation_counts::Model>, RepositoryError> {
        let count = sales_record_operation_counts::Entity::find_by_id(sales_record_line_id)
            .one(&self.db)
            .await?;
        debug!(found = count.is_some(), %sales_record_line_id, "looked up operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_operation_count_for_update(
        &self,
        tx: &DatabaseTransaction,
        sales_record_line_id: Uuid,
    ) -> Result<Option<sales_record_operation_counts::Model>, RepositoryError> {
        let count = sales_record_operation_counts::Entity::find_by_id(sales_record_line_id)
            .lock_exclusive()
            .one(tx)
            .await?;
        debug!(found = count.is_some(), %sales_record_line_id, "locked operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_operation_counts_by_line_ids(
        &self,
        sales_record_line_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_operation_counts::Model>, RepositoryError> {
        if sales_record_line_ids.is_empty() {
            return Ok(Vec::new());
        }
        let counts = sales_record_operation_counts::Entity::find()
            .filter(
                sales_record_operation_counts::Column::SalesRecordLineId
                    .is_in(sales_record_line_ids),
            )
            .all(&self.db)
            .await?;
        debug!(
            count = counts.len(),
            "looked up operation counts by line ids"
        );
        Ok(counts)
    }

    #[tracing::instrument(level = "debug", skip(self, conn))]
    pub async fn find_operation_counts_by_line_ids_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_line_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_operation_counts::Model>, RepositoryError> {
        if sales_record_line_ids.is_empty() {
            return Ok(Vec::new());
        }
        let counts = sales_record_operation_counts::Entity::find()
            .filter(
                sales_record_operation_counts::Column::SalesRecordLineId
                    .is_in(sales_record_line_ids),
            )
            .all(conn)
            .await?;
        debug!(
            count = counts.len(),
            "looked up operation counts by line ids in connection"
        );
        Ok(counts)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_operation_counts(
        &self,
        filters: OperationCountFilters<'_>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<sales_record_operation_counts::Model>, u64), RepositoryError> {
        let mut query = sales_record_operation_counts::Entity::find()
            .order_by_desc(sales_record_operation_counts::Column::CreatedAt)
            .order_by_asc(sales_record_operation_counts::Column::SalesRecordLineId);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query
                .filter(sales_record_operation_counts::Column::Status.eq(status_filter.trim()));
        }
        if let Some(sales_record_line_id) = filters.sales_record_line_id {
            query = query.filter(
                sales_record_operation_counts::Column::SalesRecordLineId.eq(sales_record_line_id),
            );
        }
        if let Some(sales_record_id) = filters.sales_record_id {
            query = query
                .inner_join(sales_record_lines::Entity)
                .filter(sales_record_lines::Column::SalesRecordId.eq(sales_record_id));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let counts = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = counts.len(),
            total_count, page_number, page_size, "listed operation counts"
        );
        Ok((counts, total_count))
    }

    #[tracing::instrument(level = "info", skip(self, conn, count, changes), fields(sales_record_line_id = %count.sales_record_line_id))]
    pub async fn update_operation_count<C: ConnectionTrait>(
        &self,
        conn: &C,
        count: &sales_record_operation_counts::Model,
        changes: OperationCountChanges,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_counts::Model, RepositoryError> {
        let mut active: sales_record_operation_counts::ActiveModel = count.clone().into();
        if let Some(total_count) = changes.total_count {
            active.total_count = Set(total_count);
        }
        if let Some(used_count) = changes.used_count {
            active.used_count = Set(used_count);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        active.updated_at = Set(now);

        let count = active.update(conn).await?;
        info!(sales_record_line_id = %count.sales_record_line_id, "updated operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "info", skip(self, conn, usage), fields(sales_record_line_id = %usage.sales_record_line_id))]
    pub async fn insert_operation_usage<C: ConnectionTrait>(
        &self,
        conn: &C,
        usage: NewOperationUsage,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_usages::Model, RepositoryError> {
        validate_required("status", &usage.status)?;
        let usage = sales_record_operation_usages::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_line_id: Set(usage.sales_record_line_id),
            operated_at: Set(usage.operated_at),
            operator_user_id: Set(usage.operator_user_id),
            doctor_user_id: Set(usage.doctor_user_id),
            operation_count: Set(usage.operation_count),
            remark: Set(usage.remark),
            status: Set(usage.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(operation_usage_id = %usage.id, "inserted operation usage");
        Ok(usage)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_operation_usage_by_id(
        &self,
        usage_id: Uuid,
    ) -> Result<Option<sales_record_operation_usages::Model>, RepositoryError> {
        let usage = sales_record_operation_usages::Entity::find_by_id(usage_id)
            .one(&self.db)
            .await?;
        debug!(found = usage.is_some(), %usage_id, "looked up operation usage by id");
        Ok(usage)
    }

    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_operation_usage_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        usage_id: Uuid,
    ) -> Result<Option<sales_record_operation_usages::Model>, RepositoryError> {
        let usage = sales_record_operation_usages::Entity::find_by_id(usage_id)
            .lock_exclusive()
            .one(tx)
            .await?;
        debug!(found = usage.is_some(), %usage_id, "locked operation usage by id");
        Ok(usage)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_operation_usages(
        &self,
        filters: OperationUsageFilters<'_>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<sales_record_operation_usages::Model>, u64), RepositoryError> {
        let mut query = sales_record_operation_usages::Entity::find()
            .order_by_desc(sales_record_operation_usages::Column::OperatedAt)
            .order_by_desc(sales_record_operation_usages::Column::CreatedAt)
            .order_by_asc(sales_record_operation_usages::Column::Id);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query
                .filter(sales_record_operation_usages::Column::Status.eq(status_filter.trim()));
        }
        if let Some(sales_record_line_id) = filters.sales_record_line_id {
            query = query.filter(
                sales_record_operation_usages::Column::SalesRecordLineId.eq(sales_record_line_id),
            );
        }
        if let Some(sales_record_id) = filters.sales_record_id {
            query = query
                .inner_join(sales_record_lines::Entity)
                .filter(sales_record_lines::Column::SalesRecordId.eq(sales_record_id));
        }
        if let Some(operator_user_id) = filters.operator_user_id {
            query = query
                .filter(sales_record_operation_usages::Column::OperatorUserId.eq(operator_user_id));
        }
        if let Some(doctor_user_id) = filters.doctor_user_id {
            query = query
                .filter(sales_record_operation_usages::Column::DoctorUserId.eq(doctor_user_id));
        }
        if let Some(operated_at_from) = filters.operated_at_from {
            query = query
                .filter(sales_record_operation_usages::Column::OperatedAt.gte(operated_at_from));
        }
        if let Some(operated_at_to) = filters.operated_at_to {
            query =
                query.filter(sales_record_operation_usages::Column::OperatedAt.lte(operated_at_to));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let usages = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = usages.len(),
            total_count, page_number, page_size, "listed operation usages"
        );
        Ok((usages, total_count))
    }

    #[tracing::instrument(level = "debug", skip(self, conn))]
    pub async fn count_active_operation_usages_for_lines<C: ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_line_ids: Vec<Uuid>,
    ) -> Result<u64, RepositoryError> {
        if sales_record_line_ids.is_empty() {
            return Ok(0);
        }
        let count = sales_record_operation_usages::Entity::find()
            .filter(
                sales_record_operation_usages::Column::SalesRecordLineId
                    .is_in(sales_record_line_ids),
            )
            .filter(sales_record_operation_usages::Column::Status.eq("active"))
            .count(conn)
            .await?;
        debug!(
            count,
            "counted active operation usages for sales record lines"
        );
        Ok(count)
    }

    #[tracing::instrument(level = "info", skip(self, conn, usage, changes), fields(operation_usage_id = %usage.id))]
    pub async fn update_operation_usage<C: ConnectionTrait>(
        &self,
        conn: &C,
        usage: &sales_record_operation_usages::Model,
        changes: OperationUsageChanges,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_usages::Model, RepositoryError> {
        let mut active: sales_record_operation_usages::ActiveModel = usage.clone().into();
        if let Some(operated_at) = changes.operated_at {
            active.operated_at = Set(operated_at);
        }
        if let Some(operator_user_id) = changes.operator_user_id {
            active.operator_user_id = Set(operator_user_id);
        }
        if let Some(doctor_user_id) = changes.doctor_user_id {
            active.doctor_user_id = Set(doctor_user_id);
        }
        if let Some(operation_count) = changes.operation_count {
            active.operation_count = Set(operation_count);
        }
        if let Some(remark) = changes.remark {
            active.remark = Set(remark);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        active.updated_at = Set(now);

        let usage = active.update(conn).await?;
        info!(operation_usage_id = %usage.id, "updated operation usage");
        Ok(usage)
    }

    #[tracing::instrument(level = "info", skip(self, conn))]
    pub async fn delete_operation_usage_by_id<C: ConnectionTrait>(
        &self,
        conn: &C,
        usage_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let result = sales_record_operation_usages::Entity::delete_by_id(usage_id)
            .exec(conn)
            .await?;
        let deleted = result.rows_affected > 0;
        info!(%usage_id, deleted, "deleted operation usage by id");
        Ok(deleted)
    }
}

fn validate_required(field: &'static str, value: &str) -> Result<(), RepositoryError> {
    if value.trim().is_empty() {
        return Err(RepositoryError::MissingRequiredField { field });
    }
    Ok(())
}

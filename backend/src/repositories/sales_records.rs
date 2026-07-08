use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    entities::{sales_record_operation_counts, sales_record_operation_usages, sales_records},
    repositories::RepositoryError,
};

#[derive(Clone)]
pub struct SalesRecordRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewSalesRecord {
    pub record_group_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub department_id: Uuid,
    pub sale_date: NaiveDate,
    pub deal_status: String,
    pub customer_type: String,
    pub deal_type: String,
    pub content_category_id: Uuid,
    pub handler_user_id: Uuid,
    pub paid_amount: Decimal,
    pub unpaid_amount: Decimal,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub collaboration_type: String,
    pub expert_user_id: Option<Uuid>,
    pub expert_department_id: Option<Uuid>,
    pub consultant_user_id: Option<Uuid>,
    pub consultant_department_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct SalesRecordChanges {
    pub customer_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub sale_date: Option<NaiveDate>,
    pub deal_status: Option<String>,
    pub customer_type: Option<String>,
    pub deal_type: Option<String>,
    pub content_category_id: Option<Uuid>,
    pub handler_user_id: Option<Uuid>,
    pub paid_amount: Option<Decimal>,
    pub unpaid_amount: Option<Decimal>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub collaboration_type: Option<String>,
    pub expert_user_id: Option<Option<Uuid>>,
    pub expert_department_id: Option<Option<Uuid>>,
    pub consultant_user_id: Option<Option<Uuid>>,
    pub consultant_department_id: Option<Option<Uuid>>,
    pub doctor_user_id: Option<Option<Uuid>>,
}

impl SalesRecordChanges {
    pub fn is_empty(&self) -> bool {
        self.customer_id.is_none()
            && self.department_id.is_none()
            && self.sale_date.is_none()
            && self.deal_status.is_none()
            && self.customer_type.is_none()
            && self.deal_type.is_none()
            && self.content_category_id.is_none()
            && self.handler_user_id.is_none()
            && self.paid_amount.is_none()
            && self.unpaid_amount.is_none()
            && self.system_id.is_none()
            && self.store_id.is_none()
            && self.collaboration_type.is_none()
            && self.expert_user_id.is_none()
            && self.expert_department_id.is_none()
            && self.consultant_user_id.is_none()
            && self.consultant_department_id.is_none()
            && self.doctor_user_id.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SalesRecordFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub record_group_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub handler_user_id: Option<Uuid>,
    pub content_category_id: Option<Uuid>,
    pub sale_date_from: Option<NaiveDate>,
    pub sale_date_to: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct NewOperationCount {
    pub sales_record_id: Uuid,
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
    pub sales_record_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct NewOperationUsage {
    pub sales_record_id: Uuid,
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

    pub async fn begin(&self) -> Result<DatabaseTransaction, RepositoryError> {
        Ok(self.db.begin().await?)
    }

    #[tracing::instrument(level = "info", skip(self, conn, record), fields(customer_id = %record.customer_id, record_group_id = ?record.record_group_id))]
    pub async fn insert_sales_record<C: ConnectionTrait>(
        &self,
        conn: &C,
        record: NewSalesRecord,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        validate_required("deal_status", &record.deal_status)?;
        validate_required("customer_type", &record.customer_type)?;
        validate_required("deal_type", &record.deal_type)?;
        validate_required("collaboration_type", &record.collaboration_type)?;
        validate_required("status", &record.status)?;

        let record = sales_records::ActiveModel {
            id: Set(Uuid::new_v4()),
            record_group_id: Set(record.record_group_id),
            customer_id: Set(record.customer_id),
            department_id: Set(record.department_id),
            sale_date: Set(record.sale_date),
            deal_status: Set(record.deal_status),
            customer_type: Set(record.customer_type),
            deal_type: Set(record.deal_type),
            content_category_id: Set(record.content_category_id),
            handler_user_id: Set(record.handler_user_id),
            paid_amount: Set(record.paid_amount),
            unpaid_amount: Set(record.unpaid_amount),
            system_id: Set(record.system_id),
            store_id: Set(record.store_id),
            collaboration_type: Set(record.collaboration_type),
            expert_user_id: Set(record.expert_user_id),
            expert_department_id: Set(record.expert_department_id),
            consultant_user_id: Set(record.consultant_user_id),
            consultant_department_id: Set(record.consultant_department_id),
            doctor_user_id: Set(record.doctor_user_id),
            status: Set(record.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(sales_record_id = %record.id, "inserted sales record");
        Ok(record)
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
            .order_by_desc(sales_records::Column::SaleDate)
            .order_by_desc(sales_records::Column::CreatedAt)
            .order_by_asc(sales_records::Column::Id);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(sales_records::Column::Status.eq(status_filter.trim()));
        }
        if let Some(record_group_id) = filters.record_group_id {
            query = query.filter(sales_records::Column::RecordGroupId.eq(record_group_id));
        }
        if let Some(customer_id) = filters.customer_id {
            query = query.filter(sales_records::Column::CustomerId.eq(customer_id));
        }
        if let Some(department_id) = filters.department_id {
            query = query.filter(sales_records::Column::DepartmentId.eq(department_id));
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
        if let Some(content_category_id) = filters.content_category_id {
            query = query.filter(sales_records::Column::ContentCategoryId.eq(content_category_id));
        }
        if let Some(sale_date_from) = filters.sale_date_from {
            query = query.filter(sales_records::Column::SaleDate.gte(sale_date_from));
        }
        if let Some(sale_date_to) = filters.sale_date_to {
            query = query.filter(sales_records::Column::SaleDate.lte(sale_date_to));
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

    #[tracing::instrument(level = "info", skip(self, conn, record, changes), fields(sales_record_id = %record.id))]
    pub async fn update_sales_record<C: ConnectionTrait>(
        &self,
        conn: &C,
        record: &sales_records::Model,
        changes: SalesRecordChanges,
        now: DateTime<Utc>,
    ) -> Result<sales_records::Model, RepositoryError> {
        let mut active: sales_records::ActiveModel = record.clone().into();

        if let Some(customer_id) = changes.customer_id {
            active.customer_id = Set(customer_id);
        }
        if let Some(department_id) = changes.department_id {
            active.department_id = Set(department_id);
        }
        if let Some(sale_date) = changes.sale_date {
            active.sale_date = Set(sale_date);
        }
        if let Some(deal_status) = changes.deal_status {
            validate_required("deal_status", &deal_status)?;
            active.deal_status = Set(deal_status);
        }
        if let Some(customer_type) = changes.customer_type {
            validate_required("customer_type", &customer_type)?;
            active.customer_type = Set(customer_type);
        }
        if let Some(deal_type) = changes.deal_type {
            validate_required("deal_type", &deal_type)?;
            active.deal_type = Set(deal_type);
        }
        if let Some(content_category_id) = changes.content_category_id {
            active.content_category_id = Set(content_category_id);
        }
        if let Some(handler_user_id) = changes.handler_user_id {
            active.handler_user_id = Set(handler_user_id);
        }
        if let Some(paid_amount) = changes.paid_amount {
            active.paid_amount = Set(paid_amount);
        }
        if let Some(unpaid_amount) = changes.unpaid_amount {
            active.unpaid_amount = Set(unpaid_amount);
        }
        if let Some(system_id) = changes.system_id {
            active.system_id = Set(system_id);
        }
        if let Some(store_id) = changes.store_id {
            active.store_id = Set(store_id);
        }
        if let Some(collaboration_type) = changes.collaboration_type {
            validate_required("collaboration_type", &collaboration_type)?;
            active.collaboration_type = Set(collaboration_type);
        }
        if let Some(expert_user_id) = changes.expert_user_id {
            active.expert_user_id = Set(expert_user_id);
        }
        if let Some(expert_department_id) = changes.expert_department_id {
            active.expert_department_id = Set(expert_department_id);
        }
        if let Some(consultant_user_id) = changes.consultant_user_id {
            active.consultant_user_id = Set(consultant_user_id);
        }
        if let Some(consultant_department_id) = changes.consultant_department_id {
            active.consultant_department_id = Set(consultant_department_id);
        }
        if let Some(doctor_user_id) = changes.doctor_user_id {
            active.doctor_user_id = Set(doctor_user_id);
        }
        active.updated_at = Set(now);

        let record = active.update(conn).await?;
        info!(sales_record_id = %record.id, "updated sales record");
        Ok(record)
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

    #[tracing::instrument(level = "info", skip(self, conn))]
    pub async fn delete_sales_record_by_id<C: ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let result = sales_records::Entity::delete_by_id(sales_record_id)
            .exec(conn)
            .await?;
        let deleted = result.rows_affected > 0;

        info!(%sales_record_id, deleted, "deleted sales record by id");
        Ok(deleted)
    }

    #[tracing::instrument(level = "info", skip(self, conn, count), fields(sales_record_id = %count.sales_record_id))]
    pub async fn insert_operation_count<C: ConnectionTrait>(
        &self,
        conn: &C,
        count: NewOperationCount,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_counts::Model, RepositoryError> {
        validate_required("status", &count.status)?;

        let count = sales_record_operation_counts::ActiveModel {
            sales_record_id: Set(count.sales_record_id),
            total_count: Set(count.total_count),
            used_count: Set(count.used_count),
            status: Set(count.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(sales_record_id = %count.sales_record_id, "inserted sales record operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_operation_count(
        &self,
        sales_record_id: Uuid,
    ) -> Result<Option<sales_record_operation_counts::Model>, RepositoryError> {
        let count = sales_record_operation_counts::Entity::find_by_id(sales_record_id)
            .one(&self.db)
            .await?;

        debug!(found = count.is_some(), %sales_record_id, "looked up operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_operation_count_for_update(
        &self,
        tx: &DatabaseTransaction,
        sales_record_id: Uuid,
    ) -> Result<Option<sales_record_operation_counts::Model>, RepositoryError> {
        let count = sales_record_operation_counts::Entity::find_by_id(sales_record_id)
            .lock_exclusive()
            .one(tx)
            .await?;

        debug!(found = count.is_some(), %sales_record_id, "locked operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_operation_counts_by_sales_record_ids(
        &self,
        sales_record_ids: Vec<Uuid>,
    ) -> Result<Vec<sales_record_operation_counts::Model>, RepositoryError> {
        if sales_record_ids.is_empty() {
            return Ok(Vec::new());
        }

        let counts = sales_record_operation_counts::Entity::find()
            .filter(sales_record_operation_counts::Column::SalesRecordId.is_in(sales_record_ids))
            .all(&self.db)
            .await?;

        debug!(
            count = counts.len(),
            "looked up operation counts by sales record ids"
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
            .order_by_asc(sales_record_operation_counts::Column::SalesRecordId);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query
                .filter(sales_record_operation_counts::Column::Status.eq(status_filter.trim()));
        }
        if let Some(sales_record_id) = filters.sales_record_id {
            query = query
                .filter(sales_record_operation_counts::Column::SalesRecordId.eq(sales_record_id));
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

    #[tracing::instrument(level = "info", skip(self, conn, count, changes), fields(sales_record_id = %count.sales_record_id))]
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
        info!(sales_record_id = %count.sales_record_id, "updated operation count");
        Ok(count)
    }

    #[tracing::instrument(level = "info", skip(self, conn, usage), fields(sales_record_id = %usage.sales_record_id))]
    pub async fn insert_operation_usage<C: ConnectionTrait>(
        &self,
        conn: &C,
        usage: NewOperationUsage,
        now: DateTime<Utc>,
    ) -> Result<sales_record_operation_usages::Model, RepositoryError> {
        validate_required("status", &usage.status)?;

        let usage = sales_record_operation_usages::ActiveModel {
            id: Set(Uuid::new_v4()),
            sales_record_id: Set(usage.sales_record_id),
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
        if let Some(sales_record_id) = filters.sales_record_id {
            query = query
                .filter(sales_record_operation_usages::Column::SalesRecordId.eq(sales_record_id));
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
    pub async fn count_active_operation_usages<C: ConnectionTrait>(
        &self,
        conn: &C,
        sales_record_id: Uuid,
    ) -> Result<u64, RepositoryError> {
        let count = sales_record_operation_usages::Entity::find()
            .filter(sales_record_operation_usages::Column::SalesRecordId.eq(sales_record_id))
            .filter(sales_record_operation_usages::Column::Status.eq("active"))
            .count(conn)
            .await?;

        debug!(%sales_record_id, count, "counted active operation usages");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        repositories::{
            customers::{CustomerRepository, NewCustomer},
            departments::DepartmentRepository,
            product_categories::ProductCategoryRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        },
    };
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    struct Repos {
        users: UserRepository,
        departments: DepartmentRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        customers: CustomerRepository,
        categories: ProductCategoryRepository,
        sales_records: SalesRecordRepository,
    }

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn repos() -> Repos {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        Repos {
            users: UserRepository::new(db.clone()),
            departments: DepartmentRepository::new(db.clone()),
            systems: SystemRepository::new(db.clone()),
            stores: StoreRepository::new(db.clone()),
            customers: CustomerRepository::new(db.clone()),
            categories: ProductCategoryRepository::new(db.clone()),
            sales_records: SalesRecordRepository::new(db),
        }
    }

    async fn fixture(repos: &Repos) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
        let now = Utc.with_ymd_and_hms(2026, 7, 8, 0, 0, 0).unwrap();
        let user = repos
            .users
            .find_or_create_for_login("repo-user", now)
            .await
            .expect("user should be created");
        let department = repos
            .departments
            .insert_department(Uuid::new_v4(), "manual", "scope", "scope", None, now)
            .await
            .expect("department should be created");
        let system = repos
            .systems
            .create_system(
                NewSystem {
                    name: "system".to_string(),
                    department_id: department.id,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("system should be created");
        let store = repos
            .stores
            .create_store(
                NewStore {
                    name: "store".to_string(),
                    system_id: system.id,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("store should be created");
        let customer = repos
            .customers
            .create_customer(
                NewCustomer {
                    name: "Alice".to_string(),
                    creator_user_id: user.id,
                    department_id: department.id,
                    system_id: system.id,
                    store_id: store.id,
                    remark: None,
                    status: "active".to_string(),
                    attachments: None,
                },
                now,
            )
            .await
            .expect("customer should be created");
        let category = repos
            .categories
            .list_categories(Some("active"), Some(true), 1, 50)
            .await
            .expect("categories should list")
            .0[0]
            .clone();

        (
            user.id,
            department.id,
            system.id,
            store.id,
            customer.id,
            category.id,
        )
    }

    fn new_record(
        user_id: Uuid,
        department_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
        customer_id: Uuid,
        category_id: Uuid,
    ) -> NewSalesRecord {
        NewSalesRecord {
            record_group_id: Some(Uuid::new_v4()),
            customer_id,
            department_id,
            sale_date: Utc
                .with_ymd_and_hms(2026, 7, 8, 0, 0, 0)
                .unwrap()
                .date_naive(),
            deal_status: "closed".to_string(),
            customer_type: "new".to_string(),
            deal_type: "non_salon".to_string(),
            content_category_id: category_id,
            handler_user_id: user_id,
            paid_amount: Decimal::new(1000, 2),
            unpaid_amount: Decimal::ZERO,
            system_id,
            store_id,
            collaboration_type: "self_sale".to_string(),
            expert_user_id: None,
            expert_department_id: None,
            consultant_user_id: None,
            consultant_department_id: None,
            doctor_user_id: None,
            status: "active".to_string(),
        }
    }

    #[tokio::test]
    async fn repository_creates_lists_updates_and_deletes_sales_data() {
        let repos = repos().await;
        let (user_id, department_id, system_id, store_id, customer_id, category_id) =
            fixture(&repos).await;
        let now = Utc.with_ymd_and_hms(2026, 7, 8, 0, 0, 0).unwrap();
        let record = repos
            .sales_records
            .insert_sales_record(
                &repos.sales_records.db,
                new_record(
                    user_id,
                    department_id,
                    system_id,
                    store_id,
                    customer_id,
                    category_id,
                ),
                now,
            )
            .await
            .expect("sales record should insert");
        let count = repos
            .sales_records
            .insert_operation_count(
                &repos.sales_records.db,
                NewOperationCount {
                    sales_record_id: record.id,
                    total_count: 3,
                    used_count: 0,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("operation count should insert");
        assert_eq!(count.total_count, 3);

        let (listed, total_count) = repos
            .sales_records
            .list_sales_records(
                SalesRecordFilters {
                    customer_id: Some(customer_id),
                    content_category_id: Some(category_id),
                    ..SalesRecordFilters::default()
                },
                1,
                50,
            )
            .await
            .expect("records should list");
        assert_eq!(total_count, 1);
        assert_eq!(listed[0].id, record.id);

        let usage = repos
            .sales_records
            .insert_operation_usage(
                &repos.sales_records.db,
                NewOperationUsage {
                    sales_record_id: record.id,
                    operated_at: now,
                    operator_user_id: user_id,
                    doctor_user_id: None,
                    operation_count: 1,
                    remark: Some("remark".to_string()),
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("usage should insert");
        assert_eq!(
            repos
                .sales_records
                .count_active_operation_usages(&repos.sales_records.db, record.id)
                .await
                .expect("active usage count should work"),
            1
        );

        let count = repos
            .sales_records
            .update_operation_count(
                &repos.sales_records.db,
                &count,
                OperationCountChanges {
                    used_count: Some(1),
                    ..OperationCountChanges::default()
                },
                now,
            )
            .await
            .expect("count should update");
        assert_eq!(count.used_count, 1);

        assert!(
            repos
                .sales_records
                .delete_operation_usage_by_id(&repos.sales_records.db, usage.id)
                .await
                .expect("usage should delete")
        );
        assert!(
            repos
                .sales_records
                .delete_sales_record_by_id(&repos.sales_records.db, record.id)
                .await
                .expect("record should delete")
        );
    }

    #[tokio::test]
    async fn repository_database_constraints_reject_invalid_values() {
        let repos = repos().await;
        let (user_id, department_id, system_id, store_id, customer_id, category_id) =
            fixture(&repos).await;
        let now = Utc.with_ymd_and_hms(2026, 7, 8, 0, 0, 0).unwrap();
        let record = repos
            .sales_records
            .insert_sales_record(
                &repos.sales_records.db,
                new_record(
                    user_id,
                    department_id,
                    system_id,
                    store_id,
                    customer_id,
                    category_id,
                ),
                now,
            )
            .await
            .expect("sales record should insert");

        let mut negative = new_record(
            user_id,
            department_id,
            system_id,
            store_id,
            customer_id,
            category_id,
        );
        negative.paid_amount = Decimal::new(-1, 2);
        assert!(
            repos
                .sales_records
                .insert_sales_record(&repos.sales_records.db, negative, now)
                .await
                .is_err()
        );

        assert!(
            repos
                .sales_records
                .insert_operation_count(
                    &repos.sales_records.db,
                    NewOperationCount {
                        sales_record_id: record.id,
                        total_count: 1,
                        used_count: 2,
                        status: "active".to_string(),
                    },
                    now,
                )
                .await
                .is_err()
        );
    }
}

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, Statement, TransactionTrait, Value,
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
    pub performance_date: NaiveDate,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub source_entry_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct PerformanceReportFilters {
    pub performance_date_from: NaiveDate,
    pub performance_date_to: NaiveDate,
    pub user_id: Option<Uuid>,
    pub performance_role: Option<String>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub entry_type: Option<String>,
    pub payment_id: Option<Uuid>,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct PerformanceSummaryRow {
    pub user_id: Uuid,
    pub user_name: String,
    pub job_number: String,
    pub expert_amount: Decimal,
    pub guide_amount: Decimal,
    pub reversal_amount: Decimal,
    pub net_amount: Decimal,
}

#[derive(Debug, Clone, FromQueryResult)]
pub struct PerformanceEntryRow {
    pub id: Uuid,
    pub batch_id: Uuid,
    pub payment_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub allocation_ratio: Option<Decimal>,
    pub sales_record_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub job_number: String,
    pub performance_role: String,
    pub entry_type: String,
    pub amount: Decimal,
    pub period_month: NaiveDate,
    pub performance_date: NaiveDate,
    pub system_id: Uuid,
    pub system_name: String,
    pub store_id: Uuid,
    pub store_name: String,
    pub paid_at: DateTime<Utc>,
    pub source_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromQueryResult)]
struct CountRow {
    count: i64,
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
            performance_date: Set(entry.performance_date),
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

    pub async fn list_report_entries(
        &self,
        filters: &PerformanceReportFilters,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<PerformanceEntryRow>, u64), RepositoryError> {
        let (where_sql, values) = report_where(self.db.get_database_backend(), filters);
        let from_sql = " FROM sales_performance_entries e LEFT JOIN user_profiles p ON p.user_id = e.user_id LEFT JOIN systems sys ON sys.id = e.system_id LEFT JOIN stores st ON st.id = e.store_id JOIN sales_payments pay ON pay.id = e.payment_id";
        let select = "SELECT e.id, e.batch_id, e.payment_id, e.allocation_id, e.allocation_ratio, e.sales_record_id, e.user_id, COALESCE(p.name, '') AS user_name, COALESCE(p.job_number, '') AS job_number, e.performance_role, e.entry_type, e.amount, e.period_month, e.performance_date, e.system_id, COALESCE(sys.name, '') AS system_name, e.store_id, COALESCE(st.name, '') AS store_name, pay.paid_at, e.source_entry_id, e.created_at";
        let mut paged_values = values.clone();
        let limit = placeholder(self.db.get_database_backend(), paged_values.len() + 1);
        paged_values.push(Value::BigInt(Some(page_size as i64)));
        let offset = placeholder(self.db.get_database_backend(), paged_values.len() + 1);
        paged_values.push(Value::BigInt(Some(
            page_number
                .saturating_sub(1)
                .saturating_mul(page_size)
                .min(i64::MAX as u64) as i64,
        )));
        let sql = format!(
            "{select}{from_sql}{where_sql} ORDER BY e.performance_date DESC, e.created_at DESC, e.id ASC LIMIT {limit} OFFSET {offset}"
        );
        let rows = PerformanceEntryRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            sql,
            paged_values,
        ))
        .all(&self.db)
        .await?;
        let count_sql = format!("SELECT COUNT(*) AS count{from_sql}{where_sql}");
        let count = CountRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            count_sql,
            values,
        ))
        .one(&self.db)
        .await?
        .map(|row| row.count.max(0) as u64)
        .unwrap_or(0);
        Ok((rows, count))
    }

    pub async fn list_all_report_entries(
        &self,
        filters: &PerformanceReportFilters,
        limit: u64,
    ) -> Result<Vec<PerformanceEntryRow>, RepositoryError> {
        Ok(self.list_report_entries(filters, 1, limit).await?.0)
    }

    pub async fn list_report_summary(
        &self,
        filters: &PerformanceReportFilters,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<PerformanceSummaryRow>, u64), RepositoryError> {
        let backend = self.db.get_database_backend();
        let (where_sql, values) = report_where(backend, filters);
        let from_sql =
            " FROM sales_performance_entries e LEFT JOIN user_profiles p ON p.user_id = e.user_id";
        let amounts = "e.user_id, COALESCE(p.name, '') AS user_name, COALESCE(p.job_number, '') AS job_number, SUM(CASE WHEN e.entry_type = 'earning' AND e.performance_role = 'expert' THEN e.amount ELSE e.amount * 0 END) AS expert_amount, SUM(CASE WHEN e.entry_type = 'earning' AND e.performance_role = 'guide' THEN e.amount ELSE e.amount * 0 END) AS guide_amount, SUM(CASE WHEN e.entry_type = 'reversal' THEN -e.amount ELSE e.amount * 0 END) AS reversal_amount, SUM(e.amount) AS net_amount";
        let group = " GROUP BY e.user_id, p.name, p.job_number";
        let mut paged_values = values.clone();
        let limit = placeholder(backend, paged_values.len() + 1);
        paged_values.push(Value::BigInt(Some(page_size as i64)));
        let offset = placeholder(backend, paged_values.len() + 1);
        paged_values.push(Value::BigInt(Some(
            page_number
                .saturating_sub(1)
                .saturating_mul(page_size)
                .min(i64::MAX as u64) as i64,
        )));
        let sql = format!(
            "SELECT {amounts}{from_sql}{where_sql}{group} ORDER BY net_amount DESC, e.user_id ASC LIMIT {limit} OFFSET {offset}"
        );
        let rows = PerformanceSummaryRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            sql,
            paged_values,
        ))
        .all(&self.db)
        .await?;
        let count_sql = format!(
            "SELECT COUNT(*) AS count FROM (SELECT e.user_id{from_sql}{where_sql}{group}) grouped"
        );
        let count =
            CountRow::find_by_statement(Statement::from_sql_and_values(backend, count_sql, values))
                .one(&self.db)
                .await?
                .map(|row| row.count.max(0) as u64)
                .unwrap_or(0);
        Ok((rows, count))
    }
}

fn placeholder(backend: DatabaseBackend, index: usize) -> String {
    match backend {
        DatabaseBackend::Postgres => format!("${index}"),
        _ => "?".to_string(),
    }
}

fn report_where(
    backend: DatabaseBackend,
    filters: &PerformanceReportFilters,
) -> (String, Vec<Value>) {
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    let mut push = |column: &str, value: Value, operator: &str| {
        values.push(value);
        clauses.push(format!(
            "{column} {operator} {}",
            placeholder(backend, values.len())
        ));
    };
    push(
        "e.performance_date",
        filters.performance_date_from.into(),
        ">=",
    );
    push(
        "e.performance_date",
        filters.performance_date_to.into(),
        "<=",
    );
    if let Some(value) = filters.user_id {
        push("e.user_id", value.into(), "=");
    }
    if let Some(value) = &filters.performance_role {
        push("e.performance_role", value.clone().into(), "=");
    }
    if let Some(value) = filters.system_id {
        push("e.system_id", value.into(), "=");
    }
    if let Some(value) = filters.store_id {
        push("e.store_id", value.into(), "=");
    }
    if let Some(value) = &filters.entry_type {
        push("e.entry_type", value.clone().into(), "=");
    }
    if let Some(value) = filters.payment_id {
        push("e.payment_id", value.into(), "=");
    }
    (format!(" WHERE {}", clauses.join(" AND ")), values)
}

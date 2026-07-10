use sea_orm::{ConnectionTrait, DatabaseBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SalesPerformanceEntries::Table)
                    .add_column(ColumnDef::new(SalesPerformanceEntries::PerformanceDate).date())
                    .to_owned(),
            )
            .await?;

        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                "UPDATE sales_performance_entries e SET performance_date = CASE WHEN e.entry_type = 'earning' THEN (p.paid_at AT TIME ZONE 'Asia/Shanghai')::date ELSE (e.created_at AT TIME ZONE 'Asia/Shanghai')::date END FROM sales_payments p WHERE p.id = e.payment_id"
            }
            DatabaseBackend::Sqlite => {
                "UPDATE sales_performance_entries SET performance_date = CASE WHEN entry_type = 'earning' THEN (SELECT date(paid_at, '+8 hours') FROM sales_payments WHERE id = sales_performance_entries.payment_id) ELSE date(created_at, '+8 hours') END"
            }
            _ => return Err(DbErr::Custom("unsupported database backend".to_string())),
        };
        manager.get_connection().execute_unprepared(sql).await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sales_performance_entries_report")
                    .table(SalesPerformanceEntries::Table)
                    .col(SalesPerformanceEntries::PerformanceDate)
                    .col(SalesPerformanceEntries::UserId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sales_performance_entries_report")
                    .table(SalesPerformanceEntries::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(SalesPerformanceEntries::Table)
                    .drop_column(SalesPerformanceEntries::PerformanceDate)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SalesPerformanceEntries {
    Table,
    PerformanceDate,
    UserId,
}

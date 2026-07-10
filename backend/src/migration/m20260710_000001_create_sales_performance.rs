use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            let connection = manager.get_connection();
            connection.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                "DO $$ DECLARE constraint_name text; BEGIN SELECT conname INTO constraint_name FROM pg_constraint WHERE conrelid = 'sales_payments'::regclass AND pg_get_constraintdef(oid) LIKE '%performance_status%' LIMIT 1; IF constraint_name IS NOT NULL THEN EXECUTE format('ALTER TABLE sales_payments DROP CONSTRAINT %I', constraint_name); END IF; END $$".to_string(),
            )).await?;
            connection.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                "ALTER TABLE sales_payments ADD CONSTRAINT sales_payments_performance_status_check CHECK (performance_status IN ('pending', 'posted', 'cancelled', 'reversed'))".to_string(),
            )).await?;
        } else if manager.get_database_backend() == DatabaseBackend::Sqlite {
            let schema = manager
                .get_connection()
                .query_one(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'sales_payments'"
                        .to_string(),
                ))
                .await?
                .and_then(|row| row.try_get::<String>("", "sql").ok())
                .unwrap_or_default();
            if !schema.contains("cancelled") {
                rebuild_sqlite_sales_payments(manager, true).await?;
            }
        }

        manager
            .create_table(
                Table::create()
                    .table(SalesPerformanceBatches::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::PeriodMonth)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::BatchType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::PaymentCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::ExpertAmount)
                            .decimal_len(12, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::GuideAmount)
                            .decimal_len(12, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::TotalAmount)
                            .decimal_len(12, 2)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SalesPerformanceBatches::PostedByUserId).uuid())
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::PostedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceBatches::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_batches_posted_by")
                            .from(
                                SalesPerformanceBatches::Table,
                                SalesPerformanceBatches::PostedByUserId,
                            )
                            .to(Users::Table, Users::Id),
                    )
                    .check(
                        Expr::col(SalesPerformanceBatches::BatchType)
                            .is_in(["posting", "reversal"]),
                    )
                    .check(Expr::col(SalesPerformanceBatches::PaymentCount).gt(0))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SalesPerformanceEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::BatchId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::PaymentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SalesPerformanceEntries::AllocationId).uuid())
                    .col(ColumnDef::new(SalesPerformanceEntries::AllocationRatio).decimal_len(5, 2))
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::SalesRecordId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::PerformanceRole)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::EntryType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::Amount)
                            .decimal_len(12, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::PeriodMonth)
                            .date()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::SystemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::StoreId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SalesPerformanceEntries::SourceEntryId).uuid())
                    .col(
                        ColumnDef::new(SalesPerformanceEntries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_batch")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::BatchId,
                            )
                            .to(SalesPerformanceBatches::Table, SalesPerformanceBatches::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_payment")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::PaymentId,
                            )
                            .to(SalesPayments::Table, SalesPayments::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_allocation")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::AllocationId,
                            )
                            .to(SalesPaymentAllocations::Table, SalesPaymentAllocations::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_record")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::SalesRecordId,
                            )
                            .to(SalesRecords::Table, SalesRecords::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_user")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::UserId,
                            )
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_system")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::SystemId,
                            )
                            .to(Systems::Table, Systems::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_store")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::StoreId,
                            )
                            .to(Stores::Table, Stores::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_performance_entries_source")
                            .from(
                                SalesPerformanceEntries::Table,
                                SalesPerformanceEntries::SourceEntryId,
                            )
                            .to(SalesPerformanceEntries::Table, SalesPerformanceEntries::Id),
                    )
                    .check(
                        Expr::col(SalesPerformanceEntries::PerformanceRole)
                            .is_in(["expert", "guide"]),
                    )
                    .check(
                        Expr::col(SalesPerformanceEntries::EntryType)
                            .is_in(["earning", "reversal"]),
                    )
                    .check(Expr::col(SalesPerformanceEntries::Amount).ne(0))
                    .index(
                        Index::create()
                            .name("uq_sales_performance_entry_payment_role_user_type")
                            .col(SalesPerformanceEntries::PaymentId)
                            .col(SalesPerformanceEntries::PerformanceRole)
                            .col(SalesPerformanceEntries::UserId)
                            .col(SalesPerformanceEntries::EntryType)
                            .unique(),
                    )
                    .index(
                        Index::create()
                            .name("uq_sales_performance_entry_source")
                            .col(SalesPerformanceEntries::SourceEntryId)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(SalesPerformanceEntries::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SalesPerformanceBatches::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            let connection = manager.get_connection();
            connection.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                "ALTER TABLE sales_payments DROP CONSTRAINT IF EXISTS sales_payments_performance_status_check".to_string(),
            )).await?;
            connection.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                "ALTER TABLE sales_payments ADD CONSTRAINT sales_payments_performance_status_check CHECK (performance_status IN ('pending', 'posted'))".to_string(),
            )).await?;
        } else if manager.get_database_backend() == DatabaseBackend::Sqlite {
            rebuild_sqlite_sales_payments(manager, false).await?;
        }
        Ok(())
    }
}

async fn rebuild_sqlite_sales_payments(
    manager: &SchemaManager<'_>,
    extended_statuses: bool,
) -> Result<(), DbErr> {
    let statuses = if extended_statuses {
        "'pending', 'posted', 'cancelled', 'reversed'"
    } else {
        "'pending', 'posted'"
    };
    let connection = manager.get_connection();
    connection
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = OFF".to_string(),
        ))
        .await?;
    let result = async {
        connection.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("CREATE TABLE sales_payments_new (id TEXT PRIMARY KEY NOT NULL, sales_record_id TEXT NOT NULL REFERENCES sales_records(id), payment_type VARCHAR(32) NOT NULL CHECK (payment_type IN ('initial', 'collection')), paid_amount REAL NOT NULL CHECK (paid_amount > 0), paid_at TEXT NOT NULL, performance_status VARCHAR(32) NOT NULL DEFAULT 'pending' CHECK (performance_status IN ({statuses})), status VARCHAR(32) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'voided')), remark TEXT NULL, created_by_user_id TEXT NOT NULL REFERENCES users(id), created_at TEXT NOT NULL, updated_at TEXT NOT NULL)"),
        )).await?;
        connection.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO sales_payments_new (id, sales_record_id, payment_type, paid_amount, paid_at, performance_status, status, remark, created_by_user_id, created_at, updated_at) SELECT id, sales_record_id, payment_type, CAST(paid_amount AS REAL), paid_at, performance_status, status, remark, created_by_user_id, created_at, updated_at FROM sales_payments".to_string(),
        )).await?;
        connection.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "DROP TABLE sales_payments".to_string(),
        )).await?;
        connection.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "ALTER TABLE sales_payments_new RENAME TO sales_payments".to_string(),
        )).await?;
        Ok::<(), DbErr>(())
    }.await;
    connection
        .execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_keys = ON".to_string(),
        ))
        .await?;
    result
}

#[derive(DeriveIden)]
enum SalesPerformanceBatches {
    Table,
    Id,
    PeriodMonth,
    BatchType,
    PaymentCount,
    ExpertAmount,
    GuideAmount,
    TotalAmount,
    PostedByUserId,
    PostedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum SalesPerformanceEntries {
    Table,
    Id,
    BatchId,
    PaymentId,
    AllocationId,
    AllocationRatio,
    SalesRecordId,
    UserId,
    PerformanceRole,
    EntryType,
    Amount,
    PeriodMonth,
    SystemId,
    StoreId,
    SourceEntryId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Systems {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum Stores {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum SalesRecords {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum SalesPayments {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum SalesPaymentAllocations {
    Table,
    Id,
}

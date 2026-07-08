use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SalesRecords::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SalesRecords::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SalesRecords::RecordGroupId).uuid())
                    .col(ColumnDef::new(SalesRecords::CustomerId).uuid().not_null())
                    .col(ColumnDef::new(SalesRecords::DepartmentId).uuid().not_null())
                    .col(ColumnDef::new(SalesRecords::SaleDate).date().not_null())
                    .col(
                        ColumnDef::new(SalesRecords::DealStatus)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::CustomerType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::DealType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::ContentCategoryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::HandlerUserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::PaidAmount)
                            .decimal_len(12, 2)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::UnpaidAmount)
                            .decimal_len(12, 2)
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(SalesRecords::SystemId).uuid().not_null())
                    .col(ColumnDef::new(SalesRecords::StoreId).uuid().not_null())
                    .col(
                        ColumnDef::new(SalesRecords::CollaborationType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SalesRecords::ExpertUserId).uuid())
                    .col(ColumnDef::new(SalesRecords::ExpertDepartmentId).uuid())
                    .col(ColumnDef::new(SalesRecords::ConsultantUserId).uuid())
                    .col(ColumnDef::new(SalesRecords::ConsultantDepartmentId).uuid())
                    .col(ColumnDef::new(SalesRecords::DoctorUserId).uuid())
                    .col(
                        ColumnDef::new(SalesRecords::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecords::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_customer_id")
                            .from(SalesRecords::Table, SalesRecords::CustomerId)
                            .to(Customers::Table, Customers::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_department_id")
                            .from(SalesRecords::Table, SalesRecords::DepartmentId)
                            .to(Departments::Table, Departments::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_content_category_id")
                            .from(SalesRecords::Table, SalesRecords::ContentCategoryId)
                            .to(ProductCategory::Table, ProductCategory::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_handler_user_id")
                            .from(SalesRecords::Table, SalesRecords::HandlerUserId)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_system_id")
                            .from(SalesRecords::Table, SalesRecords::SystemId)
                            .to(Systems::Table, Systems::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_store_id")
                            .from(SalesRecords::Table, SalesRecords::StoreId)
                            .to(Stores::Table, Stores::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_expert_user_id")
                            .from(SalesRecords::Table, SalesRecords::ExpertUserId)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_expert_department_id")
                            .from(SalesRecords::Table, SalesRecords::ExpertDepartmentId)
                            .to(Departments::Table, Departments::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_consultant_user_id")
                            .from(SalesRecords::Table, SalesRecords::ConsultantUserId)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_consultant_department_id")
                            .from(SalesRecords::Table, SalesRecords::ConsultantDepartmentId)
                            .to(Departments::Table, Departments::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_records_doctor_user_id")
                            .from(SalesRecords::Table, SalesRecords::DoctorUserId)
                            .to(Users::Table, Users::Id),
                    )
                    .check(Expr::col(SalesRecords::DealStatus).is_in(["closed", "not_closed"]))
                    .check(Expr::col(SalesRecords::CustomerType).is_in(["new", "returning"]))
                    .check(Expr::col(SalesRecords::DealType).is_in(["non_salon", "salon"]))
                    .check(
                        Expr::col(SalesRecords::CollaborationType)
                            .is_in(["expert_consultation", "self_sale"]),
                    )
                    .check(Expr::col(SalesRecords::Status).is_in(["active", "voided"]))
                    .check(Expr::col(SalesRecords::PaidAmount).gte(0))
                    .check(Expr::col(SalesRecords::UnpaidAmount).gte(0))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SalesRecordOperationCounts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SalesRecordOperationCounts::SalesRecordId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationCounts::TotalCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationCounts::UsedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationCounts::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationCounts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationCounts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_record_operation_counts_sales_record_id")
                            .from(
                                SalesRecordOperationCounts::Table,
                                SalesRecordOperationCounts::SalesRecordId,
                            )
                            .to(SalesRecords::Table, SalesRecords::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::col(SalesRecordOperationCounts::TotalCount).gt(0))
                    .check(Expr::col(SalesRecordOperationCounts::UsedCount).gte(0))
                    .check(
                        Expr::col(SalesRecordOperationCounts::UsedCount)
                            .lte(Expr::col(SalesRecordOperationCounts::TotalCount)),
                    )
                    .check(
                        Expr::col(SalesRecordOperationCounts::Status).is_in(["active", "voided"]),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SalesRecordOperationUsages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::SalesRecordId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::OperatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::OperatorUserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SalesRecordOperationUsages::DoctorUserId).uuid())
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::OperationCount)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(ColumnDef::new(SalesRecordOperationUsages::Remark).text())
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SalesRecordOperationUsages::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_record_operation_usages_sales_record_id")
                            .from(
                                SalesRecordOperationUsages::Table,
                                SalesRecordOperationUsages::SalesRecordId,
                            )
                            .to(SalesRecords::Table, SalesRecords::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_record_operation_usages_operator_user_id")
                            .from(
                                SalesRecordOperationUsages::Table,
                                SalesRecordOperationUsages::OperatorUserId,
                            )
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sales_record_operation_usages_doctor_user_id")
                            .from(
                                SalesRecordOperationUsages::Table,
                                SalesRecordOperationUsages::DoctorUserId,
                            )
                            .to(Users::Table, Users::Id),
                    )
                    .check(Expr::col(SalesRecordOperationUsages::OperationCount).gt(0))
                    .check(
                        Expr::col(SalesRecordOperationUsages::Status).is_in(["active", "voided"]),
                    )
                    .to_owned(),
            )
            .await?;

        for (name, table, column) in [
            (
                "idx_sales_records_sale_date",
                SalesRecords::Table.into_iden(),
                SalesRecords::SaleDate.into_iden(),
            ),
            (
                "idx_sales_records_customer_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::CustomerId.into_iden(),
            ),
            (
                "idx_sales_records_department_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::DepartmentId.into_iden(),
            ),
            (
                "idx_sales_records_system_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::SystemId.into_iden(),
            ),
            (
                "idx_sales_records_store_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::StoreId.into_iden(),
            ),
            (
                "idx_sales_records_handler_user_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::HandlerUserId.into_iden(),
            ),
            (
                "idx_sales_records_content_category_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::ContentCategoryId.into_iden(),
            ),
            (
                "idx_sales_records_record_group_id",
                SalesRecords::Table.into_iden(),
                SalesRecords::RecordGroupId.into_iden(),
            ),
            (
                "idx_sales_record_operation_counts_status",
                SalesRecordOperationCounts::Table.into_iden(),
                SalesRecordOperationCounts::Status.into_iden(),
            ),
            (
                "idx_sales_record_operation_usages_sales_record_id",
                SalesRecordOperationUsages::Table.into_iden(),
                SalesRecordOperationUsages::SalesRecordId.into_iden(),
            ),
            (
                "idx_sales_record_operation_usages_operated_at",
                SalesRecordOperationUsages::Table.into_iden(),
                SalesRecordOperationUsages::OperatedAt.into_iden(),
            ),
            (
                "idx_sales_record_operation_usages_operator_user_id",
                SalesRecordOperationUsages::Table.into_iden(),
                SalesRecordOperationUsages::OperatorUserId.into_iden(),
            ),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(table)
                        .col(column)
                        .if_not_exists()
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(SalesRecordOperationUsages::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SalesRecordOperationCounts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SalesRecords::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesRecords {
    Table,
    Id,
    RecordGroupId,
    CustomerId,
    DepartmentId,
    SaleDate,
    DealStatus,
    CustomerType,
    DealType,
    ContentCategoryId,
    HandlerUserId,
    PaidAmount,
    UnpaidAmount,
    SystemId,
    StoreId,
    CollaborationType,
    ExpertUserId,
    ExpertDepartmentId,
    ConsultantUserId,
    ConsultantDepartmentId,
    DoctorUserId,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesRecordOperationCounts {
    Table,
    SalesRecordId,
    TotalCount,
    UsedCount,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesRecordOperationUsages {
    Table,
    Id,
    SalesRecordId,
    OperatedAt,
    OperatorUserId,
    DoctorUserId,
    OperationCount,
    Remark,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Customers {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Departments {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProductCategory {
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
enum Users {
    Table,
    Id,
}

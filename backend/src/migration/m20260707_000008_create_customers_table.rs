use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Customers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Customers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Customers::Name).string_len(128).not_null())
                    .col(ColumnDef::new(Customers::CreatorUserId).uuid().not_null())
                    .col(ColumnDef::new(Customers::DepartmentId).uuid().not_null())
                    .col(ColumnDef::new(Customers::SystemId).uuid().not_null())
                    .col(ColumnDef::new(Customers::StoreId).uuid().not_null())
                    .col(ColumnDef::new(Customers::Remark).text())
                    .col(
                        ColumnDef::new(Customers::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(ColumnDef::new(Customers::Attachments).text())
                    .col(
                        ColumnDef::new(Customers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Customers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customers_creator_user_id")
                            .from(Customers::Table, Customers::CreatorUserId)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customers_department_id")
                            .from(Customers::Table, Customers::DepartmentId)
                            .to(Departments::Table, Departments::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customers_system_id")
                            .from(Customers::Table, Customers::SystemId)
                            .to(Systems::Table, Systems::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_customers_store_id")
                            .from(Customers::Table, Customers::StoreId)
                            .to(Stores::Table, Stores::Id),
                    )
                    .check(Expr::col(Customers::Status).is_in(["active", "disabled"]))
                    .to_owned(),
            )
            .await?;

        for (name, column) in [
            ("idx_customers_status", Customers::Status),
            ("idx_customers_creator_user_id", Customers::CreatorUserId),
            ("idx_customers_department_id", Customers::DepartmentId),
            ("idx_customers_system_id", Customers::SystemId),
            ("idx_customers_store_id", Customers::StoreId),
            ("idx_customers_created_at", Customers::CreatedAt),
            ("idx_customers_name", Customers::Name),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(name)
                        .table(Customers::Table)
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
            .drop_table(Table::drop().table(Customers::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden, Copy, Clone)]
enum Customers {
    Table,
    Id,
    Name,
    CreatorUserId,
    DepartmentId,
    SystemId,
    StoreId,
    Remark,
    Status,
    Attachments,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Departments {
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

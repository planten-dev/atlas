use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Products::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Products::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Products::Name).string_len(128).not_null())
                    .col(ColumnDef::new(Products::Category).string_len(64))
                    .col(ColumnDef::new(Products::Series).string_len(128))
                    .col(ColumnDef::new(Products::BrandName).string_len(128))
                    .col(ColumnDef::new(Products::Specification).string_len(255))
                    .col(ColumnDef::new(Products::Unit).string_len(32))
                    .col(
                        ColumnDef::new(Products::UnitPrice)
                            .decimal_len(12, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Products::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Products::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Products::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::col(Products::Status).is_in(["active", "disabled"]))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_products_status")
                    .table(Products::Table)
                    .col(Products::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_products_created_at")
                    .table(Products::Table)
                    .col(Products::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Products::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Products {
    Table,
    Id,
    Name,
    Category,
    Series,
    BrandName,
    Specification,
    Unit,
    UnitPrice,
    Status,
    CreatedAt,
    UpdatedAt,
}

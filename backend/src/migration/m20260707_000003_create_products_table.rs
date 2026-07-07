use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProductCategory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProductCategory::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProductCategory::CategoryName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductCategory::RequiresOperationCount)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ProductCategory::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(ProductCategory::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProductCategory::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::col(ProductCategory::Status).is_in(["active", "disabled"]))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_product_category_category_name")
                    .table(ProductCategory::Table)
                    .col(ProductCategory::CategoryName)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_product_category_status")
                    .table(ProductCategory::Table)
                    .col(ProductCategory::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        seed_product_categories(manager).await?;

        manager
            .create_table(
                Table::create()
                    .table(Products::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Products::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Products::Name).string_len(128).not_null())
                    .col(ColumnDef::new(Products::CategoryId).uuid().not_null())
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
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_products_category_id")
                            .from(Products::Table, Products::CategoryId)
                            .to(ProductCategory::Table, ProductCategory::Id),
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
                    .name("idx_products_category_id")
                    .table(Products::Table)
                    .col(Products::CategoryId)
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
        manager
            .drop_table(
                Table::drop()
                    .table(ProductCategory::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

async fn seed_product_categories(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let now = chrono::Utc::now();
    for (category_name, requires_operation_count) in [
        ("产品", false),
        ("医疗", true),
        ("仪器", true),
        ("卡项", true),
    ] {
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(ProductCategory::Table)
                    .columns([
                        ProductCategory::Id,
                        ProductCategory::CategoryName,
                        ProductCategory::RequiresOperationCount,
                        ProductCategory::Status,
                        ProductCategory::CreatedAt,
                        ProductCategory::UpdatedAt,
                    ])
                    .values_panic([
                        Uuid::new_v4().into(),
                        category_name.into(),
                        requires_operation_count.into(),
                        "active".into(),
                        now.into(),
                        now.into(),
                    ])
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
}

#[derive(DeriveIden)]
enum ProductCategory {
    Table,
    Id,
    CategoryName,
    RequiresOperationCount,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Products {
    Table,
    Id,
    Name,
    CategoryId,
    Series,
    BrandName,
    Specification,
    Unit,
    UnitPrice,
    Status,
    CreatedAt,
    UpdatedAt,
}

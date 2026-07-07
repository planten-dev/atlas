use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Stores::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Stores::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Stores::Name).string_len(128).not_null())
                    .col(ColumnDef::new(Stores::SystemId).uuid().not_null())
                    .col(
                        ColumnDef::new(Stores::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Stores::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Stores::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_stores_system_id")
                            .from(Stores::Table, Stores::SystemId)
                            .to(Systems::Table, Systems::Id),
                    )
                    .check(Expr::col(Stores::Status).is_in(["active", "disabled"]))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_stores_status")
                    .table(Stores::Table)
                    .col(Stores::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_stores_system_id")
                    .table(Stores::Table)
                    .col(Stores::SystemId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_stores_created_at")
                    .table(Stores::Table)
                    .col(Stores::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Stores::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Stores {
    Table,
    Id,
    Name,
    SystemId,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Systems {
    Table,
    Id,
}

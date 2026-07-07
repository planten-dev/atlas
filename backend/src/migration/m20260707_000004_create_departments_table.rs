use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Departments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Departments::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Departments::Source)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Departments::ExternalDepartmentId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Departments::ParentId).uuid())
                    .col(ColumnDef::new(Departments::Name).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Departments::Status)
                            .string_len(32)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Departments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Departments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_departments_parent_id")
                            .from(Departments::Table, Departments::ParentId)
                            .to(Departments::Table, Departments::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_departments_source_external_department_id")
                    .table(Departments::Table)
                    .col(Departments::Source)
                    .col(Departments::ExternalDepartmentId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_departments_parent_id")
                    .table(Departments::Table)
                    .col(Departments::ParentId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Departments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Departments {
    Table,
    Id,
    Source,
    ExternalDepartmentId,
    ParentId,
    Name,
    Status,
    CreatedAt,
    UpdatedAt,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Events::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Events::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Events::ResourceType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Events::ResourceId).uuid())
                    .col(ColumnDef::new(Events::ActorUserId).uuid())
                    .col(ColumnDef::new(Events::EventType).small_integer().not_null())
                    .col(
                        ColumnDef::new(Events::ApprovalStatus)
                            .small_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Events::RequiredApprovalCount).small_integer())
                    .col(ColumnDef::new(Events::TargetEventId).uuid())
                    .col(ColumnDef::new(Events::OldValue).json_binary())
                    .col(ColumnDef::new(Events::NewValue).json_binary())
                    .col(ColumnDef::new(Events::Remark).text())
                    .col(
                        ColumnDef::new(Events::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Events::UpdatedAt).timestamp_with_time_zone())
                    .check(Expr::col(Events::EventType).is_in([0, 1, 2, 3, 4]))
                    .check(Expr::col(Events::ApprovalStatus).is_in([0, 1, 2, 3]))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_events_resource_type_approval_status")
                    .table(Events::Table)
                    .col(Events::ResourceType)
                    .col(Events::ApprovalStatus)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_events_target_event_id")
                    .table(Events::Table)
                    .col(Events::TargetEventId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_events_updated_at")
                    .table(Events::Table)
                    .col(Events::UpdatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // One review per user per target. Rows with a NULL target_event_id
        // (the reviewable create/update/delete events themselves) never
        // collide because both Postgres and SQLite treat NULLs as distinct
        // in unique indexes.
        manager
            .create_index(
                Index::create()
                    .name("uq_events_target_actor")
                    .table(Events::Table)
                    .col(Events::TargetEventId)
                    .col(Events::ActorUserId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Events::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
    ResourceType,
    ResourceId,
    ActorUserId,
    EventType,
    ApprovalStatus,
    RequiredApprovalCount,
    TargetEventId,
    OldValue,
    NewValue,
    Remark,
    CreatedAt,
    UpdatedAt,
}

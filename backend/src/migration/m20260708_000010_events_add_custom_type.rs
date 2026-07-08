use sea_orm_migration::prelude::*;

/// Adds EventType::Custom (5) support to the events table: widens the
/// event_type CHECK to 0..=5 and adds the nullable custom_type column that
/// names the custom event kind. Neither Postgres (anonymous inline CHECK)
/// nor SQLite (no ALTER of CHECK at all) can modify the constraint in
/// place, so the table is rebuilt and existing rows are copied over.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EventsNew::Table)
                    .col(
                        ColumnDef::new(EventsNew::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EventsNew::ResourceType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(EventsNew::ResourceId).uuid())
                    .col(ColumnDef::new(EventsNew::ActorUserId).uuid())
                    .col(
                        ColumnDef::new(EventsNew::EventType)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EventsNew::ApprovalStatus)
                            .small_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EventsNew::RequiredApprovalCount).small_integer())
                    .col(ColumnDef::new(EventsNew::CustomType).string_len(64))
                    .col(ColumnDef::new(EventsNew::TargetEventId).uuid())
                    .col(ColumnDef::new(EventsNew::OldValue).json_binary())
                    .col(ColumnDef::new(EventsNew::NewValue).json_binary())
                    .col(ColumnDef::new(EventsNew::Remark).text())
                    .col(
                        ColumnDef::new(EventsNew::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EventsNew::UpdatedAt).timestamp_with_time_zone())
                    .check(Expr::col(EventsNew::EventType).is_in([0, 1, 2, 3, 4, 5]))
                    .check(Expr::col(EventsNew::ApprovalStatus).is_in([0, 1, 2, 3]))
                    .to_owned(),
            )
            .await?;

        manager
            .exec_stmt(
                Query::insert()
                    .into_table(EventsNew::Table)
                    .columns([
                        EventsNew::Id,
                        EventsNew::ResourceType,
                        EventsNew::ResourceId,
                        EventsNew::ActorUserId,
                        EventsNew::EventType,
                        EventsNew::ApprovalStatus,
                        EventsNew::RequiredApprovalCount,
                        EventsNew::TargetEventId,
                        EventsNew::OldValue,
                        EventsNew::NewValue,
                        EventsNew::Remark,
                        EventsNew::CreatedAt,
                        EventsNew::UpdatedAt,
                    ])
                    .select_from(
                        Query::select()
                            .columns([
                                Events::Id,
                                Events::ResourceType,
                                Events::ResourceId,
                                Events::ActorUserId,
                                Events::EventType,
                                Events::ApprovalStatus,
                                Events::RequiredApprovalCount,
                                Events::TargetEventId,
                                Events::OldValue,
                                Events::NewValue,
                                Events::Remark,
                                Events::CreatedAt,
                                Events::UpdatedAt,
                            ])
                            .from(Events::Table)
                            .to_owned(),
                    )
                    .map_err(|error| DbErr::Migration(error.to_string()))?
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Events::Table).to_owned())
            .await?;
        manager
            .rename_table(
                Table::rename()
                    .table(EventsNew::Table, Events::Table)
                    .to_owned(),
            )
            .await?;

        create_indexes(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rebuild with the original 0..=4 CHECK and no custom_type. Rows
        // with event_type = 5 would violate the narrower constraint, so
        // drop them first.
        manager
            .exec_stmt(
                Query::delete()
                    .from_table(Events::Table)
                    .and_where(Expr::col(Events::EventType).eq(5))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(EventsNew::Table)
                    .col(
                        ColumnDef::new(EventsNew::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EventsNew::ResourceType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(EventsNew::ResourceId).uuid())
                    .col(ColumnDef::new(EventsNew::ActorUserId).uuid())
                    .col(
                        ColumnDef::new(EventsNew::EventType)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EventsNew::ApprovalStatus)
                            .small_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EventsNew::RequiredApprovalCount).small_integer())
                    .col(ColumnDef::new(EventsNew::TargetEventId).uuid())
                    .col(ColumnDef::new(EventsNew::OldValue).json_binary())
                    .col(ColumnDef::new(EventsNew::NewValue).json_binary())
                    .col(ColumnDef::new(EventsNew::Remark).text())
                    .col(
                        ColumnDef::new(EventsNew::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(EventsNew::UpdatedAt).timestamp_with_time_zone())
                    .check(Expr::col(EventsNew::EventType).is_in([0, 1, 2, 3, 4]))
                    .check(Expr::col(EventsNew::ApprovalStatus).is_in([0, 1, 2, 3]))
                    .to_owned(),
            )
            .await?;

        manager
            .exec_stmt(
                Query::insert()
                    .into_table(EventsNew::Table)
                    .columns([
                        EventsNew::Id,
                        EventsNew::ResourceType,
                        EventsNew::ResourceId,
                        EventsNew::ActorUserId,
                        EventsNew::EventType,
                        EventsNew::ApprovalStatus,
                        EventsNew::RequiredApprovalCount,
                        EventsNew::TargetEventId,
                        EventsNew::OldValue,
                        EventsNew::NewValue,
                        EventsNew::Remark,
                        EventsNew::CreatedAt,
                        EventsNew::UpdatedAt,
                    ])
                    .select_from(
                        Query::select()
                            .columns([
                                Events::Id,
                                Events::ResourceType,
                                Events::ResourceId,
                                Events::ActorUserId,
                                Events::EventType,
                                Events::ApprovalStatus,
                                Events::RequiredApprovalCount,
                                Events::TargetEventId,
                                Events::OldValue,
                                Events::NewValue,
                                Events::Remark,
                                Events::CreatedAt,
                                Events::UpdatedAt,
                            ])
                            .from(Events::Table)
                            .to_owned(),
                    )
                    .map_err(|error| DbErr::Migration(error.to_string()))?
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Events::Table).to_owned())
            .await?;
        manager
            .rename_table(
                Table::rename()
                    .table(EventsNew::Table, Events::Table)
                    .to_owned(),
            )
            .await?;

        create_indexes(manager).await?;

        Ok(())
    }
}

async fn create_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

#[derive(DeriveIden)]
enum EventsNew {
    Table,
    Id,
    ResourceType,
    ResourceId,
    ActorUserId,
    EventType,
    ApprovalStatus,
    RequiredApprovalCount,
    CustomType,
    TargetEventId,
    OldValue,
    NewValue,
    Remark,
    CreatedAt,
    UpdatedAt,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserProfiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserProfiles::UserId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserProfiles::Name).string_len(128))
                    .col(ColumnDef::new(UserProfiles::AvatarUrl).text())
                    .col(ColumnDef::new(UserProfiles::Mobile).string_len(32))
                    .col(ColumnDef::new(UserProfiles::HideMobile).boolean())
                    .col(ColumnDef::new(UserProfiles::Telephone).string_len(32))
                    .col(ColumnDef::new(UserProfiles::JobNumber).string_len(64))
                    .col(ColumnDef::new(UserProfiles::Title).string_len(128))
                    .col(ColumnDef::new(UserProfiles::Email).string_len(255))
                    .col(ColumnDef::new(UserProfiles::OrgEmail).string_len(255))
                    .col(ColumnDef::new(UserProfiles::WorkPlace).string_len(255))
                    .col(ColumnDef::new(UserProfiles::Remark).text())
                    .col(ColumnDef::new(UserProfiles::DepartmentExternalIds).text())
                    .col(ColumnDef::new(UserProfiles::IsAdmin).boolean())
                    .col(ColumnDef::new(UserProfiles::IsBoss).boolean())
                    .col(ColumnDef::new(UserProfiles::IsActive).boolean())
                    .col(ColumnDef::new(UserProfiles::IsSenior).boolean())
                    .col(ColumnDef::new(UserProfiles::HiredAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(UserProfiles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserProfiles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_profiles_user_id")
                            .from(UserProfiles::Table, UserProfiles::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(UserProfiles::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum UserProfiles {
    Table,
    UserId,
    Name,
    AvatarUrl,
    Mobile,
    HideMobile,
    Telephone,
    JobNumber,
    Title,
    Email,
    OrgEmail,
    WorkPlace,
    Remark,
    DepartmentExternalIds,
    IsAdmin,
    IsBoss,
    IsActive,
    IsSenior,
    HiredAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

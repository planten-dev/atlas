use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Roles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Roles::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Roles::Code)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Roles::Name).string_len(128).not_null())
                    .col(ColumnDef::new(Roles::Kind).string_len(16).not_null())
                    .col(ColumnDef::new(Roles::Priority).integer().not_null())
                    .col(
                        ColumnDef::new(Roles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Roles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RoleInheritances::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoleInheritances::ChildRoleId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleInheritances::ParentRoleId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleInheritances::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(RoleInheritances::ChildRoleId)
                            .col(RoleInheritances::ParentRoleId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_inheritances_child_role_id")
                            .from(RoleInheritances::Table, RoleInheritances::ChildRoleId)
                            .to(Roles::Table, Roles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_role_inheritances_parent_role_id")
                            .from(RoleInheritances::Table, RoleInheritances::ParentRoleId)
                            .to(Roles::Table, Roles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_role_inheritances_parent_role_id")
                    .table(RoleInheritances::Table)
                    .col(RoleInheritances::ParentRoleId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserRoles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserRoles::UserId).uuid().not_null())
                    .col(ColumnDef::new(UserRoles::RoleId).uuid().not_null())
                    .col(
                        ColumnDef::new(UserRoles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserRoles::UserId)
                            .col(UserRoles::RoleId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_roles_user_id")
                            .from(UserRoles::Table, UserRoles::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_roles_role_id")
                            .from(UserRoles::Table, UserRoles::RoleId)
                            .to(Roles::Table, Roles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_roles_role_id")
                    .table(UserRoles::Table)
                    .col(UserRoles::RoleId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PermissionPolicies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PermissionPolicies::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PermissionPolicies::SubjectKind)
                            .string_len(8)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionPolicies::SubjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionPolicies::Object)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionPolicies::Action)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionPolicies::Effect)
                            .string_len(8)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionPolicies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_permission_policies_subject_object_action")
                    .table(PermissionPolicies::Table)
                    .col(PermissionPolicies::SubjectKind)
                    .col(PermissionPolicies::SubjectId)
                    .col(PermissionPolicies::Object)
                    .col(PermissionPolicies::Action)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_permission_policies_subject")
                    .table(PermissionPolicies::Table)
                    .col(PermissionPolicies::SubjectKind)
                    .col(PermissionPolicies::SubjectId)
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
                    .table(PermissionPolicies::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(UserRoles::Table).if_exists().to_owned())
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(RoleInheritances::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Roles::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Roles {
    Table,
    Id,
    Code,
    Name,
    Kind,
    Priority,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RoleInheritances {
    Table,
    ChildRoleId,
    ParentRoleId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UserRoles {
    Table,
    UserId,
    RoleId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum PermissionPolicies {
    Table,
    Id,
    SubjectKind,
    SubjectId,
    Object,
    Action,
    Effect,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

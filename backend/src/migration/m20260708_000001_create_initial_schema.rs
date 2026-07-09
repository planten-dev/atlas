use sea_orm_migration::prelude::*;
use uuid::Uuid;

const SUPER_ADMIN_ROLE_ID: &str = "00000000-0000-0000-0000-000000000001";
const SUPER_ADMIN_POLICY_ID: &str = "00000000-0000-0000-0000-000000000002";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_auth_tables(manager).await?;
        create_authz_tables(manager).await?;
        create_product_tables(manager).await?;
        create_departments_table(manager).await?;
        create_systems_table(manager).await?;
        create_stores_table(manager).await?;
        create_events_table(manager).await?;
        create_user_profiles_table(manager).await?;
        create_customers_table(manager).await?;
        create_sales_record_tables(manager).await?;

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
                    .table(SalesPaymentAllocations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SalesPayments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(SalesRecordLines::Table)
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
        manager
            .drop_table(Table::drop().table(Customers::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(UserProfiles::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Events::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Stores::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Systems::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Departments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
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
        manager
            .drop_table(
                Table::drop()
                    .table(OauthLoginStates::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(AuthSessions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).if_exists().to_owned())
            .await?;

        Ok(())
    }
}

async fn create_auth_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Users::Table)
                .if_not_exists()
                .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key())
                .col(
                    ColumnDef::new(Users::DingtalkUserId)
                        .string_len(128)
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(Users::Status)
                        .string_len(32)
                        .not_null()
                        .default("active"),
                )
                .col(
                    ColumnDef::new(Users::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Users::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(Users::LastLoginAt).timestamp_with_time_zone())
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(AuthSessions::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(AuthSessions::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(AuthSessions::UserId).uuid().not_null())
                .col(
                    ColumnDef::new(AuthSessions::SessionTokenHash)
                        .string_len(128)
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(AuthSessions::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(AuthSessions::LastSeenAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(AuthSessions::ExpiresAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(AuthSessions::RevokedAt).timestamp_with_time_zone())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_auth_sessions_user_id")
                        .from(AuthSessions::Table, AuthSessions::UserId)
                        .to(Users::Table, Users::Id),
                )
                .to_owned(),
        )
        .await?;

    for (name, column) in [
        ("idx_auth_sessions_user_id", AuthSessions::UserId),
        ("idx_auth_sessions_expires_at", AuthSessions::ExpiresAt),
    ] {
        manager
            .create_index(
                Index::create()
                    .name(name)
                    .table(AuthSessions::Table)
                    .col(column)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    }

    manager
        .create_table(
            Table::create()
                .table(OauthLoginStates::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(OauthLoginStates::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(OauthLoginStates::Provider)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OauthLoginStates::StateHash)
                        .string_len(128)
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(OauthLoginStates::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OauthLoginStates::ExpiresAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(OauthLoginStates::ConsumedAt).timestamp_with_time_zone())
                .to_owned(),
        )
        .await?;

    for (name, column) in [
        (
            "idx_oauth_login_states_provider",
            OauthLoginStates::Provider,
        ),
        (
            "idx_oauth_login_states_expires_at",
            OauthLoginStates::ExpiresAt,
        ),
    ] {
        manager
            .create_index(
                Index::create()
                    .name(name)
                    .table(OauthLoginStates::Table)
                    .col(column)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
}

async fn create_authz_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

    seed_super_admin(manager).await?;

    Ok(())
}

async fn seed_super_admin(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let now = chrono::Utc::now();
    let super_admin_role_id = Uuid::parse_str(SUPER_ADMIN_ROLE_ID)
        .map_err(|error| DbErr::Custom(format!("invalid super admin role id: {error}")))?;
    let super_admin_policy_id = Uuid::parse_str(SUPER_ADMIN_POLICY_ID)
        .map_err(|error| DbErr::Custom(format!("invalid super admin policy id: {error}")))?;

    manager
        .exec_stmt(
            Query::insert()
                .into_table(Roles::Table)
                .columns([
                    Roles::Id,
                    Roles::Code,
                    Roles::Name,
                    Roles::Kind,
                    Roles::Priority,
                    Roles::CreatedAt,
                    Roles::UpdatedAt,
                ])
                .values_panic([
                    super_admin_role_id.into(),
                    "super_admin".into(),
                    "超级管理员".into(),
                    "custom".into(),
                    1.into(),
                    now.into(),
                    now.into(),
                ])
                .on_conflict(OnConflict::column(Roles::Code).do_nothing().to_owned())
                .to_owned(),
        )
        .await?;

    manager
        .exec_stmt(
            Query::insert()
                .into_table(PermissionPolicies::Table)
                .columns([
                    PermissionPolicies::Id,
                    PermissionPolicies::SubjectKind,
                    PermissionPolicies::SubjectId,
                    PermissionPolicies::Object,
                    PermissionPolicies::Action,
                    PermissionPolicies::Effect,
                    PermissionPolicies::CreatedAt,
                ])
                .values_panic([
                    super_admin_policy_id.into(),
                    "role".into(),
                    super_admin_role_id.into(),
                    "*".into(),
                    "*".into(),
                    "allow".into(),
                    now.into(),
                ])
                .on_conflict(
                    OnConflict::columns([
                        PermissionPolicies::SubjectKind,
                        PermissionPolicies::SubjectId,
                        PermissionPolicies::Object,
                        PermissionPolicies::Action,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .to_owned(),
        )
        .await?;

    Ok(())
}

async fn create_product_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

    for (name, column) in [
        ("idx_products_status", Products::Status),
        ("idx_products_category_id", Products::CategoryId),
        ("idx_products_created_at", Products::CreatedAt),
    ] {
        manager
            .create_index(
                Index::create()
                    .name(name)
                    .table(Products::Table)
                    .col(column)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
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
                    .on_conflict(
                        OnConflict::column(ProductCategory::CategoryName)
                            .do_nothing()
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
}

async fn create_departments_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

async fn create_systems_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Systems::Table)
                .if_not_exists()
                .col(ColumnDef::new(Systems::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Systems::Name).string_len(128).not_null())
                .col(
                    ColumnDef::new(Systems::Status)
                        .string_len(32)
                        .not_null()
                        .default("active"),
                )
                .col(
                    ColumnDef::new(Systems::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Systems::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .check(Expr::col(Systems::Status).is_in(["active", "disabled"]))
                .to_owned(),
        )
        .await?;

    for (name, column) in [
        ("idx_systems_status", Systems::Status),
        ("idx_systems_created_at", Systems::CreatedAt),
    ] {
        manager
            .create_index(
                Index::create()
                    .name(name)
                    .table(Systems::Table)
                    .col(column)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
}

async fn create_stores_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

    for (name, column) in [
        ("idx_stores_status", Stores::Status),
        ("idx_stores_system_id", Stores::SystemId),
        ("idx_stores_created_at", Stores::CreatedAt),
    ] {
        manager
            .create_index(
                Index::create()
                    .name(name)
                    .table(Stores::Table)
                    .col(column)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    }

    Ok(())
}

async fn create_events_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
                .col(ColumnDef::new(Events::CustomType).string_len(64))
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
                .check(Expr::col(Events::EventType).is_in([0, 1, 2, 3, 4, 5]))
                .check(Expr::col(Events::ApprovalStatus).is_in([0, 1, 2, 3]))
                .to_owned(),
        )
        .await?;

    for (name, columns) in [
        (
            "idx_events_resource_type_approval_status",
            vec![Events::ResourceType, Events::ApprovalStatus],
        ),
        ("idx_events_target_event_id", vec![Events::TargetEventId]),
        ("idx_events_updated_at", vec![Events::UpdatedAt]),
        (
            "uq_events_target_actor",
            vec![Events::TargetEventId, Events::ActorUserId],
        ),
    ] {
        let mut index = Index::create();
        index.name(name).table(Events::Table).if_not_exists();
        if name == "uq_events_target_actor" {
            index.unique();
        }
        for column in columns {
            index.col(column);
        }
        manager.create_index(index.to_owned()).await?;
    }

    Ok(())
}

async fn create_user_profiles_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

async fn create_customers_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

async fn create_sales_record_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
                .col(
                    ColumnDef::new(SalesRecords::RecordType)
                        .string_len(32)
                        .not_null(),
                )
                .col(ColumnDef::new(SalesRecords::CustomerId).uuid().not_null())
                .col(ColumnDef::new(SalesRecords::RecordDate).date().not_null())
                .col(ColumnDef::new(SalesRecords::CustomerType).string_len(32))
                .col(ColumnDef::new(SalesRecords::DealType).string_len(32))
                .col(ColumnDef::new(SalesRecords::SystemId).uuid().not_null())
                .col(ColumnDef::new(SalesRecords::StoreId).uuid().not_null())
                .col(
                    ColumnDef::new(SalesRecords::HandlerUserId)
                        .uuid()
                        .not_null(),
                )
                .col(ColumnDef::new(SalesRecords::ExpertUserId).uuid())
                .col(ColumnDef::new(SalesRecords::ConsultantUserId).uuid())
                .col(ColumnDef::new(SalesRecords::DoctorUserId).uuid())
                .col(ColumnDef::new(SalesRecords::Remark).text())
                .col(
                    ColumnDef::new(SalesRecords::Status)
                        .string_len(32)
                        .not_null()
                        .default("active"),
                )
                .col(
                    ColumnDef::new(SalesRecords::CreatedByUserId)
                        .uuid()
                        .not_null(),
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
                        .name("fk_sales_records_consultant_user_id")
                        .from(SalesRecords::Table, SalesRecords::ConsultantUserId)
                        .to(Users::Table, Users::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_records_doctor_user_id")
                        .from(SalesRecords::Table, SalesRecords::DoctorUserId)
                        .to(Users::Table, Users::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_records_created_by_user_id")
                        .from(SalesRecords::Table, SalesRecords::CreatedByUserId)
                        .to(Users::Table, Users::Id),
                )
                .check(Expr::col(SalesRecords::RecordType).is_in(["sale", "service"]))
                .check(
                    Expr::col(SalesRecords::CustomerType)
                        .is_null()
                        .or(Expr::col(SalesRecords::CustomerType).is_in(["new", "returning"])),
                )
                .check(
                    Expr::col(SalesRecords::DealType)
                        .is_null()
                        .or(Expr::col(SalesRecords::DealType).is_in(["non_salon", "salon"])),
                )
                .check(Expr::col(SalesRecords::Status).is_in(["active", "voided"]))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(SalesRecordLines::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SalesRecordLines::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(SalesRecordLines::SalesRecordId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesRecordLines::ProductId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesRecordLines::ItemName)
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesRecordLines::ReceivableAmount)
                        .decimal_len(12, 2)
                        .not_null()
                        .default(0),
                )
                .col(ColumnDef::new(SalesRecordLines::OperationTotalCount).integer())
                .col(ColumnDef::new(SalesRecordLines::Remark).text())
                .col(
                    ColumnDef::new(SalesRecordLines::Status)
                        .string_len(32)
                        .not_null()
                        .default("active"),
                )
                .col(
                    ColumnDef::new(SalesRecordLines::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesRecordLines::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_record_lines_sales_record_id")
                        .from(SalesRecordLines::Table, SalesRecordLines::SalesRecordId)
                        .to(SalesRecords::Table, SalesRecords::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_record_lines_product_id")
                        .from(SalesRecordLines::Table, SalesRecordLines::ProductId)
                        .to(Products::Table, Products::Id),
                )
                .check(Expr::col(SalesRecordLines::ReceivableAmount).gte(0))
                .check(
                    Expr::col(SalesRecordLines::OperationTotalCount)
                        .is_null()
                        .or(Expr::col(SalesRecordLines::OperationTotalCount).gt(0)),
                )
                .check(Expr::col(SalesRecordLines::Status).is_in(["active", "voided"]))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(SalesPayments::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SalesPayments::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(SalesPayments::SalesRecordId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPayments::PaymentType)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPayments::PaidAmount)
                        .decimal_len(12, 2)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPayments::PaidAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPayments::PerformanceStatus)
                        .string_len(32)
                        .not_null()
                        .default("pending"),
                )
                .col(
                    ColumnDef::new(SalesPayments::Status)
                        .string_len(32)
                        .not_null()
                        .default("active"),
                )
                .col(ColumnDef::new(SalesPayments::Remark).text())
                .col(
                    ColumnDef::new(SalesPayments::CreatedByUserId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPayments::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPayments::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_payments_sales_record_id")
                        .from(SalesPayments::Table, SalesPayments::SalesRecordId)
                        .to(SalesRecords::Table, SalesRecords::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_payments_created_by_user_id")
                        .from(SalesPayments::Table, SalesPayments::CreatedByUserId)
                        .to(Users::Table, Users::Id),
                )
                .check(Expr::col(SalesPayments::PaymentType).is_in(["initial", "collection"]))
                .check(Expr::col(SalesPayments::PaidAmount).gt(0))
                .check(Expr::col(SalesPayments::PerformanceStatus).is_in(["pending", "posted"]))
                .check(Expr::col(SalesPayments::Status).is_in(["active", "voided"]))
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(SalesPaymentAllocations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SalesPaymentAllocations::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(SalesPaymentAllocations::PaymentId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPaymentAllocations::GuideUserId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPaymentAllocations::AllocationRatio)
                        .decimal_len(5, 2)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPaymentAllocations::AllocatedAmount)
                        .decimal_len(12, 2)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPaymentAllocations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SalesPaymentAllocations::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_payment_allocations_payment_id")
                        .from(
                            SalesPaymentAllocations::Table,
                            SalesPaymentAllocations::PaymentId,
                        )
                        .to(SalesPayments::Table, SalesPayments::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sales_payment_allocations_guide_user_id")
                        .from(
                            SalesPaymentAllocations::Table,
                            SalesPaymentAllocations::GuideUserId,
                        )
                        .to(Users::Table, Users::Id),
                )
                .check(
                    Expr::col(SalesPaymentAllocations::AllocationRatio)
                        .gt(0)
                        .and(Expr::col(SalesPaymentAllocations::AllocationRatio).lte(100)),
                )
                .check(Expr::col(SalesPaymentAllocations::AllocatedAmount).gte(0))
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("uq_sales_payment_allocations_payment_guide")
                .table(SalesPaymentAllocations::Table)
                .col(SalesPaymentAllocations::PaymentId)
                .col(SalesPaymentAllocations::GuideUserId)
                .unique()
                .if_not_exists()
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(SalesRecordOperationCounts::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SalesRecordOperationCounts::SalesRecordLineId)
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
                        .name("fk_sales_record_operation_counts_sales_record_line_id")
                        .from(
                            SalesRecordOperationCounts::Table,
                            SalesRecordOperationCounts::SalesRecordLineId,
                        )
                        .to(SalesRecordLines::Table, SalesRecordLines::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::col(SalesRecordOperationCounts::TotalCount).gt(0))
                .check(Expr::col(SalesRecordOperationCounts::UsedCount).gte(0))
                .check(
                    Expr::col(SalesRecordOperationCounts::UsedCount)
                        .lte(Expr::col(SalesRecordOperationCounts::TotalCount)),
                )
                .check(Expr::col(SalesRecordOperationCounts::Status).is_in(["active", "voided"]))
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
                    ColumnDef::new(SalesRecordOperationUsages::SalesRecordLineId)
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
                        .name("fk_sales_record_operation_usages_sales_record_line_id")
                        .from(
                            SalesRecordOperationUsages::Table,
                            SalesRecordOperationUsages::SalesRecordLineId,
                        )
                        .to(SalesRecordLines::Table, SalesRecordLines::Id),
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
                .check(Expr::col(SalesRecordOperationUsages::Status).is_in(["active", "voided"]))
                .to_owned(),
        )
        .await?;

    for (name, table, column) in [
        (
            "idx_sales_records_record_date",
            SalesRecords::Table.into_iden(),
            SalesRecords::RecordDate.into_iden(),
        ),
        (
            "idx_sales_records_record_type",
            SalesRecords::Table.into_iden(),
            SalesRecords::RecordType.into_iden(),
        ),
        (
            "idx_sales_records_customer_id",
            SalesRecords::Table.into_iden(),
            SalesRecords::CustomerId.into_iden(),
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
            "idx_sales_record_lines_sales_record_id",
            SalesRecordLines::Table.into_iden(),
            SalesRecordLines::SalesRecordId.into_iden(),
        ),
        (
            "idx_sales_record_lines_product_id",
            SalesRecordLines::Table.into_iden(),
            SalesRecordLines::ProductId.into_iden(),
        ),
        (
            "idx_sales_payments_sales_record_id",
            SalesPayments::Table.into_iden(),
            SalesPayments::SalesRecordId.into_iden(),
        ),
        (
            "idx_sales_payments_paid_at",
            SalesPayments::Table.into_iden(),
            SalesPayments::PaidAt.into_iden(),
        ),
        (
            "idx_sales_payment_allocations_payment_id",
            SalesPaymentAllocations::Table.into_iden(),
            SalesPaymentAllocations::PaymentId.into_iden(),
        ),
        (
            "idx_sales_payment_allocations_guide_user_id",
            SalesPaymentAllocations::Table.into_iden(),
            SalesPaymentAllocations::GuideUserId.into_iden(),
        ),
        (
            "idx_sales_record_operation_counts_status",
            SalesRecordOperationCounts::Table.into_iden(),
            SalesRecordOperationCounts::Status.into_iden(),
        ),
        (
            "idx_sales_record_operation_usages_sales_record_line_id",
            SalesRecordOperationUsages::Table.into_iden(),
            SalesRecordOperationUsages::SalesRecordLineId.into_iden(),
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

#[derive(DeriveIden, Copy, Clone)]
enum Users {
    Table,
    Id,
    DingtalkUserId,
    Status,
    CreatedAt,
    UpdatedAt,
    LastLoginAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum AuthSessions {
    Table,
    Id,
    UserId,
    SessionTokenHash,
    CreatedAt,
    LastSeenAt,
    ExpiresAt,
    RevokedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum OauthLoginStates {
    Table,
    Id,
    Provider,
    StateHash,
    CreatedAt,
    ExpiresAt,
    ConsumedAt,
}

#[derive(DeriveIden, Copy, Clone)]
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

#[derive(DeriveIden, Copy, Clone)]
enum RoleInheritances {
    Table,
    ChildRoleId,
    ParentRoleId,
    CreatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum UserRoles {
    Table,
    UserId,
    RoleId,
    CreatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
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

#[derive(DeriveIden, Copy, Clone)]
enum ProductCategory {
    Table,
    Id,
    CategoryName,
    RequiresOperationCount,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
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

#[derive(DeriveIden, Copy, Clone)]
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

#[derive(DeriveIden, Copy, Clone)]
enum Systems {
    Table,
    Id,
    Name,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum Stores {
    Table,
    Id,
    Name,
    SystemId,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum Events {
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

#[derive(DeriveIden, Copy, Clone)]
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

#[derive(DeriveIden, Copy, Clone)]
enum Customers {
    Table,
    Id,
    Name,
    CreatorUserId,
    SystemId,
    StoreId,
    Remark,
    Status,
    Attachments,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesRecords {
    Table,
    Id,
    RecordType,
    CustomerId,
    RecordDate,
    CustomerType,
    DealType,
    SystemId,
    StoreId,
    HandlerUserId,
    ExpertUserId,
    ConsultantUserId,
    DoctorUserId,
    Remark,
    Status,
    CreatedByUserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesRecordLines {
    Table,
    Id,
    SalesRecordId,
    ProductId,
    ItemName,
    ReceivableAmount,
    OperationTotalCount,
    Remark,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesPayments {
    Table,
    Id,
    SalesRecordId,
    PaymentType,
    PaidAmount,
    PaidAt,
    PerformanceStatus,
    Status,
    Remark,
    CreatedByUserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesPaymentAllocations {
    Table,
    Id,
    PaymentId,
    GuideUserId,
    AllocationRatio,
    AllocatedAmount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Copy, Clone)]
enum SalesRecordOperationCounts {
    Table,
    SalesRecordLineId,
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
    SalesRecordLineId,
    OperatedAt,
    OperatorUserId,
    DoctorUserId,
    OperationCount,
    Remark,
    Status,
    CreatedAt,
    UpdatedAt,
}

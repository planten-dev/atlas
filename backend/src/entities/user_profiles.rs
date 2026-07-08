use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "user_profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub mobile: Option<String>,
    pub hide_mobile: Option<bool>,
    pub telephone: Option<String>,
    pub job_number: Option<String>,
    pub title: Option<String>,
    pub email: Option<String>,
    pub org_email: Option<String>,
    pub work_place: Option<String>,
    pub remark: Option<String>,
    pub department_external_ids: Option<String>,
    pub is_admin: Option<bool>,
    pub is_boss: Option<bool>,
    pub is_active: Option<bool>,
    pub is_senior: Option<bool>,
    pub hired_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

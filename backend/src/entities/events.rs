use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum EventType {
    #[sea_orm(num_value = 0)]
    Create,
    #[sea_orm(num_value = 1)]
    Update,
    #[sea_orm(num_value = 2)]
    Delete,
    #[sea_orm(num_value = 3)]
    Approve,
    #[sea_orm(num_value = 4)]
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i16", db_type = "SmallInteger")]
pub enum ApprovalStatus {
    #[sea_orm(num_value = 0)]
    None,
    #[sea_orm(num_value = 1)]
    Pending,
    #[sea_orm(num_value = 2)]
    Approved,
    #[sea_orm(num_value = 3)]
    Rejected,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub event_type: EventType,
    pub approval_status: ApprovalStatus,
    pub required_approval_count: Option<i16>,
    pub target_event_id: Option<Uuid>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub old_value: Option<Json>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub new_value: Option<Json>,
    #[sea_orm(column_type = "Text", nullable)]
    pub remark: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

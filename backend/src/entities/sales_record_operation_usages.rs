use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_record_operation_usages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sales_record_line_id: Uuid,
    pub operated_at: DateTimeUtc,
    pub operator_user_id: Uuid,
    pub doctor_user_id: Option<Uuid>,
    pub operation_count: i32,
    pub remark: Option<String>,
    pub status: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sales_record_lines::Entity",
        from = "Column::SalesRecordLineId",
        to = "super::sales_record_lines::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    SalesRecordLine,
}

impl Related<super::sales_record_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecordLine.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

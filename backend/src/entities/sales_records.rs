use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub record_type: String,
    pub customer_id: Uuid,
    pub record_date: Date,
    pub customer_type: Option<String>,
    pub deal_type: Option<String>,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub handler_user_id: Uuid,
    pub expert_user_id: Option<Uuid>,
    pub consultant_user_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub remark: Option<String>,
    pub status: String,
    pub created_by_user_id: Uuid,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::sales_record_lines::Entity")]
    SalesRecordLines,
    #[sea_orm(has_many = "super::sales_payments::Entity")]
    SalesPayments,
}

impl Related<super::sales_record_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecordLines.def()
    }
}

impl Related<super::sales_payments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesPayments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub record_group_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub department_id: Uuid,
    pub sale_date: Date,
    pub deal_status: String,
    pub customer_type: String,
    pub deal_type: String,
    pub content_category_id: Uuid,
    pub handler_user_id: Uuid,
    pub paid_amount: Decimal,
    pub unpaid_amount: Decimal,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub collaboration_type: String,
    pub expert_user_id: Option<Uuid>,
    pub expert_department_id: Option<Uuid>,
    pub consultant_user_id: Option<Uuid>,
    pub consultant_department_id: Option<Uuid>,
    pub doctor_user_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::sales_record_operation_counts::Entity")]
    SalesRecordOperationCount,
    #[sea_orm(has_many = "super::sales_record_operation_usages::Entity")]
    SalesRecordOperationUsages,
}

impl Related<super::sales_record_operation_counts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecordOperationCount.def()
    }
}

impl Related<super::sales_record_operation_usages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecordOperationUsages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_payments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub payment_type: String,
    pub paid_amount: Decimal,
    pub paid_at: DateTimeUtc,
    pub performance_status: String,
    pub status: String,
    pub remark: Option<String>,
    pub created_by_user_id: Uuid,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sales_records::Entity",
        from = "Column::SalesRecordId",
        to = "super::sales_records::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    SalesRecord,
    #[sea_orm(has_many = "super::sales_payment_allocations::Entity")]
    SalesPaymentAllocations,
}

impl Related<super::sales_records::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecord.def()
    }
}

impl Related<super::sales_payment_allocations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesPaymentAllocations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

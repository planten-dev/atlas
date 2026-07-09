use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_payment_allocations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub payment_id: Uuid,
    pub guide_user_id: Uuid,
    pub allocation_ratio: Decimal,
    pub allocated_amount: Decimal,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sales_payments::Entity",
        from = "Column::PaymentId",
        to = "super::sales_payments::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SalesPayment,
}

impl Related<super::sales_payments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesPayment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

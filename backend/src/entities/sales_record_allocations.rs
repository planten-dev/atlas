use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_record_allocations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub guide_user_id: Uuid,
    pub allocation_ratio: Decimal,
    pub allocated_amount: Decimal,
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
        on_delete = "Cascade"
    )]
    SalesRecord,
}

impl Related<super::sales_records::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecord.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

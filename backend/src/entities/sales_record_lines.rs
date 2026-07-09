use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_record_lines")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sales_record_id: Uuid,
    pub product_id: Uuid,
    pub item_name: String,
    pub receivable_amount: Decimal,
    pub operation_total_count: Option<i32>,
    pub remark: Option<String>,
    pub status: String,
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
    #[sea_orm(
        belongs_to = "super::products::Entity",
        from = "Column::ProductId",
        to = "super::products::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Product,
    #[sea_orm(has_one = "super::sales_record_operation_counts::Entity")]
    SalesRecordOperationCount,
    #[sea_orm(has_many = "super::sales_record_operation_usages::Entity")]
    SalesRecordOperationUsages,
}

impl Related<super::sales_records::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesRecord.def()
    }
}

impl Related<super::products::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
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

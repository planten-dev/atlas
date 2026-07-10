use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_performance_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub batch_id: Uuid,
    pub payment_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub allocation_ratio: Option<Decimal>,
    pub sales_record_id: Uuid,
    pub user_id: Uuid,
    pub performance_role: String,
    pub entry_type: String,
    pub amount: Decimal,
    pub period_month: Date,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub source_entry_id: Option<Uuid>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

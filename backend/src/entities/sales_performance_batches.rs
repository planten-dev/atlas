use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sales_performance_batches")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub period_month: Date,
    pub batch_type: String,
    pub record_count: i32,
    pub expert_amount: Decimal,
    pub guide_amount: Decimal,
    pub total_amount: Decimal,
    pub posted_by_user_id: Option<Uuid>,
    pub posted_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

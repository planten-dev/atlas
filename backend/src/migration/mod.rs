use sea_orm_migration::prelude::*;

mod m20260708_000001_create_initial_schema;
mod m20260709_000001_add_required_approver_ids_to_events;
mod m20260710_000001_create_sales_performance;
mod m20260710_000002_add_performance_date;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260708_000001_create_initial_schema::Migration),
            Box::new(m20260709_000001_add_required_approver_ids_to_events::Migration),
            Box::new(m20260710_000001_create_sales_performance::Migration),
            Box::new(m20260710_000002_add_performance_date::Migration),
        ]
    }
}

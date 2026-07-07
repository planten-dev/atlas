use sea_orm_migration::prelude::*;

mod m20260707_000001_create_auth_tables;
mod m20260707_000002_create_authz_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260707_000001_create_auth_tables::Migration),
            Box::new(m20260707_000002_create_authz_tables::Migration),
        ]
    }
}

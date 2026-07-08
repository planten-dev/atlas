use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::stores, repositories::RepositoryError};

#[derive(Clone)]
pub struct StoreRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewStore {
    pub name: String,
    pub system_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct StoreChanges {
    pub name: Option<String>,
    pub system_id: Option<Uuid>,
    pub status: Option<String>,
}

impl StoreChanges {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.system_id.is_none() && self.status.is_none()
    }
}

impl StoreRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "info", skip(self, store), fields(name = %store.name, system_id = %store.system_id, status = %store.status))]
    pub async fn create_store(
        &self,
        store: NewStore,
        now: DateTime<Utc>,
    ) -> Result<stores::Model, RepositoryError> {
        validate_required("name", &store.name)?;
        validate_required("status", &store.status)?;

        let store = stores::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(store.name),
            system_id: Set(store.system_id),
            status: Set(store.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(store_id = %store.id, "created store");
        Ok(store)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        store_id: Uuid,
    ) -> Result<Option<stores::Model>, RepositoryError> {
        let store = stores::Entity::find_by_id(store_id).one(&self.db).await?;

        debug!(found = store.is_some(), %store_id, "looked up store by id");
        Ok(store)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_stores(
        &self,
        status_filter: Option<&str>,
        system_id: Option<Uuid>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<stores::Model>, u64), RepositoryError> {
        let mut query = stores::Entity::find()
            .order_by_asc(stores::Column::CreatedAt)
            .order_by_asc(stores::Column::Name);

        if let Some(status_filter) = status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(stores::Column::Status.eq(status_filter.trim()));
        }
        if let Some(system_id) = system_id {
            query = query.filter(stores::Column::SystemId.eq(system_id));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let stores = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = stores.len(),
            total_count, page_number, page_size, "listed stores"
        );
        Ok((stores, total_count))
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn count_by_system_id(&self, system_id: Uuid) -> Result<u64, RepositoryError> {
        let count = stores::Entity::find()
            .filter(stores::Column::SystemId.eq(system_id))
            .count(&self.db)
            .await?;

        debug!(%system_id, count, "counted stores by system id");
        Ok(count)
    }

    #[tracing::instrument(level = "info", skip(self, store, changes), fields(store_id = %store.id))]
    pub async fn update_store(
        &self,
        store: &stores::Model,
        changes: StoreChanges,
        now: DateTime<Utc>,
    ) -> Result<stores::Model, RepositoryError> {
        let mut active: stores::ActiveModel = store.clone().into();

        if let Some(name) = changes.name {
            validate_required("name", &name)?;
            active.name = Set(name);
        }
        if let Some(system_id) = changes.system_id {
            active.system_id = Set(system_id);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        active.updated_at = Set(now);

        let store = active.update(&self.db).await?;
        info!(store_id = %store.id, "updated store");
        Ok(store)
    }

    #[tracing::instrument(level = "info", skip(self), fields(store_id = %store.id, status = %status))]
    pub async fn update_status(
        &self,
        store: &stores::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<stores::Model, RepositoryError> {
        validate_required("status", status)?;

        let mut active: stores::ActiveModel = store.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let store = active.update(&self.db).await?;

        info!(store_id = %store.id, status = %store.status, "updated store status");
        Ok(store)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_by_id(&self, store_id: Uuid) -> Result<bool, RepositoryError> {
        let result = stores::Entity::delete_by_id(store_id)
            .exec(&self.db)
            .await?;
        let deleted = result.rows_affected > 0;

        info!(%store_id, deleted, "deleted store by id");
        Ok(deleted)
    }
}

fn validate_required(field: &'static str, value: &str) -> Result<(), RepositoryError> {
    if value.trim().is_empty() {
        return Err(RepositoryError::MissingRequiredField { field });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        repositories::systems::{NewSystem, SystemRepository},
    };
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_repositories() -> (SystemRepository, StoreRepository) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        (SystemRepository::new(db.clone()), StoreRepository::new(db))
    }

    async fn system(systems: &SystemRepository, name: &str) -> Uuid {
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        systems
            .create_system(
                NewSystem {
                    name: name.to_string(),
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("system should be created")
            .id
    }

    fn new_store(name: &str, system_id: Uuid, status: &str) -> NewStore {
        NewStore {
            name: name.to_string(),
            system_id,
            status: status.to_string(),
        }
    }

    #[tokio::test]
    async fn creates_finds_counts_and_lists_stores() {
        let (systems, stores) = test_repositories().await;
        let system_a = system(&systems, "system-a").await;
        let system_b = system(&systems, "system-b").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let active = stores
            .create_store(new_store("active store", system_a, "active"), now)
            .await
            .expect("active store should be created");
        stores
            .create_store(new_store("disabled store", system_b, "disabled"), now)
            .await
            .expect("disabled store should be created");

        let found = stores
            .find_by_id(active.id)
            .await
            .expect("store lookup should succeed")
            .expect("store should be found");
        assert_eq!(found.name, "active store");
        assert_eq!(found.system_id, system_a);

        let (disabled, total_count) = stores
            .list_stores(Some("disabled"), None, 1, 50)
            .await
            .expect("stores should list");
        assert_eq!(total_count, 1);
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].name, "disabled store");

        let (system_a_stores, total_count) = stores
            .list_stores(None, Some(system_a), 1, 50)
            .await
            .expect("stores should list by system");
        assert_eq!(total_count, 1);
        assert_eq!(system_a_stores[0].id, active.id);
        assert_eq!(
            stores
                .count_by_system_id(system_a)
                .await
                .expect("store count should work"),
            1
        );
    }

    #[tokio::test]
    async fn updates_and_disables_store() {
        let (systems, stores) = test_repositories().await;
        let system_a = system(&systems, "system-a").await;
        let system_b = system(&systems, "system-b").await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let store = stores
            .create_store(new_store("original", system_a, "active"), created_at)
            .await
            .expect("store should be created");

        let updated = stores
            .update_store(
                &store,
                StoreChanges {
                    name: Some("updated".to_string()),
                    system_id: Some(system_b),
                    ..StoreChanges::default()
                },
                updated_at,
            )
            .await
            .expect("store should update");

        assert_eq!(updated.name, "updated");
        assert_eq!(updated.system_id, system_b);
        assert_eq!(updated.updated_at, updated_at);

        let disabled_at = Utc.with_ymd_and_hms(2026, 7, 7, 2, 0, 0).unwrap();
        let disabled = stores
            .update_status(&updated, "disabled", disabled_at)
            .await
            .expect("store should be disabled");
        assert_eq!(disabled.status, "disabled");
        assert_eq!(disabled.updated_at, disabled_at);
    }

    #[tokio::test]
    async fn deletes_store_by_id() {
        let (systems, stores) = test_repositories().await;
        let system_id = system(&systems, "system-a").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let store = stores
            .create_store(new_store("delete me", system_id, "active"), now)
            .await
            .expect("store should be created");

        let deleted = stores
            .delete_by_id(store.id)
            .await
            .expect("store delete should succeed");

        assert!(deleted);
        assert!(
            stores
                .find_by_id(store.id)
                .await
                .expect("store lookup should succeed")
                .is_none()
        );
        assert!(
            !stores
                .delete_by_id(Uuid::new_v4())
                .await
                .expect("missing store delete should succeed")
        );
    }

    #[tokio::test]
    async fn rejects_required_fields_and_missing_system_fk() {
        let (systems, stores) = test_repositories().await;
        let system_id = system(&systems, "system-a").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        assert!(matches!(
            stores
                .create_store(new_store(" ", system_id, "active"), now)
                .await,
            Err(RepositoryError::MissingRequiredField { field: "name" })
        ));

        assert!(
            stores
                .create_store(new_store("missing system", Uuid::new_v4(), "active"), now)
                .await
                .is_err()
        );
    }
}

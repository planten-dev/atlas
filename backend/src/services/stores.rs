use chrono::Utc;
use serde_json::{Value, json};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::stores::{
        CreateStoreRequest, ListStoresQuery, ListStoresResponse, PatchField, StoreResponse,
        StoreStatus, StoreStatusParseError, UpdateStoreRequest,
    },
    entities::stores,
    repositories::{
        RepositoryError,
        stores::{NewStore, StoreChanges, StoreRepository},
        systems::SystemRepository,
    },
    services::audit::AuditService,
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_NAME_LENGTH: usize = 128;

#[derive(Clone)]
pub struct StoreService {
    stores: StoreRepository,
    systems: SystemRepository,
    audit: Option<AuditService>,
}

impl StoreService {
    pub fn new(stores: StoreRepository, systems: SystemRepository) -> Self {
        Self {
            stores,
            systems,
            audit: None,
        }
    }

    pub fn with_audit(
        stores: StoreRepository,
        systems: SystemRepository,
        audit: AuditService,
    ) -> Self {
        Self {
            stores,
            systems,
            audit: Some(audit),
        }
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_store(
        &self,
        request: CreateStoreRequest,
    ) -> Result<StoreResponse, StoreError> {
        self.create_store_as(None, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_store_as(
        &self,
        actor_user_id: Option<Uuid>,
        request: CreateStoreRequest,
    ) -> Result<StoreResponse, StoreError> {
        let status = match request.status {
            Some(status) => StoreStatus::parse("status", &status)?,
            None => StoreStatus::Active,
        };
        let name = required_text("name", request.name, MAX_NAME_LENGTH)?;
        let system_id = request.system_id;
        self.ensure_system_exists(system_id).await?;

        let store = NewStore {
            name,
            system_id,
            status: status.as_str().to_string(),
        };

        let now = Utc::now();
        let store = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let store = self.stores.create_store_in(&tx, store, now).await?;
            audit
                .record_create(
                    &tx,
                    "stores",
                    store.id,
                    actor_user_id,
                    store_audit_value(&store),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            store
        } else {
            self.stores.create_store(store, now).await?
        };
        info!(store_id = %store.id, "created store through service");
        Ok(StoreResponse::from(store))
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_stores(
        &self,
        query: ListStoresQuery,
    ) -> Result<ListStoresResponse, StoreError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| StoreStatus::parse("status_filter", value))
            .transpose()?;
        let (stores, total_count) = self
            .stores
            .list_stores(
                status_filter.map(StoreStatus::as_str),
                query.system_id,
                page_number,
                page_size,
            )
            .await?;

        debug!(
            count = stores.len(),
            total_count, page_number, page_size, "listed stores through service"
        );
        Ok(ListStoresResponse {
            stores: stores.into_iter().map(StoreResponse::from).collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn store_detail(&self, store_id: Uuid) -> Result<StoreResponse, StoreError> {
        let store = self
            .stores
            .find_by_id(store_id)
            .await?
            .ok_or(StoreError::StoreNotFound)?;

        debug!(%store_id, "loaded store detail");
        Ok(StoreResponse::from(store))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_store(
        &self,
        store_id: Uuid,
        request: UpdateStoreRequest,
    ) -> Result<StoreResponse, StoreError> {
        self.update_store_as(None, store_id, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_store_as(
        &self,
        actor_user_id: Option<Uuid>,
        store_id: Uuid,
        request: UpdateStoreRequest,
    ) -> Result<StoreResponse, StoreError> {
        let store = self
            .stores
            .find_by_id(store_id)
            .await?
            .ok_or(StoreError::StoreNotFound)?;
        let system_id = required_uuid_change("system_id", request.system_id)?;
        if let Some(system_id) = system_id {
            self.ensure_system_exists(system_id).await?;
        }

        let changes = StoreChanges {
            name: required_text_change("name", request.name, MAX_NAME_LENGTH)?,
            system_id,
            status: status_change("status", request.status)?,
        };

        if changes.is_empty() {
            debug!(%store_id, "store update request had no changes");
            return Ok(StoreResponse::from(store));
        }

        let old_value = store_audit_value(&store);
        let now = Utc::now();
        let store = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let store = self
                .stores
                .update_store_in(&tx, &store, changes, now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "stores",
                    store.id,
                    actor_user_id,
                    old_value,
                    store_audit_value(&store),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            store
        } else {
            self.stores.update_store(&store, changes, now).await?
        };
        info!(%store_id, "updated store through service");
        Ok(StoreResponse::from(store))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_store(&self, store_id: Uuid) -> Result<StoreResponse, StoreError> {
        self.disable_store_as(None, store_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_store_as(
        &self,
        actor_user_id: Option<Uuid>,
        store_id: Uuid,
    ) -> Result<StoreResponse, StoreError> {
        let store = self
            .stores
            .find_by_id(store_id)
            .await?
            .ok_or(StoreError::StoreNotFound)?;
        let old_value = store_audit_value(&store);
        let now = Utc::now();
        let store = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let store = self
                .stores
                .update_status_in(&tx, &store, StoreStatus::Disabled.as_str(), now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "stores",
                    store.id,
                    actor_user_id,
                    old_value,
                    store_audit_value(&store),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            store
        } else {
            self.stores
                .update_status(&store, StoreStatus::Disabled.as_str(), now)
                .await?
        };

        info!(%store_id, "disabled store through service");
        Ok(StoreResponse::from(store))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_store(&self, store_id: Uuid) -> Result<(), StoreError> {
        self.delete_store_as(None, store_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_store_as(
        &self,
        actor_user_id: Option<Uuid>,
        store_id: Uuid,
    ) -> Result<(), StoreError> {
        let store = self
            .stores
            .find_by_id(store_id)
            .await?
            .ok_or(StoreError::StoreNotFound)?;
        let old_value = store_audit_value(&store);

        let deleted = if let Some(audit) = &self.audit {
            let now = Utc::now();
            let tx = audit.begin().await?;
            let deleted = self.stores.delete_by_id_in(&tx, store_id).await?;
            if deleted {
                audit
                    .record_delete(&tx, "stores", store_id, actor_user_id, old_value, now)
                    .await?;
            }
            tx.commit().await.map_err(RepositoryError::from)?;
            deleted
        } else {
            self.stores.delete_by_id(store_id).await?
        };
        if !deleted {
            warn!(%store_id, "store disappeared before delete completed");
            return Err(StoreError::StoreNotFound);
        }

        info!(%store_id, "deleted store through service");
        Ok(())
    }

    async fn ensure_system_exists(&self, system_id: Uuid) -> Result<(), StoreError> {
        if self.systems.find_by_id(system_id).await?.is_none() {
            return Err(StoreError::SystemNotFound);
        }

        Ok(())
    }
}

fn store_audit_value(store: &stores::Model) -> Value {
    json!({
        "id": store.id,
        "name": store.name,
        "system_id": store.system_id,
        "status": store.status,
        "created_at": store.created_at,
        "updated_at": store.updated_at,
    })
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("store was not found")]
    StoreNotFound,
    #[error("system was not found")]
    SystemNotFound,
    #[error("{field} is required")]
    MissingRequiredField { field: &'static str },
    #[error("{field} must be at most {maximum} characters")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("{field} must be one of: active, disabled")]
    InvalidStatus { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidPaginationMinimum { field: &'static str, minimum: u64 },
    #[error("{field} must be less than or equal to {maximum}")]
    InvalidPaginationMaximum { field: &'static str, maximum: u64 },
}

impl StoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::StoreNotFound => "store_not_found",
            Self::SystemNotFound => "system_not_found",
            Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidStatus { .. }
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
        }
    }
}

impl From<StoreStatusParseError> for StoreError {
    fn from(error: StoreStatusParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            "rejected invalid store status"
        );
        Self::InvalidStatus {
            field: error.field,
            value: error.value,
        }
    }
}

fn required_text(field: &'static str, value: String, maximum: usize) -> Result<String, StoreError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(StoreError::MissingRequiredField { field });
    }
    validate_length(field, value, maximum)?;
    Ok(value.to_string())
}

fn required_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<String>, StoreError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(StoreError::MissingRequiredField { field }),
        PatchField::Value(value) => required_text(field, value, maximum).map(Some),
    }
}

fn required_uuid_change(
    field: &'static str,
    value: PatchField<Uuid>,
) -> Result<Option<Uuid>, StoreError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(StoreError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn status_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<String>, StoreError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(StoreError::MissingRequiredField { field }),
        PatchField::Value(value) => StoreStatus::parse(field, &value)
            .map(StoreStatus::as_str)
            .map(str::to_string)
            .map(Some)
            .map_err(Into::into),
    }
}

fn validate_length(field: &'static str, value: &str, maximum: usize) -> Result<(), StoreError> {
    if value.chars().count() > maximum {
        return Err(StoreError::FieldTooLong { field, maximum });
    }
    Ok(())
}

fn validate_page_number(page_number: u64) -> Result<(), StoreError> {
    if page_number == 0 {
        return Err(StoreError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), StoreError> {
    if page_size == 0 {
        return Err(StoreError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(StoreError::InvalidPaginationMaximum {
            field: "page_size",
            maximum: MAX_PAGE_SIZE,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        repositories::{
            departments::DepartmentRepository,
            systems::{NewSystem, SystemRepository},
        },
    };
    use serde_json::json;
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_services() -> (DepartmentRepository, SystemRepository, StoreService) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db);
        let service = StoreService::new(stores, systems.clone());
        (departments, systems, service)
    }

    async fn system(
        departments: &DepartmentRepository,
        systems: &SystemRepository,
        name: &str,
    ) -> Uuid {
        let department = departments
            .insert_department(Uuid::new_v4(), "manual", name, name, None, Utc::now())
            .await
            .expect("department should be created");
        systems
            .create_system(
                NewSystem {
                    name: name.to_string(),
                    department_id: department.id,
                    status: "active".to_string(),
                },
                Utc::now(),
            )
            .await
            .expect("system should be created")
            .id
    }

    fn create_request(name: &str, system_id: Uuid) -> CreateStoreRequest {
        CreateStoreRequest {
            name: name.to_string(),
            system_id,
            status: None,
        }
    }

    #[tokio::test]
    async fn creates_lists_and_reads_store_detail() {
        let (departments, systems, service) = test_services().await;
        let system_id = system(&departments, &systems, "system-a").await;
        let created = service
            .create_store(create_request("store-a", system_id))
            .await
            .expect("store should be created");

        assert_eq!(created.name, "store-a");
        assert_eq!(created.system_id, system_id);
        assert_eq!(created.status, "active");

        let list = service
            .list_stores(ListStoresQuery {
                status_filter: Some("active".to_string()),
                system_id: Some(system_id),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("stores should list");
        assert_eq!(list.page_number, DEFAULT_PAGE_NUMBER);
        assert_eq!(list.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(list.total_count, 1);
        assert_eq!(list.stores[0].id, created.id);

        let detail = service
            .store_detail(created.id)
            .await
            .expect("store detail should load");
        assert_eq!(detail, created);
    }

    #[tokio::test]
    async fn updates_disables_and_deletes_store() {
        let (departments, systems, service) = test_services().await;
        let system_a = system(&departments, &systems, "system-a").await;
        let system_b = system(&departments, &systems, "system-b").await;
        let created = service
            .create_store(create_request("store-a", system_a))
            .await
            .expect("store should be created");

        let request: UpdateStoreRequest = serde_json::from_value(json!({
            "name": "store-b",
            "system_id": system_b,
            "status": "active"
        }))
        .expect("update request should deserialize");
        let updated = service
            .update_store(created.id, request)
            .await
            .expect("store should update");
        assert_eq!(updated.name, "store-b");
        assert_eq!(updated.system_id, system_b);

        let no_change = service
            .update_store(
                created.id,
                serde_json::from_value(json!({})).expect("empty request should deserialize"),
            )
            .await
            .expect("empty update should return current store");
        assert_eq!(no_change.id, created.id);

        let disabled = service
            .disable_store(created.id)
            .await
            .expect("store should disable");
        assert_eq!(disabled.status, "disabled");

        service
            .delete_store(created.id)
            .await
            .expect("store should delete");
        assert!(matches!(
            service.store_detail(created.id).await,
            Err(StoreError::StoreNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_store_inputs() {
        let (departments, systems, service) = test_services().await;
        let system_id = system(&departments, &systems, "system-a").await;

        assert!(matches!(
            service.create_store(create_request(" ", system_id)).await,
            Err(StoreError::MissingRequiredField { field: "name" })
        ));
        assert!(matches!(
            service
                .create_store(create_request(&"x".repeat(MAX_NAME_LENGTH + 1), system_id))
                .await,
            Err(StoreError::FieldTooLong { field: "name", .. })
        ));
        let mut invalid_status = create_request("store-a", system_id);
        invalid_status.status = Some("deleted".to_string());
        assert!(matches!(
            service.create_store(invalid_status).await,
            Err(StoreError::InvalidStatus {
                field: "status",
                ..
            })
        ));
        assert!(matches!(
            service
                .create_store(create_request("store-a", Uuid::new_v4()))
                .await,
            Err(StoreError::SystemNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_list_and_update_parameters() {
        let (departments, systems, service) = test_services().await;
        let system_id = system(&departments, &systems, "system-a").await;
        let created = service
            .create_store(create_request("store-a", system_id))
            .await
            .expect("store should be created");

        assert!(matches!(
            service
                .list_stores(ListStoresQuery {
                    status_filter: Some("deleted".to_string()),
                    system_id: None,
                    page_number: None,
                    page_size: None,
                })
                .await,
            Err(StoreError::InvalidStatus {
                field: "status_filter",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_stores(ListStoresQuery {
                    status_filter: None,
                    system_id: None,
                    page_number: Some(0),
                    page_size: None,
                })
                .await,
            Err(StoreError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_stores(ListStoresQuery {
                    status_filter: None,
                    system_id: None,
                    page_number: None,
                    page_size: Some(MAX_PAGE_SIZE + 1),
                })
                .await,
            Err(StoreError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));

        let null_name: UpdateStoreRequest = serde_json::from_value(json!({"name": null}))
            .expect("update request should deserialize");
        assert!(matches!(
            service.update_store(created.id, null_name).await,
            Err(StoreError::MissingRequiredField { field: "name" })
        ));
        let missing_system: UpdateStoreRequest =
            serde_json::from_value(json!({"system_id": Uuid::new_v4()}))
                .expect("update request should deserialize");
        assert!(matches!(
            service.update_store(created.id, missing_system).await,
            Err(StoreError::SystemNotFound)
        ));
    }
}

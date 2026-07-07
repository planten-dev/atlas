use chrono::Utc;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::systems::{
        CreateSystemRequest, ListSystemsQuery, ListSystemsResponse, PatchField, SystemResponse,
        SystemStatus, SystemStatusParseError, UpdateSystemRequest,
    },
    repositories::{
        RepositoryError,
        departments::DepartmentRepository,
        stores::StoreRepository,
        systems::{NewSystem, SystemChanges, SystemRepository},
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_NAME_LENGTH: usize = 128;

#[derive(Clone)]
pub struct SystemService {
    systems: SystemRepository,
    departments: DepartmentRepository,
    stores: StoreRepository,
}

impl SystemService {
    pub fn new(
        systems: SystemRepository,
        departments: DepartmentRepository,
        stores: StoreRepository,
    ) -> Self {
        Self {
            systems,
            departments,
            stores,
        }
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_system(
        &self,
        request: CreateSystemRequest,
    ) -> Result<SystemResponse, SystemError> {
        let status = match request.status {
            Some(status) => SystemStatus::parse("status", &status)?,
            None => SystemStatus::Active,
        };
        let name = required_text("name", request.name, MAX_NAME_LENGTH)?;
        let department_id = request.department_id;
        self.ensure_department_exists(department_id).await?;

        let system = NewSystem {
            name,
            department_id,
            status: status.as_str().to_string(),
        };

        let system = self.systems.create_system(system, Utc::now()).await?;
        info!(system_id = %system.id, "created system through service");
        Ok(SystemResponse::from(system))
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_systems(
        &self,
        query: ListSystemsQuery,
    ) -> Result<ListSystemsResponse, SystemError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| SystemStatus::parse("status_filter", value))
            .transpose()?;
        let (systems, total_count) = self
            .systems
            .list_systems(
                status_filter.map(SystemStatus::as_str),
                query.department_id,
                page_number,
                page_size,
            )
            .await?;

        debug!(
            count = systems.len(),
            total_count, page_number, page_size, "listed systems through service"
        );
        Ok(ListSystemsResponse {
            systems: systems.into_iter().map(SystemResponse::from).collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn system_detail(&self, system_id: Uuid) -> Result<SystemResponse, SystemError> {
        let system = self
            .systems
            .find_by_id(system_id)
            .await?
            .ok_or(SystemError::SystemNotFound)?;

        debug!(%system_id, "loaded system detail");
        Ok(SystemResponse::from(system))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_system(
        &self,
        system_id: Uuid,
        request: UpdateSystemRequest,
    ) -> Result<SystemResponse, SystemError> {
        let system = self
            .systems
            .find_by_id(system_id)
            .await?
            .ok_or(SystemError::SystemNotFound)?;
        let department_id = required_uuid_change("department_id", request.department_id)?;
        if let Some(department_id) = department_id {
            self.ensure_department_exists(department_id).await?;
        }

        let changes = SystemChanges {
            name: required_text_change("name", request.name, MAX_NAME_LENGTH)?,
            department_id,
            status: status_change("status", request.status)?,
        };

        if changes.is_empty() {
            debug!(%system_id, "system update request had no changes");
            return Ok(SystemResponse::from(system));
        }

        let system = self
            .systems
            .update_system(&system, changes, Utc::now())
            .await?;
        info!(%system_id, "updated system through service");
        Ok(SystemResponse::from(system))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_system(&self, system_id: Uuid) -> Result<SystemResponse, SystemError> {
        let system = self
            .systems
            .find_by_id(system_id)
            .await?
            .ok_or(SystemError::SystemNotFound)?;
        let system = self
            .systems
            .update_status(&system, SystemStatus::Disabled.as_str(), Utc::now())
            .await?;

        info!(%system_id, "disabled system through service");
        Ok(SystemResponse::from(system))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_system(&self, system_id: Uuid) -> Result<(), SystemError> {
        if self.systems.find_by_id(system_id).await?.is_none() {
            return Err(SystemError::SystemNotFound);
        }
        if self.stores.count_by_system_id(system_id).await? > 0 {
            warn!(%system_id, "rejected system delete because stores still reference it");
            return Err(SystemError::SystemHasStores);
        }

        let deleted = self.systems.delete_by_id(system_id).await?;
        if !deleted {
            warn!(%system_id, "system disappeared before delete completed");
            return Err(SystemError::SystemNotFound);
        }

        info!(%system_id, "deleted system through service");
        Ok(())
    }

    async fn ensure_department_exists(&self, department_id: Uuid) -> Result<(), SystemError> {
        if self.departments.find_by_id(department_id).await?.is_none() {
            return Err(SystemError::DepartmentNotFound);
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum SystemError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("system was not found")]
    SystemNotFound,
    #[error("department was not found")]
    DepartmentNotFound,
    #[error("system has stores and cannot be deleted")]
    SystemHasStores,
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

impl SystemError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::SystemNotFound => "system_not_found",
            Self::DepartmentNotFound => "department_not_found",
            Self::SystemHasStores => "system_has_stores",
            Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidStatus { .. }
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
        }
    }
}

impl From<SystemStatusParseError> for SystemError {
    fn from(error: SystemStatusParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            "rejected invalid system status"
        );
        Self::InvalidStatus {
            field: error.field,
            value: error.value,
        }
    }
}

fn required_text(
    field: &'static str,
    value: String,
    maximum: usize,
) -> Result<String, SystemError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SystemError::MissingRequiredField { field });
    }
    validate_length(field, value, maximum)?;
    Ok(value.to_string())
}

fn required_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<String>, SystemError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SystemError::MissingRequiredField { field }),
        PatchField::Value(value) => required_text(field, value, maximum).map(Some),
    }
}

fn required_uuid_change(
    field: &'static str,
    value: PatchField<Uuid>,
) -> Result<Option<Uuid>, SystemError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SystemError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn status_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<String>, SystemError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(SystemError::MissingRequiredField { field }),
        PatchField::Value(value) => SystemStatus::parse(field, &value)
            .map(SystemStatus::as_str)
            .map(str::to_string)
            .map(Some)
            .map_err(Into::into),
    }
}

fn validate_length(field: &'static str, value: &str, maximum: usize) -> Result<(), SystemError> {
    if value.chars().count() > maximum {
        return Err(SystemError::FieldTooLong { field, maximum });
    }
    Ok(())
}

fn validate_page_number(page_number: u64) -> Result<(), SystemError> {
    if page_number == 0 {
        return Err(SystemError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), SystemError> {
    if page_size == 0 {
        return Err(SystemError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(SystemError::InvalidPaginationMaximum {
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
        repositories::stores::{NewStore, StoreRepository},
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

    async fn test_services() -> (
        DepartmentRepository,
        SystemRepository,
        StoreRepository,
        SystemService,
    ) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db);
        let service = SystemService::new(systems.clone(), departments.clone(), stores.clone());
        (departments, systems, stores, service)
    }

    async fn department(repository: &DepartmentRepository, name: &str) -> Uuid {
        repository
            .insert_department(Uuid::new_v4(), "manual", name, name, None, Utc::now())
            .await
            .expect("department should be created")
            .id
    }

    fn create_request(name: &str, department_id: Uuid) -> CreateSystemRequest {
        CreateSystemRequest {
            name: name.to_string(),
            department_id,
            status: None,
        }
    }

    #[tokio::test]
    async fn creates_lists_and_reads_system_detail() {
        let (departments, _, _, service) = test_services().await;
        let department_id = department(&departments, "dept-a").await;
        let created = service
            .create_system(create_request("system-a", department_id))
            .await
            .expect("system should be created");

        assert_eq!(created.name, "system-a");
        assert_eq!(created.department_id, department_id);
        assert_eq!(created.status, "active");

        let list = service
            .list_systems(ListSystemsQuery {
                status_filter: Some("active".to_string()),
                department_id: Some(department_id),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("systems should list");
        assert_eq!(list.page_number, DEFAULT_PAGE_NUMBER);
        assert_eq!(list.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(list.total_count, 1);
        assert_eq!(list.systems[0].id, created.id);

        let detail = service
            .system_detail(created.id)
            .await
            .expect("system detail should load");
        assert_eq!(detail, created);
    }

    #[tokio::test]
    async fn updates_disables_and_deletes_system() {
        let (departments, _, _, service) = test_services().await;
        let department_a = department(&departments, "dept-a").await;
        let department_b = department(&departments, "dept-b").await;
        let created = service
            .create_system(create_request("system-a", department_a))
            .await
            .expect("system should be created");

        let request: UpdateSystemRequest = serde_json::from_value(json!({
            "name": "system-b",
            "department_id": department_b,
            "status": "active"
        }))
        .expect("update request should deserialize");
        let updated = service
            .update_system(created.id, request)
            .await
            .expect("system should update");
        assert_eq!(updated.name, "system-b");
        assert_eq!(updated.department_id, department_b);

        let no_change = service
            .update_system(
                created.id,
                serde_json::from_value(json!({})).expect("empty request should deserialize"),
            )
            .await
            .expect("empty update should return current system");
        assert_eq!(no_change.id, created.id);

        let disabled = service
            .disable_system(created.id)
            .await
            .expect("system should disable");
        assert_eq!(disabled.status, "disabled");

        service
            .delete_system(created.id)
            .await
            .expect("system should delete");
        assert!(matches!(
            service.system_detail(created.id).await,
            Err(SystemError::SystemNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_delete_when_system_has_stores() {
        let (departments, _, stores, service) = test_services().await;
        let department_id = department(&departments, "dept-a").await;
        let created = service
            .create_system(create_request("system-a", department_id))
            .await
            .expect("system should be created");
        stores
            .create_store(
                NewStore {
                    name: "store-a".to_string(),
                    system_id: created.id,
                    status: "active".to_string(),
                },
                Utc::now(),
            )
            .await
            .expect("store should be created");

        assert!(matches!(
            service.delete_system(created.id).await,
            Err(SystemError::SystemHasStores)
        ));
        assert!(
            service
                .system_detail(created.id)
                .await
                .expect("system should remain after rejected delete")
                .id
                == created.id
        );
    }

    #[tokio::test]
    async fn rejects_invalid_system_inputs() {
        let (departments, _, _, service) = test_services().await;
        let department_id = department(&departments, "dept-a").await;

        assert!(matches!(
            service
                .create_system(create_request(" ", department_id))
                .await,
            Err(SystemError::MissingRequiredField { field: "name" })
        ));
        assert!(matches!(
            service
                .create_system(create_request(
                    &"x".repeat(MAX_NAME_LENGTH + 1),
                    department_id
                ))
                .await,
            Err(SystemError::FieldTooLong { field: "name", .. })
        ));
        let mut invalid_status = create_request("system-a", department_id);
        invalid_status.status = Some("deleted".to_string());
        assert!(matches!(
            service.create_system(invalid_status).await,
            Err(SystemError::InvalidStatus {
                field: "status",
                ..
            })
        ));
        assert!(matches!(
            service
                .create_system(create_request("system-a", Uuid::new_v4()))
                .await,
            Err(SystemError::DepartmentNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_list_and_update_parameters() {
        let (departments, _, _, service) = test_services().await;
        let department_id = department(&departments, "dept-a").await;
        let created = service
            .create_system(create_request("system-a", department_id))
            .await
            .expect("system should be created");

        assert!(matches!(
            service
                .list_systems(ListSystemsQuery {
                    status_filter: Some("deleted".to_string()),
                    department_id: None,
                    page_number: None,
                    page_size: None,
                })
                .await,
            Err(SystemError::InvalidStatus {
                field: "status_filter",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_systems(ListSystemsQuery {
                    status_filter: None,
                    department_id: None,
                    page_number: Some(0),
                    page_size: None,
                })
                .await,
            Err(SystemError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_systems(ListSystemsQuery {
                    status_filter: None,
                    department_id: None,
                    page_number: None,
                    page_size: Some(MAX_PAGE_SIZE + 1),
                })
                .await,
            Err(SystemError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));

        let null_name: UpdateSystemRequest = serde_json::from_value(json!({"name": null}))
            .expect("update request should deserialize");
        assert!(matches!(
            service.update_system(created.id, null_name).await,
            Err(SystemError::MissingRequiredField { field: "name" })
        ));
        let missing_department: UpdateSystemRequest =
            serde_json::from_value(json!({"department_id": Uuid::new_v4()}))
                .expect("update request should deserialize");
        assert!(matches!(
            service.update_system(created.id, missing_department).await,
            Err(SystemError::DepartmentNotFound)
        ));
    }
}

use chrono::Utc;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::customers::{
        CreateCustomerRequest, CustomerAttachment, CustomerResponse, CustomerStatus,
        CustomerStatusParseError, ListCustomersQuery, ListCustomersResponse, PatchField,
        UpdateCustomerRequest,
    },
    repositories::{
        RepositoryError,
        customers::{CustomerChanges, CustomerFilters, CustomerRepository, NewCustomer},
        departments::DepartmentRepository,
        stores::StoreRepository,
        systems::SystemRepository,
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_NAME_LENGTH: usize = 128;
const MAX_FILE_ID_LENGTH: usize = 255;
const MAX_FILE_NAME_LENGTH: usize = 255;
const MAX_MIME_TYPE_LENGTH: usize = 128;

#[derive(Clone)]
pub struct CustomerService {
    customers: CustomerRepository,
    departments: DepartmentRepository,
    systems: SystemRepository,
    stores: StoreRepository,
}

impl CustomerService {
    pub fn new(
        customers: CustomerRepository,
        departments: DepartmentRepository,
        systems: SystemRepository,
        stores: StoreRepository,
    ) -> Self {
        Self {
            customers,
            departments,
            systems,
            stores,
        }
    }

    #[tracing::instrument(level = "info", skip(self, request), fields(actor_user_id = %actor_user_id))]
    pub async fn create_customer(
        &self,
        actor_user_id: Uuid,
        request: CreateCustomerRequest,
    ) -> Result<CustomerResponse, CustomerError> {
        let status = match request.status {
            Some(status) => CustomerStatus::parse("status", &status)?,
            None => CustomerStatus::Active,
        };
        let name = required_text("name", request.name, MAX_NAME_LENGTH)?;
        let remark = nullable_text(request.remark)?;
        let attachments = attachments_json("attachments", request.attachments)?;
        let (department_id, system_id, store_id) = self
            .resolve_customer_scope(request.store_id, request.department_id, request.system_id)
            .await?;

        let customer = NewCustomer {
            name,
            creator_user_id: actor_user_id,
            department_id,
            system_id,
            store_id,
            remark,
            status: status.as_str().to_string(),
            attachments,
        };

        let customer = self.customers.create_customer(customer, Utc::now()).await?;
        info!(customer_id = %customer.id, "created customer through service");
        Ok(CustomerResponse::from(customer))
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_customers(
        &self,
        query: ListCustomersQuery,
    ) -> Result<ListCustomersResponse, CustomerError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| CustomerStatus::parse("status_filter", value))
            .transpose()?;
        let name_keyword =
            nullable_limited_text("name_keyword", query.name_keyword, MAX_NAME_LENGTH)?;
        let (customers, total_count) = self
            .customers
            .list_customers(
                CustomerFilters {
                    status_filter: status_filter.map(CustomerStatus::as_str),
                    department_id: query.department_id,
                    system_id: query.system_id,
                    store_id: query.store_id,
                    creator_user_id: query.creator_user_id,
                    name_keyword: name_keyword.as_deref(),
                },
                page_number,
                page_size,
            )
            .await?;

        debug!(
            count = customers.len(),
            total_count, page_number, page_size, "listed customers through service"
        );
        Ok(ListCustomersResponse {
            customers: customers.into_iter().map(CustomerResponse::from).collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn customer_detail(
        &self,
        customer_id: Uuid,
    ) -> Result<CustomerResponse, CustomerError> {
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(CustomerError::CustomerNotFound)?;

        debug!(%customer_id, "loaded customer detail");
        Ok(CustomerResponse::from(customer))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_customer(
        &self,
        customer_id: Uuid,
        request: UpdateCustomerRequest,
    ) -> Result<CustomerResponse, CustomerError> {
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(CustomerError::CustomerNotFound)?;

        let department_id = required_uuid_change("department_id", request.department_id)?;
        let system_id = required_uuid_change("system_id", request.system_id)?;
        let store_id = required_uuid_change("store_id", request.store_id)?;
        if department_id.is_some() || system_id.is_some() || store_id.is_some() {
            self.ensure_customer_scope(
                department_id.unwrap_or(customer.department_id),
                system_id.unwrap_or(customer.system_id),
                store_id.unwrap_or(customer.store_id),
            )
            .await?;
        }

        let changes = CustomerChanges {
            name: required_text_change("name", request.name, MAX_NAME_LENGTH)?,
            department_id,
            system_id,
            store_id,
            remark: nullable_text_change("remark", request.remark)?,
            status: status_change("status", request.status)?,
            attachments: attachments_change("attachments", request.attachments)?,
        };

        if changes.is_empty() {
            debug!(%customer_id, "customer update request had no changes");
            return Ok(CustomerResponse::from(customer));
        }

        let customer = self
            .customers
            .update_customer(&customer, changes, Utc::now())
            .await?;
        info!(%customer_id, "updated customer through service");
        Ok(CustomerResponse::from(customer))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_customer(
        &self,
        customer_id: Uuid,
    ) -> Result<CustomerResponse, CustomerError> {
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(CustomerError::CustomerNotFound)?;
        let customer = self
            .customers
            .update_status(&customer, CustomerStatus::Disabled.as_str(), Utc::now())
            .await?;

        info!(%customer_id, "disabled customer through service");
        Ok(CustomerResponse::from(customer))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_customer(&self, customer_id: Uuid) -> Result<(), CustomerError> {
        if self.customers.find_by_id(customer_id).await?.is_none() {
            return Err(CustomerError::CustomerNotFound);
        }

        let deleted = self.customers.delete_by_id(customer_id).await?;
        if !deleted {
            warn!(%customer_id, "customer disappeared before delete completed");
            return Err(CustomerError::CustomerNotFound);
        }

        info!(%customer_id, "deleted customer through service");
        Ok(())
    }

    async fn resolve_customer_scope(
        &self,
        store_id: Uuid,
        requested_department_id: Option<Uuid>,
        requested_system_id: Option<Uuid>,
    ) -> Result<(Uuid, Uuid, Uuid), CustomerError> {
        let store = self
            .stores
            .find_by_id(store_id)
            .await?
            .ok_or(CustomerError::StoreNotFound)?;
        let system_id = store.system_id;

        if let Some(requested_system_id) = requested_system_id
            && requested_system_id != system_id
        {
            warn!(
                %store_id,
                %requested_system_id,
                store_system_id = %system_id,
                "rejected customer create because requested system does not match store"
            );
            return Err(CustomerError::StoreSystemMismatch);
        }

        let system = self
            .systems
            .find_by_id(system_id)
            .await?
            .ok_or(CustomerError::SystemNotFound)?;
        let department_id = system.department_id;

        if let Some(requested_department_id) = requested_department_id
            && requested_department_id != department_id
        {
            warn!(
                %requested_department_id,
                system_department_id = %department_id,
                %system_id,
                "rejected customer create because requested department does not match system"
            );
            return Err(CustomerError::CustomerScopeMismatch);
        }

        if self.departments.find_by_id(department_id).await?.is_none() {
            return Err(CustomerError::DepartmentNotFound);
        }

        Ok((department_id, system_id, store_id))
    }

    async fn ensure_customer_scope(
        &self,
        department_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
    ) -> Result<(), CustomerError> {
        if self.departments.find_by_id(department_id).await?.is_none() {
            return Err(CustomerError::DepartmentNotFound);
        }
        let system = self
            .systems
            .find_by_id(system_id)
            .await?
            .ok_or(CustomerError::SystemNotFound)?;
        let store = self
            .stores
            .find_by_id(store_id)
            .await?
            .ok_or(CustomerError::StoreNotFound)?;

        if system.department_id != department_id {
            warn!(
                %department_id,
                %system_id,
                system_department_id = %system.department_id,
                "rejected customer scope because system does not belong to department"
            );
            return Err(CustomerError::CustomerScopeMismatch);
        }
        if store.system_id != system_id {
            warn!(
                %store_id,
                %system_id,
                store_system_id = %store.system_id,
                "rejected customer scope because store does not belong to system"
            );
            return Err(CustomerError::StoreSystemMismatch);
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CustomerError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("customer was not found")]
    CustomerNotFound,
    #[error("department was not found")]
    DepartmentNotFound,
    #[error("system was not found")]
    SystemNotFound,
    #[error("store was not found")]
    StoreNotFound,
    #[error("system does not belong to the customer department")]
    CustomerScopeMismatch,
    #[error("store does not belong to the customer system")]
    StoreSystemMismatch,
    #[error("{field} is required")]
    MissingRequiredField { field: &'static str },
    #[error("{field} must be at most {maximum} characters")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("{field} must be one of: active, disabled")]
    InvalidStatus { field: &'static str, value: String },
    #[error("{field} must be an image mime type")]
    InvalidAttachmentMimeType { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidPaginationMinimum { field: &'static str, minimum: u64 },
    #[error("{field} must be less than or equal to {maximum}")]
    InvalidPaginationMaximum { field: &'static str, maximum: u64 },
}

impl CustomerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::CustomerNotFound => "customer_not_found",
            Self::DepartmentNotFound => "department_not_found",
            Self::SystemNotFound => "system_not_found",
            Self::StoreNotFound => "store_not_found",
            Self::CustomerScopeMismatch => "customer_scope_mismatch",
            Self::StoreSystemMismatch => "store_system_mismatch",
            Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidStatus { .. }
            | Self::InvalidAttachmentMimeType { .. }
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
        }
    }
}

impl From<CustomerStatusParseError> for CustomerError {
    fn from(error: CustomerStatusParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            "rejected invalid customer status"
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
) -> Result<String, CustomerError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CustomerError::MissingRequiredField { field });
    }
    validate_length(field, value, maximum)?;
    Ok(value.to_string())
}

fn nullable_text(value: Option<String>) -> Result<Option<String>, CustomerError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn nullable_limited_text(
    field: &'static str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, CustomerError> {
    let value = nullable_text(value)?;
    if let Some(value) = &value {
        validate_length(field, value, maximum)?;
    }
    Ok(value)
}

fn required_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<String>, CustomerError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(CustomerError::MissingRequiredField { field }),
        PatchField::Value(value) => required_text(field, value, maximum).map(Some),
    }
}

fn required_uuid_change(
    field: &'static str,
    value: PatchField<Uuid>,
) -> Result<Option<Uuid>, CustomerError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(CustomerError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn nullable_text_change(
    _field: &'static str,
    value: PatchField<String>,
) -> Result<Option<Option<String>>, CustomerError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Ok(Some(None)),
        PatchField::Value(value) => nullable_text(Some(value)).map(Some),
    }
}

fn status_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<String>, CustomerError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(CustomerError::MissingRequiredField { field }),
        PatchField::Value(value) => CustomerStatus::parse(field, &value)
            .map(CustomerStatus::as_str)
            .map(str::to_string)
            .map(Some)
            .map_err(Into::into),
    }
}

fn attachments_change(
    field: &'static str,
    value: PatchField<Vec<CustomerAttachment>>,
) -> Result<Option<Option<String>>, CustomerError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Ok(Some(None)),
        PatchField::Value(value) => attachments_json(field, Some(value)).map(Some),
    }
}

fn attachments_json(
    field: &'static str,
    value: Option<Vec<CustomerAttachment>>,
) -> Result<Option<String>, CustomerError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }

    let attachments = value
        .into_iter()
        .map(|attachment| normalize_attachment(field, attachment))
        .collect::<Result<Vec<_>, _>>()?;

    if attachments.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        serde_json::to_string(&attachments).expect("customer attachments should serialize"),
    ))
}

fn normalize_attachment(
    field: &'static str,
    attachment: CustomerAttachment,
) -> Result<CustomerAttachment, CustomerError> {
    let file_id = required_text(
        "attachments.file_id",
        attachment.file_id,
        MAX_FILE_ID_LENGTH,
    )?;
    let file_name = nullable_attachment_text(
        "attachments.file_name",
        attachment.file_name,
        MAX_FILE_NAME_LENGTH,
    )?;
    let mime_type = nullable_attachment_text(
        "attachments.mime_type",
        attachment.mime_type,
        MAX_MIME_TYPE_LENGTH,
    )?;
    if let Some(mime_type) = &mime_type
        && !mime_type.starts_with("image/")
    {
        return Err(CustomerError::InvalidAttachmentMimeType {
            field,
            value: mime_type.clone(),
        });
    }

    Ok(CustomerAttachment {
        file_id,
        file_name,
        mime_type,
        size_bytes: attachment.size_bytes,
    })
}

fn nullable_attachment_text(
    field: &'static str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, CustomerError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    validate_length(field, value, maximum)?;
    Ok(Some(value.to_string()))
}

fn validate_length(field: &'static str, value: &str, maximum: usize) -> Result<(), CustomerError> {
    if value.chars().count() > maximum {
        return Err(CustomerError::FieldTooLong { field, maximum });
    }
    Ok(())
}

fn validate_page_number(page_number: u64) -> Result<(), CustomerError> {
    if page_number == 0 {
        return Err(CustomerError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), CustomerError> {
    if page_size == 0 {
        return Err(CustomerError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(CustomerError::InvalidPaginationMaximum {
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
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
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

    async fn test_services() -> (
        UserRepository,
        DepartmentRepository,
        SystemRepository,
        StoreRepository,
        CustomerService,
    ) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let users = UserRepository::new(db.clone());
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db.clone());
        let customers = CustomerRepository::new(db);
        let service = CustomerService::new(
            customers,
            departments.clone(),
            systems.clone(),
            stores.clone(),
        );
        (users, departments, systems, stores, service)
    }

    async fn user(repository: &UserRepository, dingtalk_id: &str) -> Uuid {
        repository
            .find_or_create_for_login(dingtalk_id, Utc::now())
            .await
            .expect("user should be created")
            .id
    }

    async fn scope(
        departments: &DepartmentRepository,
        systems: &SystemRepository,
        stores: &StoreRepository,
        name: &str,
    ) -> (Uuid, Uuid, Uuid) {
        let department = departments
            .insert_department(Uuid::new_v4(), "manual", name, name, None, Utc::now())
            .await
            .expect("department should be created");
        let system = systems
            .create_system(
                NewSystem {
                    name: name.to_string(),
                    department_id: department.id,
                    status: "active".to_string(),
                },
                Utc::now(),
            )
            .await
            .expect("system should be created");
        let store = stores
            .create_store(
                NewStore {
                    name: name.to_string(),
                    system_id: system.id,
                    status: "active".to_string(),
                },
                Utc::now(),
            )
            .await
            .expect("store should be created");

        (department.id, system.id, store.id)
    }

    fn create_request(name: &str, store_id: Uuid) -> CreateCustomerRequest {
        CreateCustomerRequest {
            name: name.to_string(),
            department_id: None,
            system_id: None,
            store_id,
            remark: Some("  remark  ".to_string()),
            attachments: None,
            status: None,
        }
    }

    fn image_attachment() -> CustomerAttachment {
        CustomerAttachment {
            file_id: " img-1 ".to_string(),
            file_name: Some(" photo.png ".to_string()),
            mime_type: Some("image/png".to_string()),
            size_bytes: Some(1024),
        }
    }

    #[tokio::test]
    async fn creates_lists_and_reads_customer_detail() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (department_id, system_id, store_id) =
            scope(&departments, &systems, &stores, "scope-a").await;
        let created = service
            .create_customer(actor, create_request("Alice", store_id))
            .await
            .expect("customer should be created");

        assert_eq!(created.name, "Alice");
        assert_eq!(created.creator_user_id, actor);
        assert_eq!(created.department_id, department_id);
        assert_eq!(created.system_id, system_id);
        assert_eq!(created.store_id, store_id);
        assert_eq!(created.status, "active");
        assert_eq!(created.remark.as_deref(), Some("remark"));
        assert!(created.attachments.is_empty());

        let list = service
            .list_customers(ListCustomersQuery {
                status_filter: Some("active".to_string()),
                department_id: Some(department_id),
                system_id: Some(system_id),
                store_id: Some(store_id),
                creator_user_id: Some(actor),
                name_keyword: Some("lic".to_string()),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("customers should list");
        assert_eq!(list.page_number, DEFAULT_PAGE_NUMBER);
        assert_eq!(list.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(list.total_count, 1);
        assert_eq!(list.customers[0].id, created.id);

        let detail = service
            .customer_detail(created.id)
            .await
            .expect("customer detail should load");
        assert_eq!(detail, created);
    }

    #[tokio::test]
    async fn updates_clears_disables_and_deletes_customer() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (_, _, store_a) = scope(&departments, &systems, &stores, "scope-a").await;
        let (department_b, system_b, store_b) =
            scope(&departments, &systems, &stores, "scope-b").await;
        let created = service
            .create_customer(actor, create_request("Alice", store_a))
            .await
            .expect("customer should be created");

        let request: UpdateCustomerRequest = serde_json::from_value(json!({
            "name": "Alice Updated",
            "department_id": department_b,
            "system_id": system_b,
            "store_id": store_b,
            "remark": null,
            "attachments": null,
            "status": "active"
        }))
        .expect("update request should deserialize");
        let updated = service
            .update_customer(created.id, request)
            .await
            .expect("customer should update");

        assert_eq!(updated.name, "Alice Updated");
        assert_eq!(updated.creator_user_id, actor);
        assert_eq!(updated.department_id, department_b);
        assert_eq!(updated.system_id, system_b);
        assert_eq!(updated.store_id, store_b);
        assert_eq!(updated.remark, None);
        assert!(updated.attachments.is_empty());

        let disabled = service
            .disable_customer(created.id)
            .await
            .expect("customer should disable");
        assert_eq!(disabled.status, "disabled");

        service
            .delete_customer(created.id)
            .await
            .expect("customer should delete");
        assert!(matches!(
            service.customer_detail(created.id).await,
            Err(CustomerError::CustomerNotFound)
        ));
    }

    #[tokio::test]
    async fn creates_customer_with_optional_scope_fields_and_attachments() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (department_id, system_id, store_id) =
            scope(&departments, &systems, &stores, "scope-a").await;

        let mut empty_attachments = create_request("Alice Empty", store_id);
        empty_attachments.attachments = Some(vec![]);
        let created_empty = service
            .create_customer(actor, empty_attachments)
            .await
            .expect("customer with empty attachments should be created");
        assert_eq!(created_empty.department_id, department_id);
        assert_eq!(created_empty.system_id, system_id);
        assert!(created_empty.attachments.is_empty());

        let mut with_attachment = create_request("Alice Attached", store_id);
        with_attachment.department_id = Some(department_id);
        with_attachment.system_id = Some(system_id);
        with_attachment.attachments = Some(vec![image_attachment()]);
        let created_attached = service
            .create_customer(actor, with_attachment)
            .await
            .expect("customer with image attachment should be created");
        assert_eq!(created_attached.department_id, department_id);
        assert_eq!(created_attached.system_id, system_id);
        assert_eq!(created_attached.attachments[0].file_id, "img-1");
        assert_eq!(
            created_attached.attachments[0].file_name.as_deref(),
            Some("photo.png")
        );
    }

    #[tokio::test]
    async fn rejects_invalid_customer_inputs() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (_, _, store_id) = scope(&departments, &systems, &stores, "scope-a").await;

        assert!(matches!(
            service
                .create_customer(actor, create_request(" ", store_id))
                .await,
            Err(CustomerError::MissingRequiredField { field: "name" })
        ));
        let mut invalid_status = create_request("Alice", store_id);
        invalid_status.status = Some("deleted".to_string());
        assert!(matches!(
            service.create_customer(actor, invalid_status).await,
            Err(CustomerError::InvalidStatus {
                field: "status",
                ..
            })
        ));
        let mut invalid_attachment = create_request("Alice", store_id);
        invalid_attachment.attachments = Some(vec![CustomerAttachment {
            file_id: "file-1".to_string(),
            file_name: None,
            mime_type: Some("application/pdf".to_string()),
            size_bytes: None,
        }]);
        assert!(matches!(
            service.create_customer(actor, invalid_attachment).await,
            Err(CustomerError::InvalidAttachmentMimeType { .. })
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_scope_and_pagination() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (_, _, store_a) = scope(&departments, &systems, &stores, "scope-a").await;
        let (department_b, system_b, _) = scope(&departments, &systems, &stores, "scope-b").await;
        let created = service
            .create_customer(actor, create_request("Alice", store_a))
            .await
            .expect("customer should be created");

        let mut mismatched_department = create_request("Bob", store_a);
        mismatched_department.department_id = Some(department_b);
        assert!(matches!(
            service.create_customer(actor, mismatched_department).await,
            Err(CustomerError::CustomerScopeMismatch)
        ));

        let mut mismatched_system = create_request("Bob", store_a);
        mismatched_system.system_id = Some(system_b);
        assert!(matches!(
            service.create_customer(actor, mismatched_system).await,
            Err(CustomerError::StoreSystemMismatch)
        ));

        assert!(matches!(
            service
                .create_customer(actor, create_request("Bob", Uuid::new_v4()))
                .await,
            Err(CustomerError::StoreNotFound)
        ));
        assert!(matches!(
            service
                .update_customer(
                    created.id,
                    serde_json::from_value(json!({"system_id": system_b}))
                        .expect("update request should deserialize")
                )
                .await,
            Err(CustomerError::CustomerScopeMismatch) | Err(CustomerError::StoreSystemMismatch)
        ));
        assert!(matches!(
            service
                .list_customers(ListCustomersQuery {
                    page_number: Some(0),
                    ..ListCustomersQuery::default()
                })
                .await,
            Err(CustomerError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_customers(ListCustomersQuery {
                    page_size: Some(MAX_PAGE_SIZE + 1),
                    ..ListCustomersQuery::default()
                })
                .await,
            Err(CustomerError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));
    }
}

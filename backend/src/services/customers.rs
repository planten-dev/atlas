use chrono::Utc;
use serde_json::{Value, json};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::customers::{
        CreateCustomerRequest, CustomerAttachment, CustomerResponse, CustomerStatus,
        CustomerStatusParseError, ListCustomersQuery, ListCustomersResponse, PatchField,
        UpdateCustomerRequest,
    },
    entities::customers,
    repositories::{
        RepositoryError,
        customers::{CustomerChanges, CustomerFilters, CustomerRepository, NewCustomer},
        departments::DepartmentRepository,
        stores::StoreRepository,
        systems::SystemRepository,
    },
    services::audit::AuditService,
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
    audit: Option<AuditService>,
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
            audit: None,
        }
    }

    pub fn with_audit(
        customers: CustomerRepository,
        departments: DepartmentRepository,
        systems: SystemRepository,
        stores: StoreRepository,
        audit: AuditService,
    ) -> Self {
        Self {
            customers,
            departments,
            systems,
            stores,
            audit: Some(audit),
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
        self.ensure_customer_scope(request.department_id, request.system_id, request.store_id)
            .await?;

        let customer = NewCustomer {
            name,
            creator_user_id: actor_user_id,
            department_id: request.department_id,
            system_id: request.system_id,
            store_id: request.store_id,
            remark,
            status: status.as_str().to_string(),
            attachments,
        };

        let now = Utc::now();
        let customer = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let customer = self
                .customers
                .create_customer_in(&tx, customer, now)
                .await?;
            audit
                .record_create(
                    &tx,
                    "customers",
                    customer.id,
                    Some(actor_user_id),
                    customer_audit_value(&customer),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            customer
        } else {
            self.customers.create_customer(customer, now).await?
        };
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
        self.update_customer_as(None, customer_id, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_customer_as(
        &self,
        actor_user_id: Option<Uuid>,
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

        let old_value = customer_audit_value(&customer);
        let now = Utc::now();
        let customer = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let customer = self
                .customers
                .update_customer_in(&tx, &customer, changes, now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "customers",
                    customer.id,
                    actor_user_id,
                    old_value,
                    customer_audit_value(&customer),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            customer
        } else {
            self.customers
                .update_customer(&customer, changes, now)
                .await?
        };
        info!(%customer_id, "updated customer through service");
        Ok(CustomerResponse::from(customer))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_customer(
        &self,
        customer_id: Uuid,
    ) -> Result<CustomerResponse, CustomerError> {
        self.disable_customer_as(None, customer_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_customer_as(
        &self,
        actor_user_id: Option<Uuid>,
        customer_id: Uuid,
    ) -> Result<CustomerResponse, CustomerError> {
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(CustomerError::CustomerNotFound)?;
        let old_value = customer_audit_value(&customer);
        let now = Utc::now();
        let customer = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let customer = self
                .customers
                .update_status_in(&tx, &customer, CustomerStatus::Disabled.as_str(), now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "customers",
                    customer.id,
                    actor_user_id,
                    old_value,
                    customer_audit_value(&customer),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            customer
        } else {
            self.customers
                .update_status(&customer, CustomerStatus::Disabled.as_str(), now)
                .await?
        };

        info!(%customer_id, "disabled customer through service");
        Ok(CustomerResponse::from(customer))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_customer(&self, customer_id: Uuid) -> Result<(), CustomerError> {
        self.delete_customer_as(None, customer_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_customer_as(
        &self,
        actor_user_id: Option<Uuid>,
        customer_id: Uuid,
    ) -> Result<(), CustomerError> {
        let customer = self
            .customers
            .find_by_id(customer_id)
            .await?
            .ok_or(CustomerError::CustomerNotFound)?;
        let old_value = customer_audit_value(&customer);

        let deleted = if let Some(audit) = &self.audit {
            let now = Utc::now();
            let tx = audit.begin().await?;
            let deleted = self.customers.delete_by_id_in(&tx, customer_id).await?;
            if deleted {
                audit
                    .record_delete(&tx, "customers", customer_id, actor_user_id, old_value, now)
                    .await?;
            }
            tx.commit().await.map_err(RepositoryError::from)?;
            deleted
        } else {
            self.customers.delete_by_id(customer_id).await?
        };
        if !deleted {
            warn!(%customer_id, "customer disappeared before delete completed");
            return Err(CustomerError::CustomerNotFound);
        }

        info!(%customer_id, "deleted customer through service");
        Ok(())
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

fn customer_audit_value(customer: &customers::Model) -> Value {
    json!({
        "id": customer.id,
        "name": customer.name,
        "creator_user_id": customer.creator_user_id,
        "department_id": customer.department_id,
        "system_id": customer.system_id,
        "store_id": customer.store_id,
        "remark": text_summary(customer.remark.as_deref()),
        "status": customer.status,
        "attachments": attachments_summary(customer.attachments.as_deref()),
        "created_at": customer.created_at,
        "updated_at": customer.updated_at,
    })
}

fn text_summary(value: Option<&str>) -> Value {
    match value {
        Some(value) => json!({
            "present": true,
            "length": value.chars().count(),
        }),
        None => json!({
            "present": false,
            "length": 0,
        }),
    }
}

fn attachments_summary(value: Option<&str>) -> Value {
    let Some(value) = value else {
        return json!({
            "present": false,
            "count": 0,
        });
    };
    let Ok(attachments) = serde_json::from_str::<Vec<CustomerAttachment>>(value) else {
        return json!({
            "present": true,
            "parseable": false,
        });
    };

    let file_name_count = attachments
        .iter()
        .filter(|attachment| attachment.file_name.is_some())
        .count();
    let mime_type_count = attachments
        .iter()
        .filter(|attachment| attachment.mime_type.is_some())
        .count();
    let size_bytes_count = attachments
        .iter()
        .filter(|attachment| attachment.size_bytes.is_some())
        .count();

    json!({
        "present": !attachments.is_empty(),
        "parseable": true,
        "count": attachments.len(),
        "fields": {
            "file_id": attachments.len(),
            "file_name": file_name_count,
            "mime_type": mime_type_count,
            "size_bytes": size_bytes_count,
        },
    })
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
        entities::events,
        repositories::{
            events::EventRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        },
        services::audit::AuditService,
    };
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::{Value, json};
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

    async fn audited_services() -> (
        UserRepository,
        DepartmentRepository,
        SystemRepository,
        StoreRepository,
        CustomerService,
        EventRepository,
    ) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let users = UserRepository::new(db.clone());
        let departments = DepartmentRepository::new(db.clone());
        let systems = SystemRepository::new(db.clone());
        let stores = StoreRepository::new(db.clone());
        let customers = CustomerRepository::new(db.clone());
        let events = EventRepository::new(db.clone());
        let service = CustomerService::with_audit(
            customers,
            departments.clone(),
            systems.clone(),
            stores.clone(),
            AuditService::new(EventRepository::new(db)),
        );
        (users, departments, systems, stores, service, events)
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

    fn create_request(
        name: &str,
        department_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
    ) -> CreateCustomerRequest {
        CreateCustomerRequest {
            name: name.to_string(),
            department_id,
            system_id,
            store_id,
            remark: Some("  remark  ".to_string()),
            attachments: Some(vec![CustomerAttachment {
                file_id: " img-1 ".to_string(),
                file_name: Some(" photo.png ".to_string()),
                mime_type: Some("image/png".to_string()),
                size_bytes: Some(1024),
            }]),
            status: None,
        }
    }

    #[tokio::test]
    async fn creates_lists_and_reads_customer_detail() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (department_id, system_id, store_id) =
            scope(&departments, &systems, &stores, "scope-a").await;
        let created = service
            .create_customer(
                actor,
                create_request("Alice", department_id, system_id, store_id),
            )
            .await
            .expect("customer should be created");

        assert_eq!(created.name, "Alice");
        assert_eq!(created.creator_user_id, actor);
        assert_eq!(created.department_id, department_id);
        assert_eq!(created.system_id, system_id);
        assert_eq!(created.store_id, store_id);
        assert_eq!(created.status, "active");
        assert_eq!(created.remark.as_deref(), Some("remark"));
        assert_eq!(created.attachments[0].file_id, "img-1");

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
        let (department_a, system_a, store_a) =
            scope(&departments, &systems, &stores, "scope-a").await;
        let (department_b, system_b, store_b) =
            scope(&departments, &systems, &stores, "scope-b").await;
        let created = service
            .create_customer(
                actor,
                create_request("Alice", department_a, system_a, store_a),
            )
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
    async fn rejects_invalid_customer_inputs() {
        let (users, departments, systems, stores, service) = test_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (department_id, system_id, store_id) =
            scope(&departments, &systems, &stores, "scope-a").await;

        assert!(matches!(
            service
                .create_customer(
                    actor,
                    create_request(" ", department_id, system_id, store_id)
                )
                .await,
            Err(CustomerError::MissingRequiredField { field: "name" })
        ));
        let mut invalid_status = create_request("Alice", department_id, system_id, store_id);
        invalid_status.status = Some("deleted".to_string());
        assert!(matches!(
            service.create_customer(actor, invalid_status).await,
            Err(CustomerError::InvalidStatus {
                field: "status",
                ..
            })
        ));
        let mut invalid_attachment = create_request("Alice", department_id, system_id, store_id);
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
        let (department_a, system_a, store_a) =
            scope(&departments, &systems, &stores, "scope-a").await;
        let (department_b, system_b, _) = scope(&departments, &systems, &stores, "scope-b").await;
        let created = service
            .create_customer(
                actor,
                create_request("Alice", department_a, system_a, store_a),
            )
            .await
            .expect("customer should be created");

        assert!(matches!(
            service
                .create_customer(
                    actor,
                    create_request("Bob", Uuid::new_v4(), system_a, store_a)
                )
                .await,
            Err(CustomerError::DepartmentNotFound)
        ));
        assert!(matches!(
            service
                .create_customer(
                    actor,
                    create_request("Bob", department_a, Uuid::new_v4(), store_a)
                )
                .await,
            Err(CustomerError::SystemNotFound)
        ));
        assert!(matches!(
            service
                .create_customer(
                    actor,
                    create_request("Bob", department_a, system_a, Uuid::new_v4())
                )
                .await,
            Err(CustomerError::StoreNotFound)
        ));
        assert!(matches!(
            service
                .create_customer(
                    actor,
                    create_request("Bob", department_b, system_a, store_a)
                )
                .await,
            Err(CustomerError::CustomerScopeMismatch)
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

    #[tokio::test]
    async fn customer_audit_payload_summarizes_sensitive_fields() {
        let (users, departments, systems, stores, service, events_repo) = audited_services().await;
        let actor = user(&users, "ding-user-1").await;
        let (department_id, system_id, store_id) =
            scope(&departments, &systems, &stores, "scope-a").await;

        let mut request = create_request("Alice", department_id, system_id, store_id);
        request.remark = Some("secret customer note".to_string());
        request.attachments = Some(vec![CustomerAttachment {
            file_id: "sensitive-file-id".to_string(),
            file_name: Some("sensitive-photo.png".to_string()),
            mime_type: Some("image/png".to_string()),
            size_bytes: Some(2048),
        }]);
        let created = service
            .create_customer(actor, request)
            .await
            .expect("customer should be created");

        let row = events::Entity::find()
            .filter(events::Column::ResourceType.eq("customers"))
            .filter(events::Column::ResourceId.eq(created.id))
            .one(&events_repo.db)
            .await
            .expect("audit event should load")
            .expect("customer audit should exist");
        let payload = row.new_value.expect("new value should be stored");
        let serialized = serde_json::to_string(&payload).expect("payload should serialize");

        assert_eq!(
            payload.pointer("/remark/present").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload
                .pointer("/attachments/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert!(!serialized.contains("secret customer note"));
        assert!(!serialized.contains("sensitive-file-id"));
        assert!(!serialized.contains("sensitive-photo.png"));
    }
}

use chrono::Utc;
use serde_json::{Value, json};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::product_categories::{
        CreateProductCategoryRequest, ListProductCategoriesQuery, ListProductCategoriesResponse,
        PatchField, ProductCategoryResponse, ProductCategoryStatus,
        ProductCategoryStatusParseError, UpdateProductCategoryRequest,
    },
    entities::product_category,
    repositories::{
        RepositoryError,
        product_categories::{
            NewProductCategory, ProductCategoryChanges, ProductCategoryRepository,
        },
        products::ProductRepository,
    },
    services::audit::AuditService,
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_CATEGORY_NAME_LENGTH: usize = 128;

#[derive(Clone)]
pub struct ProductCategoryService {
    categories: ProductCategoryRepository,
    products: ProductRepository,
    audit: Option<AuditService>,
}

impl ProductCategoryService {
    pub fn new(categories: ProductCategoryRepository, products: ProductRepository) -> Self {
        Self {
            categories,
            products,
            audit: None,
        }
    }

    pub fn with_audit(
        categories: ProductCategoryRepository,
        products: ProductRepository,
        audit: AuditService,
    ) -> Self {
        Self {
            categories,
            products,
            audit: Some(audit),
        }
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_category(
        &self,
        request: CreateProductCategoryRequest,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        self.create_category_as(None, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_category_as(
        &self,
        actor_user_id: Option<Uuid>,
        request: CreateProductCategoryRequest,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        let status = match request.status {
            Some(status) => ProductCategoryStatus::parse("status", &status)?,
            None => ProductCategoryStatus::Active,
        };
        let category_name = required_text(
            "category_name",
            request.category_name,
            MAX_CATEGORY_NAME_LENGTH,
        )?;
        self.ensure_category_name_available(None, &category_name)
            .await?;

        let category = NewProductCategory {
            category_name,
            requires_operation_count: request.requires_operation_count,
            status: status.as_str().to_string(),
        };

        let now = Utc::now();
        let category = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let category = self
                .categories
                .create_category_in(&tx, category, now)
                .await?;
            audit
                .record_create(
                    &tx,
                    "product_categories",
                    category.id,
                    actor_user_id,
                    category_audit_value(&category),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            category
        } else {
            self.categories.create_category(category, now).await?
        };
        info!(category_id = %category.id, "created product category through service");
        Ok(ProductCategoryResponse::from(category))
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_categories(
        &self,
        query: ListProductCategoriesQuery,
    ) -> Result<ListProductCategoriesResponse, ProductCategoryError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| ProductCategoryStatus::parse("status_filter", value))
            .transpose()?;
        let (categories, total_count) = self
            .categories
            .list_categories(
                status_filter.map(ProductCategoryStatus::as_str),
                query.requires_operation_count_filter,
                page_number,
                page_size,
            )
            .await?;

        debug!(
            count = categories.len(),
            total_count, page_number, page_size, "listed product categories through service"
        );
        Ok(ListProductCategoriesResponse {
            categories: categories
                .into_iter()
                .map(ProductCategoryResponse::from)
                .collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn category_detail(
        &self,
        category_id: Uuid,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(ProductCategoryError::CategoryNotFound)?;

        debug!(%category_id, "loaded product category detail");
        Ok(ProductCategoryResponse::from(category))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_category(
        &self,
        category_id: Uuid,
        request: UpdateProductCategoryRequest,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        self.update_category_as(None, category_id, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_category_as(
        &self,
        actor_user_id: Option<Uuid>,
        category_id: Uuid,
        request: UpdateProductCategoryRequest,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(ProductCategoryError::CategoryNotFound)?;

        let category_name = required_text_change(
            "category_name",
            request.category_name,
            MAX_CATEGORY_NAME_LENGTH,
        )?;
        if let Some(category_name) = category_name.as_deref() {
            self.ensure_category_name_available(Some(category_id), category_name)
                .await?;
        }

        let changes = ProductCategoryChanges {
            category_name,
            requires_operation_count: required_bool_change(
                "requires_operation_count",
                request.requires_operation_count,
            )?,
            status: status_change("status", request.status)?,
        };

        if changes.is_empty() {
            debug!(%category_id, "product category update request had no changes");
            return Ok(ProductCategoryResponse::from(category));
        }

        let old_value = category_audit_value(&category);
        let now = Utc::now();
        let category = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let category = self
                .categories
                .update_category_in(&tx, &category, changes, now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "product_categories",
                    category.id,
                    actor_user_id,
                    old_value,
                    category_audit_value(&category),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            category
        } else {
            self.categories
                .update_category(&category, changes, now)
                .await?
        };
        info!(%category_id, "updated product category through service");
        Ok(ProductCategoryResponse::from(category))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_category(
        &self,
        category_id: Uuid,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        self.disable_category_as(None, category_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_category_as(
        &self,
        actor_user_id: Option<Uuid>,
        category_id: Uuid,
    ) -> Result<ProductCategoryResponse, ProductCategoryError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(ProductCategoryError::CategoryNotFound)?;
        let old_value = category_audit_value(&category);
        let now = Utc::now();
        let category = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let category = self
                .categories
                .update_status_in(
                    &tx,
                    &category,
                    ProductCategoryStatus::Disabled.as_str(),
                    now,
                )
                .await?;
            audit
                .record_update(
                    &tx,
                    "product_categories",
                    category.id,
                    actor_user_id,
                    old_value,
                    category_audit_value(&category),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            category
        } else {
            self.categories
                .update_status(&category, ProductCategoryStatus::Disabled.as_str(), now)
                .await?
        };

        info!(%category_id, "disabled product category through service");
        Ok(ProductCategoryResponse::from(category))
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_category(&self, category_id: Uuid) -> Result<(), ProductCategoryError> {
        self.delete_category_as(None, category_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_category_as(
        &self,
        actor_user_id: Option<Uuid>,
        category_id: Uuid,
    ) -> Result<(), ProductCategoryError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(ProductCategoryError::CategoryNotFound)?;
        if self.products.count_by_category_id(category_id).await? > 0 {
            warn!(%category_id, "rejected product category delete because products still reference it");
            return Err(ProductCategoryError::CategoryHasProducts);
        }

        let old_value = category_audit_value(&category);
        let deleted = if let Some(audit) = &self.audit {
            let now = Utc::now();
            let tx = audit.begin().await?;
            let deleted = self.categories.delete_by_id_in(&tx, category_id).await?;
            if deleted {
                audit
                    .record_delete(
                        &tx,
                        "product_categories",
                        category_id,
                        actor_user_id,
                        old_value,
                        now,
                    )
                    .await?;
            }
            tx.commit().await.map_err(RepositoryError::from)?;
            deleted
        } else {
            self.categories.delete_by_id(category_id).await?
        };
        if !deleted {
            warn!(%category_id, "product category disappeared before delete completed");
            return Err(ProductCategoryError::CategoryNotFound);
        }

        info!(%category_id, "deleted product category through service");
        Ok(())
    }

    async fn ensure_category_name_available(
        &self,
        current_category_id: Option<Uuid>,
        category_name: &str,
    ) -> Result<(), ProductCategoryError> {
        if let Some(existing) = self.categories.find_by_category_name(category_name).await? {
            if Some(existing.id) != current_category_id {
                warn!(
                    category_name,
                    existing_category_id = %existing.id,
                    "rejected duplicate product category name"
                );
                return Err(ProductCategoryError::CategoryNameAlreadyExists);
            }
        }

        Ok(())
    }
}

fn category_audit_value(category: &product_category::Model) -> Value {
    json!({
        "id": category.id,
        "category_name": category.category_name,
        "requires_operation_count": category.requires_operation_count,
        "status": category.status,
        "created_at": category.created_at,
        "updated_at": category.updated_at,
    })
}

#[derive(Debug, Error)]
pub enum ProductCategoryError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("product category was not found")]
    CategoryNotFound,
    #[error("product category name already exists")]
    CategoryNameAlreadyExists,
    #[error("product category has products and cannot be deleted")]
    CategoryHasProducts,
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

impl ProductCategoryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::CategoryNotFound => "product_category_not_found",
            Self::CategoryNameAlreadyExists => "product_category_name_exists",
            Self::CategoryHasProducts => "product_category_has_products",
            Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidStatus { .. }
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
        }
    }
}

impl From<ProductCategoryStatusParseError> for ProductCategoryError {
    fn from(error: ProductCategoryStatusParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            "rejected invalid product category status"
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
) -> Result<String, ProductCategoryError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ProductCategoryError::MissingRequiredField { field });
    }
    validate_length(field, value, maximum)?;
    Ok(value.to_string())
}

fn required_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<String>, ProductCategoryError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(ProductCategoryError::MissingRequiredField { field }),
        PatchField::Value(value) => required_text(field, value, maximum).map(Some),
    }
}

fn required_bool_change(
    field: &'static str,
    value: PatchField<bool>,
) -> Result<Option<bool>, ProductCategoryError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(ProductCategoryError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn status_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<String>, ProductCategoryError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(ProductCategoryError::MissingRequiredField { field }),
        PatchField::Value(value) => ProductCategoryStatus::parse(field, &value)
            .map(ProductCategoryStatus::as_str)
            .map(str::to_string)
            .map(Some)
            .map_err(Into::into),
    }
}

fn validate_length(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), ProductCategoryError> {
    if value.chars().count() > maximum {
        return Err(ProductCategoryError::FieldTooLong { field, maximum });
    }
    Ok(())
}

fn validate_page_number(page_number: u64) -> Result<(), ProductCategoryError> {
    if page_number == 0 {
        return Err(ProductCategoryError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), ProductCategoryError> {
    if page_size == 0 {
        return Err(ProductCategoryError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(ProductCategoryError::InvalidPaginationMaximum {
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
        repositories::products::NewProduct,
    };
    use sea_orm::entity::prelude::Decimal;
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
        ProductCategoryRepository,
        ProductRepository,
        ProductCategoryService,
    ) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db);
        let service = ProductCategoryService::new(categories.clone(), products.clone());
        (categories, products, service)
    }

    fn create_request(name: &str, requires_operation_count: bool) -> CreateProductCategoryRequest {
        CreateProductCategoryRequest {
            category_name: name.to_string(),
            requires_operation_count,
            status: None,
        }
    }

    #[tokio::test]
    async fn creates_lists_and_reads_category_detail() {
        let (_, _, service) = test_services().await;
        let created = service
            .create_category(create_request("custom", true))
            .await
            .expect("category should be created");

        assert_eq!(created.category_name, "custom");
        assert!(created.requires_operation_count);
        assert_eq!(created.status, "active");

        let list = service
            .list_categories(ListProductCategoriesQuery {
                status_filter: Some("active".to_string()),
                requires_operation_count_filter: Some(true),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("categories should list");
        assert_eq!(list.page_number, DEFAULT_PAGE_NUMBER);
        assert_eq!(list.page_size, DEFAULT_PAGE_SIZE);
        assert!(list.total_count >= 4);
        assert!(
            list.categories
                .iter()
                .any(|category| category.id == created.id)
        );

        let detail = service
            .category_detail(created.id)
            .await
            .expect("category detail should load");
        assert_eq!(detail, created);
    }

    #[tokio::test]
    async fn updates_disables_and_deletes_category() {
        let (_, _, service) = test_services().await;
        let created = service
            .create_category(create_request("custom", true))
            .await
            .expect("category should be created");

        let request: UpdateProductCategoryRequest = serde_json::from_value(json!({
            "category_name": "custom-updated",
            "requires_operation_count": false,
            "status": "active"
        }))
        .expect("update request should deserialize");
        let updated = service
            .update_category(created.id, request)
            .await
            .expect("category should update");
        assert_eq!(updated.category_name, "custom-updated");
        assert!(!updated.requires_operation_count);

        let disabled = service
            .disable_category(created.id)
            .await
            .expect("category should disable");
        assert_eq!(disabled.status, "disabled");

        service
            .delete_category(created.id)
            .await
            .expect("category should delete");
        assert!(matches!(
            service.category_detail(created.id).await,
            Err(ProductCategoryError::CategoryNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_category_inputs() {
        let (_, _, service) = test_services().await;

        assert!(matches!(
            service.create_category(create_request(" ", true)).await,
            Err(ProductCategoryError::MissingRequiredField {
                field: "category_name"
            })
        ));
        let mut invalid_status = create_request("custom", true);
        invalid_status.status = Some("deleted".to_string());
        assert!(matches!(
            service.create_category(invalid_status).await,
            Err(ProductCategoryError::InvalidStatus {
                field: "status",
                ..
            })
        ));
        service
            .create_category(create_request("duplicate", true))
            .await
            .expect("category should be created");
        assert!(matches!(
            service
                .create_category(create_request("duplicate", false))
                .await,
            Err(ProductCategoryError::CategoryNameAlreadyExists)
        ));

        assert!(matches!(
            service
                .list_categories(ListProductCategoriesQuery {
                    status_filter: Some("deleted".to_string()),
                    requires_operation_count_filter: None,
                    page_number: None,
                    page_size: None,
                })
                .await,
            Err(ProductCategoryError::InvalidStatus {
                field: "status_filter",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_categories(ListProductCategoriesQuery {
                    status_filter: None,
                    requires_operation_count_filter: None,
                    page_number: Some(0),
                    page_size: None,
                })
                .await,
            Err(ProductCategoryError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_categories(ListProductCategoriesQuery {
                    status_filter: None,
                    requires_operation_count_filter: None,
                    page_number: None,
                    page_size: Some(MAX_PAGE_SIZE + 1),
                })
                .await,
            Err(ProductCategoryError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));

        let category = service
            .create_category(create_request("nullable", true))
            .await
            .expect("category should be created");
        let null_name: UpdateProductCategoryRequest =
            serde_json::from_value(json!({"category_name": null}))
                .expect("update request should deserialize");
        assert!(matches!(
            service.update_category(category.id, null_name).await,
            Err(ProductCategoryError::MissingRequiredField {
                field: "category_name"
            })
        ));
    }

    #[tokio::test]
    async fn rejects_delete_when_category_has_products() {
        let (_, products, service) = test_services().await;
        let category = service
            .create_category(create_request("with-product", true))
            .await
            .expect("category should be created");
        products
            .create_product(
                NewProduct {
                    name: "product-a".to_string(),
                    category_id: category.id,
                    series: None,
                    brand_name: None,
                    specification: None,
                    unit: None,
                    unit_price: Decimal::new(1230, 2),
                    status: "active".to_string(),
                },
                Utc::now(),
            )
            .await
            .expect("product should be created");

        assert!(matches!(
            service.delete_category(category.id).await,
            Err(ProductCategoryError::CategoryHasProducts)
        ));
        assert_eq!(
            service
                .category_detail(category.id)
                .await
                .expect("category should remain after rejected delete")
                .id,
            category.id
        );
    }
}

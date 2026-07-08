use chrono::Utc;
use sea_orm::entity::prelude::Decimal;
use serde_json::{Value, json};
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::products::{
        CreateProductRequest, ListProductsQuery, ListProductsResponse, PatchField, ProductResponse,
        ProductStatus, ProductStatusParseError, UpdateProductRequest,
    },
    entities::{product_category, products},
    repositories::{
        RepositoryError,
        product_categories::ProductCategoryRepository,
        products::{NewProduct, ProductChanges, ProductRepository},
    },
    services::audit::AuditService,
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_NAME_LENGTH: usize = 128;
const MAX_SERIES_LENGTH: usize = 128;
const MAX_BRAND_NAME_LENGTH: usize = 128;
const MAX_SPECIFICATION_LENGTH: usize = 255;
const MAX_UNIT_LENGTH: usize = 32;

#[derive(Clone)]
pub struct ProductService {
    products: ProductRepository,
    categories: ProductCategoryRepository,
    audit: Option<AuditService>,
}

impl ProductService {
    pub fn new(products: ProductRepository, categories: ProductCategoryRepository) -> Self {
        Self {
            products,
            categories,
            audit: None,
        }
    }

    pub fn with_audit(
        products: ProductRepository,
        categories: ProductCategoryRepository,
        audit: AuditService,
    ) -> Self {
        Self {
            products,
            categories,
            audit: Some(audit),
        }
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_product(
        &self,
        request: CreateProductRequest,
    ) -> Result<ProductResponse, ProductError> {
        self.create_product_as(None, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_product_as(
        &self,
        actor_user_id: Option<Uuid>,
        request: CreateProductRequest,
    ) -> Result<ProductResponse, ProductError> {
        let status = match request.status {
            Some(status) => ProductStatus::parse("status", &status)?,
            None => ProductStatus::Active,
        };
        let category = self.ensure_active_category(request.category_id).await?;
        let product = NewProduct {
            name: required_text("name", request.name, MAX_NAME_LENGTH)?,
            category_id: category.id,
            series: nullable_text("series", request.series, MAX_SERIES_LENGTH)?,
            brand_name: nullable_text("brand_name", request.brand_name, MAX_BRAND_NAME_LENGTH)?,
            specification: nullable_text(
                "specification",
                request.specification,
                MAX_SPECIFICATION_LENGTH,
            )?,
            unit: nullable_text("unit", request.unit, MAX_UNIT_LENGTH)?,
            unit_price: parse_unit_price("unit_price", Some(request.unit_price))?,
            status: status.as_str().to_string(),
        };

        let now = Utc::now();
        let product = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let product = self.products.create_product_in(&tx, product, now).await?;
            audit
                .record_create(
                    &tx,
                    "products",
                    product.id,
                    actor_user_id,
                    product_audit_value(&product),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            product
        } else {
            self.products.create_product(product, now).await?
        };
        info!(product_id = %product.id, "created product through service");
        Ok(ProductResponse::from_model(product, category))
    }

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_products(
        &self,
        query: ListProductsQuery,
    ) -> Result<ListProductsResponse, ProductError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| ProductStatus::parse("status_filter", value))
            .transpose()?;
        let (products, total_count) = self
            .products
            .list_products(
                status_filter.map(ProductStatus::as_str),
                query.category_id,
                page_number,
                page_size,
            )
            .await?;
        let products = self.product_responses(products).await?;

        debug!(
            count = products.len(),
            total_count, page_number, page_size, "listed products through service"
        );
        Ok(ListProductsResponse {
            products,
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn product_detail(&self, product_id: Uuid) -> Result<ProductResponse, ProductError> {
        let product = self
            .products
            .find_by_id(product_id)
            .await?
            .ok_or(ProductError::ProductNotFound)?;
        let category = self
            .categories
            .find_by_id(product.category_id)
            .await?
            .ok_or(ProductError::ProductCategoryNotFound)?;

        debug!(%product_id, "loaded product detail");
        Ok(ProductResponse::from_model(product, category))
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_product(
        &self,
        product_id: Uuid,
        request: UpdateProductRequest,
    ) -> Result<ProductResponse, ProductError> {
        self.update_product_as(None, product_id, request).await
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn update_product_as(
        &self,
        actor_user_id: Option<Uuid>,
        product_id: Uuid,
        request: UpdateProductRequest,
    ) -> Result<ProductResponse, ProductError> {
        let product = self
            .products
            .find_by_id(product_id)
            .await?
            .ok_or(ProductError::ProductNotFound)?;
        let category_id = required_uuid_change("category_id", request.category_id)?;
        if let Some(category_id) = category_id {
            self.ensure_active_category(category_id).await?;
        }
        let changes = ProductChanges {
            name: required_text_change("name", request.name, MAX_NAME_LENGTH)?,
            category_id,
            series: nullable_text_change("series", request.series, MAX_SERIES_LENGTH)?,
            brand_name: nullable_text_change(
                "brand_name",
                request.brand_name,
                MAX_BRAND_NAME_LENGTH,
            )?,
            specification: nullable_text_change(
                "specification",
                request.specification,
                MAX_SPECIFICATION_LENGTH,
            )?,
            unit: nullable_text_change("unit", request.unit, MAX_UNIT_LENGTH)?,
            unit_price: unit_price_change("unit_price", request.unit_price)?,
            status: status_change("status", request.status)?,
        };

        if changes.is_empty() {
            debug!(%product_id, "product update request had no changes");
            return self.product_response(product).await;
        }

        let old_value = product_audit_value(&product);
        let now = Utc::now();
        let product = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let product = self
                .products
                .update_product_in(&tx, &product, changes, now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "products",
                    product.id,
                    actor_user_id,
                    old_value,
                    product_audit_value(&product),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            product
        } else {
            self.products.update_product(&product, changes, now).await?
        };
        info!(%product_id, "updated product through service");
        self.product_response(product).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_product(&self, product_id: Uuid) -> Result<ProductResponse, ProductError> {
        self.disable_product_as(None, product_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_product_as(
        &self,
        actor_user_id: Option<Uuid>,
        product_id: Uuid,
    ) -> Result<ProductResponse, ProductError> {
        let product = self
            .products
            .find_by_id(product_id)
            .await?
            .ok_or(ProductError::ProductNotFound)?;
        let old_value = product_audit_value(&product);
        let now = Utc::now();
        let product = if let Some(audit) = &self.audit {
            let tx = audit.begin().await?;
            let product = self
                .products
                .update_status_in(&tx, &product, ProductStatus::Disabled.as_str(), now)
                .await?;
            audit
                .record_update(
                    &tx,
                    "products",
                    product.id,
                    actor_user_id,
                    old_value,
                    product_audit_value(&product),
                    now,
                )
                .await?;
            tx.commit().await.map_err(RepositoryError::from)?;
            product
        } else {
            self.products
                .update_status(&product, ProductStatus::Disabled.as_str(), now)
                .await?
        };

        info!(%product_id, "disabled product through service");
        self.product_response(product).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_product(&self, product_id: Uuid) -> Result<(), ProductError> {
        self.delete_product_as(None, product_id).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_product_as(
        &self,
        actor_user_id: Option<Uuid>,
        product_id: Uuid,
    ) -> Result<(), ProductError> {
        let product = self
            .products
            .find_by_id(product_id)
            .await?
            .ok_or(ProductError::ProductNotFound)?;
        let old_value = product_audit_value(&product);

        let deleted = if let Some(audit) = &self.audit {
            let now = Utc::now();
            let tx = audit.begin().await?;
            let deleted = self.products.delete_by_id_in(&tx, product_id).await?;
            if deleted {
                audit
                    .record_delete(&tx, "products", product_id, actor_user_id, old_value, now)
                    .await?;
            }
            tx.commit().await.map_err(RepositoryError::from)?;
            deleted
        } else {
            self.products.delete_by_id(product_id).await?
        };
        if !deleted {
            warn!(%product_id, "product disappeared before delete completed");
            return Err(ProductError::ProductNotFound);
        }

        info!(%product_id, "deleted product through service");
        Ok(())
    }

    async fn ensure_active_category(
        &self,
        category_id: Uuid,
    ) -> Result<product_category::Model, ProductError> {
        let category = self
            .categories
            .find_by_id(category_id)
            .await?
            .ok_or(ProductError::ProductCategoryNotFound)?;

        if category.status != ProductStatus::Active.as_str() {
            warn!(%category_id, "rejected product category because it is disabled");
            return Err(ProductError::ProductCategoryDisabled);
        }

        Ok(category)
    }

    async fn product_response(
        &self,
        product: crate::entities::products::Model,
    ) -> Result<ProductResponse, ProductError> {
        let category = self
            .categories
            .find_by_id(product.category_id)
            .await?
            .ok_or(ProductError::ProductCategoryNotFound)?;
        Ok(ProductResponse::from_model(product, category))
    }

    async fn product_responses(
        &self,
        products: Vec<crate::entities::products::Model>,
    ) -> Result<Vec<ProductResponse>, ProductError> {
        let category_ids = products.iter().map(|product| product.category_id).collect();
        let categories = self.categories.find_by_ids(category_ids).await?;
        let categories_by_id: HashMap<Uuid, product_category::Model> = categories
            .into_iter()
            .map(|category| (category.id, category))
            .collect();
        products
            .into_iter()
            .map(|product| {
                let category = categories_by_id
                    .get(&product.category_id)
                    .cloned()
                    .ok_or(ProductError::ProductCategoryNotFound)?;
                Ok(ProductResponse::from_model(product, category))
            })
            .collect()
    }
}

fn product_audit_value(product: &products::Model) -> Value {
    json!({
        "id": product.id,
        "name": product.name,
        "category_id": product.category_id,
        "series": product.series,
        "brand_name": product.brand_name,
        "specification": product.specification,
        "unit": product.unit,
        "unit_price": format!("{:.2}", product.unit_price),
        "status": product.status,
        "created_at": product.created_at,
        "updated_at": product.updated_at,
    })
}

#[derive(Debug, Error)]
pub enum ProductError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("product was not found")]
    ProductNotFound,
    #[error("product category was not found")]
    ProductCategoryNotFound,
    #[error("product category is disabled")]
    ProductCategoryDisabled,
    #[error("{field} is required")]
    MissingRequiredField { field: &'static str },
    #[error("{field} must be at most {maximum} characters")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("{field} must be one of: active, disabled")]
    InvalidStatus { field: &'static str, value: String },
    #[error("{field} must be a decimal string with at most two decimal places")]
    InvalidUnitPrice { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to 0.00")]
    NegativeUnitPrice { field: &'static str, value: String },
    #[error("{field} must be less than or equal to 9999999999.99")]
    UnitPriceTooLarge { field: &'static str, value: String },
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidPaginationMinimum { field: &'static str, minimum: u64 },
    #[error("{field} must be less than or equal to {maximum}")]
    InvalidPaginationMaximum { field: &'static str, maximum: u64 },
}

impl ProductError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::ProductNotFound => "product_not_found",
            Self::ProductCategoryNotFound => "product_category_not_found",
            Self::ProductCategoryDisabled => "product_category_disabled",
            Self::MissingRequiredField { .. }
            | Self::FieldTooLong { .. }
            | Self::InvalidStatus { .. }
            | Self::InvalidUnitPrice { .. }
            | Self::NegativeUnitPrice { .. }
            | Self::UnitPriceTooLarge { .. }
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. } => "validation_error",
        }
    }
}

impl From<ProductStatusParseError> for ProductError {
    fn from(error: ProductStatusParseError) -> Self {
        warn!(
            field = error.field,
            value = %error.value,
            "rejected invalid product status"
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
) -> Result<String, ProductError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ProductError::MissingRequiredField { field });
    }
    validate_length(field, value, maximum)?;
    Ok(value.to_string())
}

fn nullable_text(
    field: &'static str,
    value: Option<String>,
    maximum: usize,
) -> Result<Option<String>, ProductError> {
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

fn required_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<String>, ProductError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(ProductError::MissingRequiredField { field }),
        PatchField::Value(value) => required_text(field, value, maximum).map(Some),
    }
}

fn required_uuid_change(
    field: &'static str,
    value: PatchField<Uuid>,
) -> Result<Option<Uuid>, ProductError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(ProductError::MissingRequiredField { field }),
        PatchField::Value(value) => Ok(Some(value)),
    }
}

fn nullable_text_change(
    field: &'static str,
    value: PatchField<String>,
    maximum: usize,
) -> Result<Option<Option<String>>, ProductError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Ok(Some(None)),
        PatchField::Value(value) => nullable_text(field, Some(value), maximum).map(Some),
    }
}

fn status_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<String>, ProductError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => Err(ProductError::MissingRequiredField { field }),
        PatchField::Value(value) => ProductStatus::parse(field, &value)
            .map(ProductStatus::as_str)
            .map(str::to_string)
            .map(Some)
            .map_err(Into::into),
    }
}

fn unit_price_change(
    field: &'static str,
    value: PatchField<String>,
) -> Result<Option<Decimal>, ProductError> {
    match value {
        PatchField::Unset => Ok(None),
        PatchField::Null => parse_unit_price(field, None).map(Some),
        PatchField::Value(value) => parse_unit_price(field, Some(value)).map(Some),
    }
}

fn parse_unit_price(field: &'static str, value: Option<String>) -> Result<Decimal, ProductError> {
    let Some(value) = value else {
        return Err(ProductError::MissingRequiredField { field });
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ProductError::MissingRequiredField { field });
    }
    let decimal = Decimal::from_str(trimmed).map_err(|_| ProductError::InvalidUnitPrice {
        field,
        value: value.clone(),
    })?;
    if decimal.scale() > 2 {
        return Err(ProductError::InvalidUnitPrice { field, value });
    }
    if decimal < Decimal::ZERO {
        return Err(ProductError::NegativeUnitPrice { field, value });
    }
    if decimal > max_unit_price() {
        return Err(ProductError::UnitPriceTooLarge { field, value });
    }

    let mut normalized = decimal;
    normalized.rescale(2);
    Ok(normalized)
}

fn max_unit_price() -> Decimal {
    Decimal::new(999_999_999_999, 2)
}

fn validate_length(field: &'static str, value: &str, maximum: usize) -> Result<(), ProductError> {
    if value.chars().count() > maximum {
        return Err(ProductError::FieldTooLong { field, maximum });
    }
    Ok(())
}

fn validate_page_number(page_number: u64) -> Result<(), ProductError> {
    if page_number == 0 {
        return Err(ProductError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), ProductError> {
    if page_size == 0 {
        return Err(ProductError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(ProductError::InvalidPaginationMaximum {
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
        entities::events::{self, ApprovalStatus, EventType},
        repositories::events::EventRepository,
        services::audit::AuditService,
    };
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serde_json::json;
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn test_services() -> (ProductCategoryRepository, ProductRepository, ProductService) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db);
        let service = ProductService::new(products.clone(), categories.clone());
        (categories, products, service)
    }

    async fn audited_services() -> (ProductCategoryRepository, ProductService, EventRepository) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db.clone());
        let events = EventRepository::new(db.clone());
        let service = ProductService::with_audit(
            products,
            categories.clone(),
            AuditService::new(EventRepository::new(db)),
        );
        (categories, service, events)
    }

    async fn default_category(categories: &ProductCategoryRepository) -> Uuid {
        categories
            .find_by_category_name("产品")
            .await
            .expect("category lookup should succeed")
            .expect("default category should exist")
            .id
    }

    async fn medical_category(categories: &ProductCategoryRepository) -> Uuid {
        categories
            .find_by_category_name("医疗")
            .await
            .expect("category lookup should succeed")
            .expect("medical category should exist")
            .id
    }

    fn create_request(name: &str, category_id: Uuid, price: &str) -> CreateProductRequest {
        CreateProductRequest {
            name: name.to_string(),
            category_id,
            series: Some("series-a".to_string()),
            brand_name: Some("brand-a".to_string()),
            specification: Some("spec-a".to_string()),
            unit: None,
            unit_price: price.to_string(),
            status: None,
        }
    }

    #[tokio::test]
    async fn creates_lists_and_reads_product_detail() {
        let (categories, _, service) = test_services().await;
        let category_id = default_category(&categories).await;
        let created = service
            .create_product(create_request("product-a", category_id, "12.3"))
            .await
            .expect("product should be created");

        assert_eq!(created.name, "product-a");
        assert_eq!(created.category_id, category_id);
        assert_eq!(created.category_name, "产品");
        assert!(!created.requires_operation_count);
        assert_eq!(created.unit, None);
        assert_eq!(created.unit_price, "12.30");
        assert_eq!(created.status, "active");

        let list = service
            .list_products(ListProductsQuery {
                status_filter: Some("active".to_string()),
                category_id: Some(category_id),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("products should list");
        assert_eq!(list.page_number, DEFAULT_PAGE_NUMBER);
        assert_eq!(list.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(list.total_count, 1);
        assert_eq!(list.products[0].id, created.id);

        let detail = service
            .product_detail(created.id)
            .await
            .expect("product detail should load");
        assert_eq!(detail, created);
    }

    #[tokio::test]
    async fn updates_clears_disables_and_deletes_product() {
        let (categories, _, service) = test_services().await;
        let category_id = default_category(&categories).await;
        let medical_category_id = medical_category(&categories).await;
        let created = service
            .create_product(create_request("product-a", category_id, "12.30"))
            .await
            .expect("product should be created");

        let request: UpdateProductRequest = serde_json::from_value(json!({
            "name": "product-b",
            "category_id": medical_category_id,
            "brand_name": null,
            "unit": "piece",
            "unit_price": "25",
            "status": "active"
        }))
        .expect("update request should deserialize");
        let updated = service
            .update_product(created.id, request)
            .await
            .expect("product should update");
        assert_eq!(updated.name, "product-b");
        assert_eq!(updated.category_id, medical_category_id);
        assert_eq!(updated.category_name, "医疗");
        assert!(updated.requires_operation_count);
        assert_eq!(updated.brand_name, None);
        assert_eq!(updated.unit, Some("piece".to_string()));
        assert_eq!(updated.unit_price, "25.00");

        let disabled = service
            .disable_product(created.id)
            .await
            .expect("product should disable");
        assert_eq!(disabled.status, "disabled");

        service
            .delete_product(created.id)
            .await
            .expect("product should delete");
        assert!(matches!(
            service.product_detail(created.id).await,
            Err(ProductError::ProductNotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_product_inputs() {
        let (categories, _, service) = test_services().await;
        let category_id = default_category(&categories).await;

        assert!(matches!(
            service
                .create_product(create_request(" ", category_id, "12.30"))
                .await,
            Err(ProductError::MissingRequiredField { field: "name" })
        ));
        let mut invalid_status = create_request("product-a", category_id, "12.30");
        invalid_status.status = Some("deleted".to_string());
        assert!(matches!(
            service.create_product(invalid_status).await,
            Err(ProductError::InvalidStatus {
                field: "status",
                ..
            })
        ));
        assert!(matches!(
            service
                .create_product(create_request("product-a", category_id, "abc"))
                .await,
            Err(ProductError::InvalidUnitPrice {
                field: "unit_price",
                ..
            })
        ));
        assert!(matches!(
            service
                .create_product(create_request("product-a", category_id, "12.345"))
                .await,
            Err(ProductError::InvalidUnitPrice {
                field: "unit_price",
                ..
            })
        ));
        assert!(matches!(
            service
                .create_product(create_request("product-a", category_id, "-0.01"))
                .await,
            Err(ProductError::NegativeUnitPrice {
                field: "unit_price",
                ..
            })
        ));
        assert!(matches!(
            service
                .create_product(create_request("product-a", category_id, "10000000000.00"))
                .await,
            Err(ProductError::UnitPriceTooLarge {
                field: "unit_price",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_list_and_update_parameters() {
        let (categories, _, service) = test_services().await;
        let category_id = default_category(&categories).await;
        let created = service
            .create_product(create_request("product-a", category_id, "12.30"))
            .await
            .expect("product should be created");

        assert!(matches!(
            service
                .list_products(ListProductsQuery {
                    status_filter: Some("deleted".to_string()),
                    category_id: None,
                    page_number: None,
                    page_size: None,
                })
                .await,
            Err(ProductError::InvalidStatus {
                field: "status_filter",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_products(ListProductsQuery {
                    status_filter: None,
                    category_id: None,
                    page_number: Some(0),
                    page_size: None,
                })
                .await,
            Err(ProductError::InvalidPaginationMinimum {
                field: "page_number",
                ..
            })
        ));
        assert!(matches!(
            service
                .list_products(ListProductsQuery {
                    status_filter: None,
                    category_id: None,
                    page_number: None,
                    page_size: Some(MAX_PAGE_SIZE + 1),
                })
                .await,
            Err(ProductError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));

        let null_name: UpdateProductRequest = serde_json::from_value(json!({"name": null}))
            .expect("update request should deserialize");
        assert!(matches!(
            service.update_product(created.id, null_name).await,
            Err(ProductError::MissingRequiredField { field: "name" })
        ));

        let null_category: UpdateProductRequest =
            serde_json::from_value(json!({"category_id": null}))
                .expect("update request should deserialize");
        assert!(matches!(
            service.update_product(created.id, null_category).await,
            Err(ProductError::MissingRequiredField {
                field: "category_id"
            })
        ));
    }

    #[tokio::test]
    async fn rejects_missing_or_disabled_category() {
        let (categories, _, service) = test_services().await;
        let category_id = default_category(&categories).await;
        let created = service
            .create_product(create_request("product-a", category_id, "12.30"))
            .await
            .expect("product should be created");

        assert!(matches!(
            service
                .create_product(create_request("product-b", Uuid::new_v4(), "12.30"))
                .await,
            Err(ProductError::ProductCategoryNotFound)
        ));

        let medical_category_id = medical_category(&categories).await;
        let medical_category = categories
            .find_by_id(medical_category_id)
            .await
            .expect("category lookup should succeed")
            .expect("medical category should exist");
        categories
            .update_status(&medical_category, "disabled", Utc::now())
            .await
            .expect("category should disable");

        assert!(matches!(
            service
                .update_product(
                    created.id,
                    serde_json::from_value(json!({"category_id": medical_category_id}))
                        .expect("update request should deserialize")
                )
                .await,
            Err(ProductError::ProductCategoryDisabled)
        ));
    }

    #[tokio::test]
    async fn audited_product_mutations_write_events_and_skip_empty_update() {
        let (categories, service, events_repo) = audited_services().await;
        let category_id = default_category(&categories).await;
        let actor = Uuid::new_v4();

        let created = service
            .create_product_as(
                Some(actor),
                create_request("product-a", category_id, "12.30"),
            )
            .await
            .expect("product should be created");
        service
            .update_product_as(
                Some(actor),
                created.id,
                serde_json::from_value(json!({"name": "product-b"}))
                    .expect("update request should deserialize"),
            )
            .await
            .expect("product should update");
        service
            .update_product_as(
                Some(actor),
                created.id,
                serde_json::from_value(json!({})).expect("empty update should deserialize"),
            )
            .await
            .expect("empty update should be accepted");
        service
            .disable_product_as(Some(actor), created.id)
            .await
            .expect("product should disable");

        let rows = events::Entity::find()
            .filter(events::Column::ResourceType.eq("products"))
            .filter(events::Column::ResourceId.eq(created.id))
            .all(&events_repo.db)
            .await
            .expect("audit events should load");

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].event_type, EventType::Create);
        assert_eq!(rows[1].event_type, EventType::Update);
        assert_eq!(rows[2].event_type, EventType::Update);
        assert!(rows.iter().all(|row| row.actor_user_id == Some(actor)));
        assert!(
            rows.iter()
                .all(|row| row.approval_status == ApprovalStatus::None)
        );
    }
}

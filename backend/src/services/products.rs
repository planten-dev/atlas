use chrono::Utc;
use sea_orm::entity::prelude::Decimal;
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::products::{
        CreateProductRequest, ListProductsQuery, ListProductsResponse, PatchField, ProductResponse,
        ProductStatus, ProductStatusParseError, ProductSuggestionsQuery, ProductSuggestionsResponse,
        UpdateProductRequest,
    },
    entities::product_category,
    repositories::{
        RepositoryError,
        product_categories::ProductCategoryRepository,
        products::{NewProduct, ProductChanges, ProductFilters, ProductRepository},
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;
const DEFAULT_SUGGESTIONS_LIMIT: u64 = 50;
const MAX_SUGGESTIONS_LIMIT: u64 = 200;

const MAX_NAME_LENGTH: usize = 128;
const MAX_KEYWORD_LENGTH: usize = 128;
const MAX_SERIES_LENGTH: usize = 128;
const MAX_BRAND_NAME_LENGTH: usize = 128;
const MAX_SPECIFICATION_LENGTH: usize = 255;
const MAX_UNIT_LENGTH: usize = 32;

#[derive(Clone)]
pub struct ProductService {
    products: ProductRepository,
    categories: ProductCategoryRepository,
}

impl ProductService {
    pub fn new(products: ProductRepository, categories: ProductCategoryRepository) -> Self {
        Self {
            products,
            categories,
        }
    }

    #[tracing::instrument(level = "info", skip(self, request))]
    pub async fn create_product(
        &self,
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

        let product = self.products.create_product(product, Utc::now()).await?;
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
        let keyword = normalize_keyword("keyword", query.keyword, MAX_KEYWORD_LENGTH)?;
        let (products, total_count) = self
            .products
            .list_products(
                ProductFilters {
                    status_filter: status_filter.map(ProductStatus::as_str),
                    category_id: query.category_id,
                    keyword: keyword.as_deref(),
                },
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

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn product_suggestions(
        &self,
        query: ProductSuggestionsQuery,
    ) -> Result<ProductSuggestionsResponse, ProductError> {
        let status_filter = query
            .status_filter
            .as_deref()
            .map(|value| ProductStatus::parse("status_filter", value))
            .transpose()?;
        let keyword = normalize_keyword("keyword", query.keyword, MAX_KEYWORD_LENGTH)?;
        let limit_per_field = query.limit_per_field.unwrap_or(DEFAULT_SUGGESTIONS_LIMIT);
        validate_suggestions_limit(limit_per_field)?;

        let suggestions = self
            .products
            .product_suggestions(
                ProductFilters {
                    status_filter: status_filter.map(ProductStatus::as_str),
                    category_id: query.category_id,
                    keyword: keyword.as_deref(),
                },
                limit_per_field,
            )
            .await?;

        debug!(
            series_count = suggestions.series.len(),
            brand_name_count = suggestions.brand_names.len(),
            unit_count = suggestions.units.len(),
            limit_per_field,
            "loaded product suggestions through service"
        );
        Ok(ProductSuggestionsResponse {
            series: suggestions.series,
            brand_names: suggestions.brand_names,
            units: suggestions.units,
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

        let product = self
            .products
            .update_product(&product, changes, Utc::now())
            .await?;
        info!(%product_id, "updated product through service");
        self.product_response(product).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn disable_product(&self, product_id: Uuid) -> Result<ProductResponse, ProductError> {
        let product = self
            .products
            .find_by_id(product_id)
            .await?
            .ok_or(ProductError::ProductNotFound)?;
        let product = self
            .products
            .update_status(&product, ProductStatus::Disabled.as_str(), Utc::now())
            .await?;

        info!(%product_id, "disabled product through service");
        self.product_response(product).await
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_product(&self, product_id: Uuid) -> Result<(), ProductError> {
        if self.products.find_by_id(product_id).await?.is_none() {
            return Err(ProductError::ProductNotFound);
        }
        let reference_count = self.products.count_business_references(product_id).await?;
        if reference_count > 0 {
            warn!(%product_id, reference_count, "rejected product delete because it has business references");
            return Err(ProductError::ProductHasReferences);
        }

        let deleted = self.products.delete_by_id(product_id).await?;
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

#[derive(Debug, Error)]
pub enum ProductError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("product was not found")]
    ProductNotFound,
    #[error("product has business references and cannot be deleted")]
    ProductHasReferences,
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
            Self::ProductHasReferences => "product_has_references",
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

fn normalize_keyword(
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

fn validate_suggestions_limit(limit: u64) -> Result<(), ProductError> {
    if limit == 0 {
        return Err(ProductError::InvalidPaginationMinimum {
            field: "limit_per_field",
            minimum: 1,
        });
    }

    if limit > MAX_SUGGESTIONS_LIMIT {
        return Err(ProductError::InvalidPaginationMaximum {
            field: "limit_per_field",
            maximum: MAX_SUGGESTIONS_LIMIT,
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

    async fn test_services() -> (ProductCategoryRepository, ProductRepository, ProductService) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let categories = ProductCategoryRepository::new(db.clone());
        let products = ProductRepository::new(db);
        let service = ProductService::new(products.clone(), categories.clone());
        (categories, products, service)
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

    async fn create_sales_line_reference(products: &ProductRepository, product_id: Uuid) {
        use crate::repositories::{
            customers::{CustomerRepository, NewCustomer},
            sales_records::{NewSalesRecord, NewSalesRecordLine, SalesRecordRepository},
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        };
        use chrono::TimeZone;

        let now = chrono::Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let users = UserRepository::new(products.db.clone());
        let systems = SystemRepository::new(products.db.clone());
        let stores = StoreRepository::new(products.db.clone());
        let customers = CustomerRepository::new(products.db.clone());
        let sales_records = SalesRecordRepository::new(products.db.clone());

        let user = users
            .find_or_create_for_login("product-reference-user", now)
            .await
            .expect("user should be created");
        let system = systems
            .create_system(
                NewSystem {
                    name: "reference system".to_string(),
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("system should be created");
        let store = stores
            .create_store(
                NewStore {
                    name: "reference store".to_string(),
                    system_id: system.id,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("store should be created");
        let customer = customers
            .create_customer(
                NewCustomer {
                    name: "reference customer".to_string(),
                    creator_user_id: user.id,
                    system_id: system.id,
                    store_id: store.id,
                    remark: None,
                    status: "active".to_string(),
                    attachments: None,
                },
                now,
            )
            .await
            .expect("customer should be created");
        let record = sales_records
            .insert_sales_record(
                &products.db,
                NewSalesRecord {
                    record_type: "sale".to_string(),
                    customer_id: customer.id,
                    record_date: now.date_naive(),
                    customer_type: Some("new".to_string()),
                    deal_type: Some("non_salon".to_string()),
                    system_id: system.id,
                    store_id: store.id,
                    handler_user_id: user.id,
                    expert_user_id: None,
                    consultant_user_id: None,
                    doctor_user_id: None,
                    remark: None,
                    status: "active".to_string(),
                    created_by_user_id: user.id,
                },
                now,
            )
            .await
            .expect("sales record should be inserted");
        sales_records
            .insert_sales_record_line(
                &products.db,
                NewSalesRecordLine {
                    sales_record_id: record.id,
                    product_id,
                    item_name: "referenced".to_string(),
                    receivable_amount: Decimal::new(10000, 2),
                    operation_total_count: None,
                    remark: None,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("sales line should be inserted");
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
                keyword: None,
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
                    keyword: None,
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
                    keyword: None,
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
                    keyword: None,
                    page_number: None,
                    page_size: Some(MAX_PAGE_SIZE + 1),
                })
                .await,
            Err(ProductError::InvalidPaginationMaximum {
                field: "page_size",
                ..
            })
        ));

        assert!(matches!(
            service
                .list_products(ListProductsQuery {
                    status_filter: None,
                    category_id: None,
                    keyword: Some("x".repeat(MAX_KEYWORD_LENGTH + 1)),
                    page_number: None,
                    page_size: None,
                })
                .await,
            Err(ProductError::FieldTooLong {
                field: "keyword",
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
    async fn searches_products_and_returns_input_suggestions() {
        let (categories, _, service) = test_services().await;
        let category_id = default_category(&categories).await;
        let mut serum = create_request("hydrating serum", category_id, "12.30");
        serum.series = Some("skin line".to_string());
        serum.brand_name = Some("atlas lab".to_string());
        serum.specification = Some("30ml bottle".to_string());
        serum.unit = Some("bottle".to_string());
        let serum = service
            .create_product(serum)
            .await
            .expect("serum product should be created");
        let mut cream = create_request("repair cream", category_id, "25.00");
        cream.series = Some("skin line".to_string());
        cream.brand_name = Some("atlas lab".to_string());
        cream.specification = Some("50g jar".to_string());
        cream.unit = Some("jar".to_string());
        service
            .create_product(cream)
            .await
            .expect("cream product should be created");

        let list = service
            .list_products(ListProductsQuery {
                status_filter: Some("active".to_string()),
                category_id: Some(category_id),
                keyword: Some("30ml".to_string()),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("products should list by keyword");
        assert_eq!(list.total_count, 1);
        assert_eq!(list.products[0].id, serum.id);

        let blank_keyword = service
            .list_products(ListProductsQuery {
                status_filter: Some("active".to_string()),
                category_id: Some(category_id),
                keyword: Some("   ".to_string()),
                page_number: None,
                page_size: None,
            })
            .await
            .expect("blank keyword should be ignored");
        assert_eq!(blank_keyword.total_count, 2);

        let suggestions = service
            .product_suggestions(ProductSuggestionsQuery {
                status_filter: Some("active".to_string()),
                category_id: Some(category_id),
                keyword: Some("atlas".to_string()),
                limit_per_field: Some(10),
            })
            .await
            .expect("suggestions should load");
        assert_eq!(suggestions.series, vec!["skin line".to_string()]);
        assert_eq!(suggestions.brand_names, vec!["atlas lab".to_string()]);
        assert_eq!(
            suggestions.units,
            vec!["bottle".to_string(), "jar".to_string()]
        );

        assert!(matches!(
            service
                .product_suggestions(ProductSuggestionsQuery {
                    limit_per_field: Some(MAX_SUGGESTIONS_LIMIT + 1),
                    ..ProductSuggestionsQuery::default()
                })
                .await,
            Err(ProductError::InvalidPaginationMaximum {
                field: "limit_per_field",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn rejects_delete_when_product_has_sales_line_references() {
        let (categories, products, service) = test_services().await;
        let category_id = default_category(&categories).await;
        let product = service
            .create_product(create_request("referenced", category_id, "12.30"))
            .await
            .expect("product should be created");

        create_sales_line_reference(&products, product.id).await;

        assert!(matches!(
            service.delete_product(product.id).await,
            Err(ProductError::ProductHasReferences)
        ));
    }
}

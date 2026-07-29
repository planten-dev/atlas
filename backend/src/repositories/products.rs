use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, Set, Statement,
    Value,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::products, repositories::RepositoryError};

#[derive(Clone)]
pub struct ProductRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewProduct {
    pub name: String,
    pub category_id: Uuid,
    pub series: Option<String>,
    pub brand_name: Option<String>,
    pub specification: Option<String>,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProductChanges {
    pub name: Option<String>,
    pub category_id: Option<Uuid>,
    pub series: Option<Option<String>>,
    pub brand_name: Option<Option<String>>,
    pub specification: Option<Option<String>>,
    pub unit: Option<Option<String>>,
    pub unit_price: Option<Decimal>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProductFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub category_id: Option<Uuid>,
    pub keyword: Option<&'a str>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductSuggestions {
    pub series: Vec<String>,
    pub brand_names: Vec<String>,
    pub units: Vec<String>,
}

impl ProductChanges {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.category_id.is_none()
            && self.series.is_none()
            && self.brand_name.is_none()
            && self.specification.is_none()
            && self.unit.is_none()
            && self.unit_price.is_none()
            && self.status.is_none()
    }
}

impl ProductRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "info", skip(self, product), fields(name = %product.name, status = %product.status))]
    pub async fn create_product(
        &self,
        product: NewProduct,
        now: DateTime<Utc>,
    ) -> Result<products::Model, RepositoryError> {
        validate_required("name", &product.name)?;
        validate_required("status", &product.status)?;

        let product = products::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(product.name),
            category_id: Set(product.category_id),
            series: Set(product.series),
            brand_name: Set(product.brand_name),
            specification: Set(product.specification),
            unit: Set(product.unit),
            unit_price: Set(product.unit_price),
            status: Set(product.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(product_id = %product.id, "created product");
        Ok(product)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        product_id: Uuid,
    ) -> Result<Option<products::Model>, RepositoryError> {
        let product = products::Entity::find_by_id(product_id)
            .one(&self.db)
            .await?;

        debug!(found = product.is_some(), %product_id, "looked up product by id");
        Ok(product)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_products(
        &self,
        filters: ProductFilters<'_>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<products::Model>, u64), RepositoryError> {
        let query = apply_product_filters(
            products::Entity::find()
                .order_by_asc(products::Column::CreatedAt)
                .order_by_asc(products::Column::Name),
            filters,
        )?;

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let products = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = products.len(),
            total_count, page_number, page_size, "listed products"
        );
        Ok((products, total_count))
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn product_suggestions(
        &self,
        filters: ProductFilters<'_>,
        limit_per_field: u64,
    ) -> Result<ProductSuggestions, RepositoryError> {
        let series = self
            .distinct_text_values(products::Column::Series, filters, limit_per_field)
            .await?;
        let brand_names = self
            .distinct_text_values(products::Column::BrandName, filters, limit_per_field)
            .await?;
        let units = self
            .distinct_text_values(products::Column::Unit, filters, limit_per_field)
            .await?;

        debug!(
            series_count = series.len(),
            brand_name_count = brand_names.len(),
            unit_count = units.len(),
            "loaded product suggestions"
        );
        Ok(ProductSuggestions {
            series,
            brand_names,
            units,
        })
    }

    #[tracing::instrument(level = "info", skip(self, product, changes), fields(product_id = %product.id))]
    pub async fn update_product(
        &self,
        product: &products::Model,
        changes: ProductChanges,
        now: DateTime<Utc>,
    ) -> Result<products::Model, RepositoryError> {
        let mut active: products::ActiveModel = product.clone().into();

        if let Some(name) = changes.name {
            validate_required("name", &name)?;
            active.name = Set(name);
        }
        if let Some(category_id) = changes.category_id {
            active.category_id = Set(category_id);
        }
        if let Some(series) = changes.series {
            active.series = Set(series);
        }
        if let Some(brand_name) = changes.brand_name {
            active.brand_name = Set(brand_name);
        }
        if let Some(specification) = changes.specification {
            active.specification = Set(specification);
        }
        if let Some(unit) = changes.unit {
            active.unit = Set(unit);
        }
        if let Some(unit_price) = changes.unit_price {
            active.unit_price = Set(unit_price);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        active.updated_at = Set(now);

        let product = active.update(&self.db).await?;
        info!(product_id = %product.id, "updated product");
        Ok(product)
    }

    #[tracing::instrument(level = "info", skip(self), fields(product_id = %product.id, status = %status))]
    pub async fn update_status(
        &self,
        product: &products::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<products::Model, RepositoryError> {
        validate_required("status", status)?;

        let mut active: products::ActiveModel = product.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let product = active.update(&self.db).await?;

        info!(product_id = %product.id, status = %product.status, "updated product status");
        Ok(product)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_by_id(&self, product_id: Uuid) -> Result<bool, RepositoryError> {
        let result = products::Entity::delete_by_id(product_id)
            .exec(&self.db)
            .await?;
        let deleted = result.rows_affected > 0;

        info!(%product_id, deleted, "deleted product by id");
        Ok(deleted)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn count_business_references(
        &self,
        product_id: Uuid,
    ) -> Result<u64, RepositoryError> {
        if !self.table_exists("sales_record_lines").await? {
            debug!(%product_id, "sales_record_lines table does not exist; product has no line references");
            return Ok(0);
        }

        let backend = self.db.get_database_backend();
        let statement = match backend {
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                backend,
                "SELECT COUNT(*) AS reference_count FROM sales_record_lines WHERE product_id = ?",
                vec![Value::Uuid(Some(Box::new(product_id)))],
            ),
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                backend,
                "SELECT COUNT(*) AS reference_count FROM sales_record_lines WHERE product_id = $1",
                vec![Value::Uuid(Some(Box::new(product_id)))],
            ),
            other => {
                return Err(RepositoryError::Database(sea_orm::DbErr::Custom(format!(
                    "unsupported database backend for product reference check: {other:?}"
                ))));
            }
        };
        let row = self.db.query_one(statement).await?.ok_or_else(|| {
            sea_orm::DbErr::Custom("reference count query returned no row".into())
        })?;
        let count: i64 = row.try_get("", "reference_count")?;
        let count = u64::try_from(count).map_err(|error| {
            sea_orm::DbErr::Custom(format!("reference count was negative: {error}"))
        })?;

        debug!(%product_id, count, "counted product business references");
        Ok(count)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn count_by_category_id(&self, category_id: Uuid) -> Result<u64, RepositoryError> {
        let count = products::Entity::find()
            .filter(products::Column::CategoryId.eq(category_id))
            .count(&self.db)
            .await?;

        debug!(%category_id, count, "counted products by category id");
        Ok(count)
    }

    async fn distinct_text_values(
        &self,
        column: products::Column,
        filters: ProductFilters<'_>,
        limit: u64,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = apply_product_filters(
            products::Entity::find()
                .select_only()
                .column(column)
                .filter(column.is_not_null())
                .filter(column.ne(""))
                .distinct()
                .order_by_asc(column)
                .limit(limit),
            filters,
        )?;
        let values: Vec<Option<String>> = query.into_tuple().all(&self.db).await?;

        Ok(values
            .into_iter()
            .flatten()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect())
    }

    async fn table_exists(&self, table_name: &str) -> Result<bool, RepositoryError> {
        validate_required("table_name", table_name)?;

        let backend = self.db.get_database_backend();
        let statement = match backend {
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                backend,
                "SELECT 1 AS exists_flag FROM sqlite_master WHERE type = 'table' AND name = ?",
                vec![Value::String(Some(Box::new(table_name.to_string())))],
            ),
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                backend,
                "SELECT 1 AS exists_flag FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1",
                vec![Value::String(Some(Box::new(table_name.to_string())))],
            ),
            other => {
                return Err(RepositoryError::Database(sea_orm::DbErr::Custom(format!(
                    "unsupported database backend for table existence check: {other:?}"
                ))));
            }
        };

        Ok(self.db.query_one(statement).await?.is_some())
    }
}

fn apply_product_filters(
    mut query: Select<products::Entity>,
    filters: ProductFilters<'_>,
) -> Result<Select<products::Entity>, RepositoryError> {
    if let Some(status_filter) = filters.status_filter {
        validate_required("status_filter", status_filter)?;
        query = query.filter(products::Column::Status.eq(status_filter.trim()));
    }
    if let Some(category_id) = filters.category_id {
        query = query.filter(products::Column::CategoryId.eq(category_id));
    }
    if let Some(keyword) = filters.keyword {
        validate_required("keyword", keyword)?;
        let keyword = keyword.trim();
        query = query.filter(
            Condition::any()
                .add(products::Column::Name.contains(keyword))
                .add(products::Column::Series.contains(keyword))
                .add(products::Column::BrandName.contains(keyword))
                .add(products::Column::Specification.contains(keyword))
                .add(products::Column::Unit.contains(keyword)),
        );
    }

    Ok(query)
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

    async fn test_repositories() -> (
        crate::repositories::product_categories::ProductCategoryRepository,
        ProductRepository,
    ) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        (
            crate::repositories::product_categories::ProductCategoryRepository::new(db.clone()),
            ProductRepository::new(db),
        )
    }

    async fn default_category(
        categories: &crate::repositories::product_categories::ProductCategoryRepository,
    ) -> Uuid {
        categories
            .find_by_category_name("产品")
            .await
            .expect("category lookup should succeed")
            .expect("default category should exist")
            .id
    }

    async fn create_sales_line_reference(repository: &ProductRepository, product_id: Uuid) {
        use crate::repositories::{
            customers::{CustomerRepository, NewCustomer},
            sales_records::{NewSalesRecord, NewSalesRecordLine, SalesRecordRepository},
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        };

        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let users = UserRepository::new(repository.db.clone());
        let systems = SystemRepository::new(repository.db.clone());
        let stores = StoreRepository::new(repository.db.clone());
        let customers = CustomerRepository::new(repository.db.clone());
        let sales_records = SalesRecordRepository::new(repository.db.clone());

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
                &repository.db,
                NewSalesRecord {
                    record_type: "deal".to_string(),
                    customer_id: customer.id,
                    record_date: now.date_naive(),
                    total_amount: Decimal::new(10000, 2),
                    received_amount: Decimal::new(10000, 2),
                    performance_status: Some("pending".to_string()),
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
                &repository.db,
                NewSalesRecordLine {
                    sales_record_id: record.id,
                    product_id,
                    item_name: "referenced".to_string(),
                    operation_total_count: None,
                    remark: None,
                    status: "active".to_string(),
                },
                now,
            )
            .await
            .expect("sales line should be inserted");
    }

    fn new_product(name: &str, category_id: Uuid, status: &str, cents: i64) -> NewProduct {
        NewProduct {
            name: name.to_string(),
            category_id,
            series: Some("series-a".to_string()),
            brand_name: Some("brand-a".to_string()),
            specification: Some("spec-a".to_string()),
            unit: Some("box".to_string()),
            unit_price: Decimal::new(cents, 2),
            status: status.to_string(),
        }
    }

    #[tokio::test]
    async fn creates_finds_and_lists_products() {
        let (categories, repository) = test_repositories().await;
        let category_id = default_category(&categories).await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let active = repository
            .create_product(
                new_product("active product", category_id, "active", 1230),
                now,
            )
            .await
            .expect("active product should be created");
        repository
            .create_product(
                new_product("disabled product", category_id, "disabled", 9900),
                now,
            )
            .await
            .expect("disabled product should be created");

        let found = repository
            .find_by_id(active.id)
            .await
            .expect("product lookup should succeed")
            .expect("product should be found");
        assert_eq!(found.name, "active product");
        assert_eq!(found.category_id, category_id);
        assert_eq!(found.unit_price, Decimal::new(1230, 2));

        let (disabled, total_count) = repository
            .list_products(
                ProductFilters {
                    status_filter: Some("disabled"),
                    category_id: Some(category_id),
                    keyword: None,
                },
                1,
                50,
            )
            .await
            .expect("products should list");
        assert_eq!(total_count, 1);
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].name, "disabled product");
        assert_eq!(
            repository
                .count_by_category_id(category_id)
                .await
                .expect("product count should succeed"),
            2
        );
        assert_eq!(
            repository
                .count_by_category_id(Uuid::new_v4())
                .await
                .expect("missing category count should succeed"),
            0
        );
    }

    #[tokio::test]
    async fn lists_products_by_category() {
        let (categories, repository) = test_repositories().await;
        let category_a = default_category(&categories).await;
        let category_b = categories
            .find_by_category_name("医疗")
            .await
            .expect("category lookup should succeed")
            .expect("medical category should exist")
            .id;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        repository
            .create_product(new_product("product-a", category_a, "active", 1230), now)
            .await
            .expect("product should be created");
        let product_b = repository
            .create_product(new_product("product-b", category_b, "active", 9900), now)
            .await
            .expect("product should be created");

        let (category_b_products, total_count) = repository
            .list_products(
                ProductFilters {
                    status_filter: Some("active"),
                    category_id: Some(category_b),
                    keyword: None,
                },
                1,
                50,
            )
            .await
            .expect("products should list");
        assert_eq!(total_count, 1);
        assert_eq!(category_b_products[0].id, product_b.id);
    }

    #[tokio::test]
    async fn lists_products_by_keyword_and_loads_suggestions() {
        let (categories, repository) = test_repositories().await;
        let category_id = default_category(&categories).await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let mut serum = new_product("hydrating serum", category_id, "active", 1230);
        serum.series = Some("skin line".to_string());
        serum.brand_name = Some("atlas lab".to_string());
        serum.specification = Some("30ml bottle".to_string());
        serum.unit = Some("bottle".to_string());
        let serum = repository
            .create_product(serum, now)
            .await
            .expect("serum product should be created");
        let mut cream = new_product("repair cream", category_id, "active", 9900);
        cream.series = Some("skin line".to_string());
        cream.brand_name = Some("atlas lab".to_string());
        cream.specification = Some("50g jar".to_string());
        cream.unit = Some("jar".to_string());
        repository
            .create_product(cream, now)
            .await
            .expect("cream product should be created");

        let (by_brand, total_count) = repository
            .list_products(
                ProductFilters {
                    status_filter: Some("active"),
                    category_id: Some(category_id),
                    keyword: Some("atlas"),
                },
                1,
                50,
            )
            .await
            .expect("products should list by keyword");
        assert_eq!(total_count, 2);
        assert_eq!(by_brand.len(), 2);

        let (by_spec, total_count) = repository
            .list_products(
                ProductFilters {
                    status_filter: Some("active"),
                    category_id: Some(category_id),
                    keyword: Some("30ml"),
                },
                1,
                50,
            )
            .await
            .expect("products should list by specification keyword");
        assert_eq!(total_count, 1);
        assert_eq!(by_spec[0].id, serum.id);

        let suggestions = repository
            .product_suggestions(
                ProductFilters {
                    status_filter: Some("active"),
                    category_id: Some(category_id),
                    keyword: Some("atlas"),
                },
                10,
            )
            .await
            .expect("product suggestions should load");
        assert_eq!(suggestions.series, vec!["skin line".to_string()]);
        assert_eq!(suggestions.brand_names, vec!["atlas lab".to_string()]);
        assert_eq!(
            suggestions.units,
            vec!["bottle".to_string(), "jar".to_string()]
        );
    }

    #[tokio::test]
    async fn updates_and_disables_product() {
        let (categories, repository) = test_repositories().await;
        let category_id = default_category(&categories).await;
        let medical_category_id = categories
            .find_by_category_name("医疗")
            .await
            .expect("category lookup should succeed")
            .expect("medical category should exist")
            .id;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let product = repository
            .create_product(
                new_product("original", category_id, "active", 1000),
                created_at,
            )
            .await
            .expect("product should be created");

        let updated = repository
            .update_product(
                &product,
                ProductChanges {
                    name: Some("updated".to_string()),
                    category_id: Some(medical_category_id),
                    unit: Some(Some("piece".to_string())),
                    unit_price: Some(Decimal::new(2500, 2)),
                    ..ProductChanges::default()
                },
                updated_at,
            )
            .await
            .expect("product should update");

        assert_eq!(updated.name, "updated");
        assert_eq!(updated.category_id, medical_category_id);
        assert_eq!(updated.unit, Some("piece".to_string()));
        assert_eq!(updated.unit_price, Decimal::new(2500, 2));
        assert_eq!(updated.updated_at, updated_at);

        let disabled_at = Utc.with_ymd_and_hms(2026, 7, 7, 2, 0, 0).unwrap();
        let disabled = repository
            .update_status(&updated, "disabled", disabled_at)
            .await
            .expect("product should be disabled");
        assert_eq!(disabled.status, "disabled");
        assert_eq!(disabled.updated_at, disabled_at);
    }

    #[tokio::test]
    async fn deletes_product_by_id() {
        let (categories, repository) = test_repositories().await;
        let category_id = default_category(&categories).await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let product = repository
            .create_product(new_product("delete me", category_id, "active", 1000), now)
            .await
            .expect("product should be created");

        let deleted = repository
            .delete_by_id(product.id)
            .await
            .expect("product delete should succeed");

        assert!(deleted);
        assert!(
            repository
                .find_by_id(product.id)
                .await
                .expect("product lookup should succeed")
                .is_none()
        );
        assert!(
            !repository
                .delete_by_id(Uuid::new_v4())
                .await
                .expect("missing product delete should succeed")
        );
    }

    #[tokio::test]
    async fn counts_sales_record_line_references() {
        let (categories, repository) = test_repositories().await;
        let category_id = default_category(&categories).await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let product = repository
            .create_product(new_product("referenced", category_id, "active", 1000), now)
            .await
            .expect("product should be created");

        assert_eq!(
            repository
                .count_business_references(product.id)
                .await
                .expect("reference count should succeed without future table"),
            0
        );

        create_sales_line_reference(&repository, product.id).await;

        assert_eq!(
            repository
                .count_business_references(product.id)
                .await
                .expect("reference count should see future table"),
            1
        );
    }
}

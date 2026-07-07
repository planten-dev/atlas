use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
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
    pub category: Option<String>,
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
    pub category: Option<Option<String>>,
    pub series: Option<Option<String>>,
    pub brand_name: Option<Option<String>>,
    pub specification: Option<Option<String>>,
    pub unit: Option<Option<String>>,
    pub unit_price: Option<Decimal>,
    pub status: Option<String>,
}

impl ProductChanges {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.category.is_none()
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
            category: Set(product.category),
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
        status_filter: Option<&str>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<products::Model>, u64), RepositoryError> {
        let mut query = products::Entity::find()
            .order_by_asc(products::Column::CreatedAt)
            .order_by_asc(products::Column::Name);

        if let Some(status_filter) = status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(products::Column::Status.eq(status_filter.trim()));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let products = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = products.len(),
            total_count, page_number, page_size, "listed products"
        );
        Ok((products, total_count))
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
        if let Some(category) = changes.category {
            active.category = Set(category);
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

    async fn test_repository() -> ProductRepository {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        ProductRepository::new(db)
    }

    fn new_product(name: &str, status: &str, cents: i64) -> NewProduct {
        NewProduct {
            name: name.to_string(),
            category: Some("category-a".to_string()),
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
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let active = repository
            .create_product(new_product("active product", "active", 1230), now)
            .await
            .expect("active product should be created");
        repository
            .create_product(new_product("disabled product", "disabled", 9900), now)
            .await
            .expect("disabled product should be created");

        let found = repository
            .find_by_id(active.id)
            .await
            .expect("product lookup should succeed")
            .expect("product should be found");
        assert_eq!(found.name, "active product");
        assert_eq!(found.unit_price, Decimal::new(1230, 2));

        let (disabled, total_count) = repository
            .list_products(Some("disabled"), 1, 50)
            .await
            .expect("products should list");
        assert_eq!(total_count, 1);
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].name, "disabled product");
    }

    #[tokio::test]
    async fn updates_and_disables_product() {
        let repository = test_repository().await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let product = repository
            .create_product(new_product("original", "active", 1000), created_at)
            .await
            .expect("product should be created");

        let updated = repository
            .update_product(
                &product,
                ProductChanges {
                    name: Some("updated".to_string()),
                    category: Some(None),
                    unit: Some(Some("piece".to_string())),
                    unit_price: Some(Decimal::new(2500, 2)),
                    ..ProductChanges::default()
                },
                updated_at,
            )
            .await
            .expect("product should update");

        assert_eq!(updated.name, "updated");
        assert_eq!(updated.category, None);
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
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let product = repository
            .create_product(new_product("delete me", "active", 1000), now)
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
}

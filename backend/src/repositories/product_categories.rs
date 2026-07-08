use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::product_category, repositories::RepositoryError};

#[derive(Clone)]
pub struct ProductCategoryRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewProductCategory {
    pub category_name: String,
    pub requires_operation_count: bool,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProductCategoryChanges {
    pub category_name: Option<String>,
    pub requires_operation_count: Option<bool>,
    pub status: Option<String>,
}

impl ProductCategoryChanges {
    pub fn is_empty(&self) -> bool {
        self.category_name.is_none()
            && self.requires_operation_count.is_none()
            && self.status.is_none()
    }
}

impl ProductCategoryRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "info", skip(self, category), fields(category_name = %category.category_name, status = %category.status))]
    pub async fn create_category(
        &self,
        category: NewProductCategory,
        now: DateTime<Utc>,
    ) -> Result<product_category::Model, RepositoryError> {
        self.create_category_in(&self.db, category, now).await
    }

    #[tracing::instrument(level = "info", skip(self, conn, category), fields(category_name = %category.category_name, status = %category.status))]
    pub async fn create_category_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        category: NewProductCategory,
        now: DateTime<Utc>,
    ) -> Result<product_category::Model, RepositoryError> {
        validate_required("category_name", &category.category_name)?;
        validate_required("status", &category.status)?;

        let category = product_category::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_name: Set(category.category_name),
            requires_operation_count: Set(category.requires_operation_count),
            status: Set(category.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;

        info!(category_id = %category.id, "created product category");
        Ok(category)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        category_id: Uuid,
    ) -> Result<Option<product_category::Model>, RepositoryError> {
        let category = product_category::Entity::find_by_id(category_id)
            .one(&self.db)
            .await?;

        debug!(found = category.is_some(), %category_id, "looked up product category by id");
        Ok(category)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_category_name(
        &self,
        category_name: &str,
    ) -> Result<Option<product_category::Model>, RepositoryError> {
        validate_required("category_name", category_name)?;
        let category = product_category::Entity::find()
            .filter(product_category::Column::CategoryName.eq(category_name.trim()))
            .one(&self.db)
            .await?;

        debug!(
            found = category.is_some(),
            category_name = %category_name.trim(),
            "looked up product category by name"
        );
        Ok(category)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_ids(
        &self,
        category_ids: Vec<Uuid>,
    ) -> Result<Vec<product_category::Model>, RepositoryError> {
        if category_ids.is_empty() {
            return Ok(Vec::new());
        }

        let categories = product_category::Entity::find()
            .filter(product_category::Column::Id.is_in(category_ids))
            .all(&self.db)
            .await?;

        debug!(
            count = categories.len(),
            "looked up product categories by ids"
        );
        Ok(categories)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_categories(
        &self,
        status_filter: Option<&str>,
        requires_operation_count_filter: Option<bool>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<product_category::Model>, u64), RepositoryError> {
        let mut query = product_category::Entity::find()
            .order_by_asc(product_category::Column::CreatedAt)
            .order_by_asc(product_category::Column::CategoryName);

        if let Some(status_filter) = status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(product_category::Column::Status.eq(status_filter.trim()));
        }
        if let Some(requires_operation_count_filter) = requires_operation_count_filter {
            query = query.filter(
                product_category::Column::RequiresOperationCount
                    .eq(requires_operation_count_filter),
            );
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let categories = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = categories.len(),
            total_count, page_number, page_size, "listed product categories"
        );
        Ok((categories, total_count))
    }

    #[tracing::instrument(level = "info", skip(self, category, changes), fields(category_id = %category.id))]
    pub async fn update_category(
        &self,
        category: &product_category::Model,
        changes: ProductCategoryChanges,
        now: DateTime<Utc>,
    ) -> Result<product_category::Model, RepositoryError> {
        self.update_category_in(&self.db, category, changes, now)
            .await
    }

    #[tracing::instrument(level = "info", skip(self, conn, category, changes), fields(category_id = %category.id))]
    pub async fn update_category_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        category: &product_category::Model,
        changes: ProductCategoryChanges,
        now: DateTime<Utc>,
    ) -> Result<product_category::Model, RepositoryError> {
        let mut active: product_category::ActiveModel = category.clone().into();

        if let Some(category_name) = changes.category_name {
            validate_required("category_name", &category_name)?;
            active.category_name = Set(category_name);
        }
        if let Some(requires_operation_count) = changes.requires_operation_count {
            active.requires_operation_count = Set(requires_operation_count);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        active.updated_at = Set(now);

        let category = active.update(conn).await?;
        info!(category_id = %category.id, "updated product category");
        Ok(category)
    }

    #[tracing::instrument(level = "info", skip(self), fields(category_id = %category.id, status = %status))]
    pub async fn update_status(
        &self,
        category: &product_category::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<product_category::Model, RepositoryError> {
        self.update_status_in(&self.db, category, status, now).await
    }

    #[tracing::instrument(level = "info", skip(self, conn), fields(category_id = %category.id, status = %status))]
    pub async fn update_status_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        category: &product_category::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<product_category::Model, RepositoryError> {
        validate_required("status", status)?;

        let mut active: product_category::ActiveModel = category.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let category = active.update(conn).await?;

        info!(category_id = %category.id, status = %category.status, "updated product category status");
        Ok(category)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_by_id(&self, category_id: Uuid) -> Result<bool, RepositoryError> {
        self.delete_by_id_in(&self.db, category_id).await
    }

    #[tracing::instrument(level = "info", skip(self, conn))]
    pub async fn delete_by_id_in<C: ConnectionTrait>(
        &self,
        conn: &C,
        category_id: Uuid,
    ) -> Result<bool, RepositoryError> {
        let result = product_category::Entity::delete_by_id(category_id)
            .exec(conn)
            .await?;
        let deleted = result.rows_affected > 0;

        info!(%category_id, deleted, "deleted product category by id");
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

    async fn test_repository() -> ProductCategoryRepository {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        ProductCategoryRepository::new(db)
    }

    fn new_category(name: &str, requires_operation_count: bool) -> NewProductCategory {
        NewProductCategory {
            category_name: name.to_string(),
            requires_operation_count,
            status: "active".to_string(),
        }
    }

    #[tokio::test]
    async fn seeds_finds_and_lists_categories() {
        let repository = test_repository().await;
        let product = repository
            .find_by_category_name("产品")
            .await
            .expect("category lookup should succeed")
            .expect("product category should be seeded");
        assert!(!product.requires_operation_count);

        let (operation_categories, total_count) = repository
            .list_categories(Some("active"), Some(true), 1, 50)
            .await
            .expect("categories should list");
        assert_eq!(total_count, 3);
        assert!(
            operation_categories.iter().all(|category| {
                category.requires_operation_count && category.status == "active"
            })
        );
    }

    #[tokio::test]
    async fn creates_updates_disables_and_deletes_category() {
        let repository = test_repository().await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let category = repository
            .create_category(new_category("custom", true), created_at)
            .await
            .expect("category should be created");

        let found = repository
            .find_by_id(category.id)
            .await
            .expect("category lookup should succeed")
            .expect("category should be found");
        assert_eq!(found.category_name, "custom");

        let updated = repository
            .update_category(
                &found,
                ProductCategoryChanges {
                    category_name: Some("updated".to_string()),
                    requires_operation_count: Some(false),
                    ..ProductCategoryChanges::default()
                },
                updated_at,
            )
            .await
            .expect("category should update");
        assert_eq!(updated.category_name, "updated");
        assert!(!updated.requires_operation_count);
        assert_eq!(updated.updated_at, updated_at);

        let disabled_at = Utc.with_ymd_and_hms(2026, 7, 7, 2, 0, 0).unwrap();
        let disabled = repository
            .update_status(&updated, "disabled", disabled_at)
            .await
            .expect("category should disable");
        assert_eq!(disabled.status, "disabled");
        assert_eq!(disabled.updated_at, disabled_at);

        assert!(
            repository
                .delete_by_id(disabled.id)
                .await
                .expect("category delete should succeed")
        );
        assert!(
            !repository
                .delete_by_id(Uuid::new_v4())
                .await
                .expect("missing category delete should succeed")
        );
    }

    #[tokio::test]
    async fn rejects_required_fields_and_duplicate_names() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        assert!(matches!(
            repository
                .create_category(new_category(" ", true), now)
                .await,
            Err(RepositoryError::MissingRequiredField {
                field: "category_name"
            })
        ));

        repository
            .create_category(new_category("unique", true), now)
            .await
            .expect("category should be created");
        assert!(
            repository
                .create_category(new_category("unique", false), now)
                .await
                .is_err()
        );
    }
}

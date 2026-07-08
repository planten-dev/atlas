use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::departments, repositories::RepositoryError};

#[derive(Clone)]
pub struct DepartmentRepository {
    pub(crate) db: DatabaseConnection,
}

impl DepartmentRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        department_id: Uuid,
    ) -> Result<Option<departments::Model>, RepositoryError> {
        let department = departments::Entity::find_by_id(department_id)
            .one(&self.db)
            .await?;

        debug!(
            found = department.is_some(),
            %department_id,
            "looked up department by id"
        );
        Ok(department)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_departments(
        &self,
        status_filter: Option<&str>,
        source_filter: Option<&str>,
        parent_id: Option<Uuid>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<departments::Model>, u64), RepositoryError> {
        let mut query = departments::Entity::find()
            .order_by_asc(departments::Column::CreatedAt)
            .order_by_asc(departments::Column::Name);

        if let Some(status_filter) = status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(departments::Column::Status.eq(status_filter.trim()));
        }
        if let Some(source_filter) = source_filter {
            validate_required("source_filter", source_filter)?;
            query = query.filter(departments::Column::Source.eq(source_filter.trim()));
        }
        if let Some(parent_id) = parent_id {
            query = query.filter(departments::Column::ParentId.eq(parent_id));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let departments = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = departments.len(),
            total_count, page_number, page_size, "listed departments"
        );
        Ok((departments, total_count))
    }

    #[tracing::instrument(level = "debug", skip(self), fields(source = %source))]
    pub async fn list_by_source(
        &self,
        source: &str,
    ) -> Result<Vec<departments::Model>, RepositoryError> {
        validate_required("source", source)?;

        let departments = departments::Entity::find()
            .filter(departments::Column::Source.eq(source.trim()))
            .order_by_asc(departments::Column::CreatedAt)
            .all(&self.db)
            .await?;

        debug!(count = departments.len(), "listed departments by source");
        Ok(departments)
    }

    #[tracing::instrument(
        level = "debug",
        skip(self),
        fields(source = %source, external_department_id = %external_department_id)
    )]
    pub async fn find_by_source_and_external_id(
        &self,
        source: &str,
        external_department_id: &str,
    ) -> Result<Option<departments::Model>, RepositoryError> {
        validate_required("source", source)?;
        validate_required("external_department_id", external_department_id)?;

        let department = departments::Entity::find()
            .filter(departments::Column::Source.eq(source.trim()))
            .filter(departments::Column::ExternalDepartmentId.eq(external_department_id.trim()))
            .one(&self.db)
            .await?;

        debug!(
            found = department.is_some(),
            "looked up department by source and external id"
        );
        Ok(department)
    }

    #[tracing::instrument(
        level = "info",
        skip(self),
        fields(source = %source, external_department_id = %external_department_id)
    )]
    pub async fn insert_department(
        &self,
        id: Uuid,
        source: &str,
        external_department_id: &str,
        name: &str,
        parent_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<departments::Model, RepositoryError> {
        validate_required("source", source)?;
        validate_required("external_department_id", external_department_id)?;
        validate_required("name", name)?;

        let department = departments::ActiveModel {
            id: Set(id),
            source: Set(source.trim().to_string()),
            external_department_id: Set(external_department_id.trim().to_string()),
            parent_id: Set(parent_id),
            name: Set(name.trim().to_string()),
            status: Set("active".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(department_id = %department.id, "created department");
        Ok(department)
    }

    #[tracing::instrument(level = "info", skip(self), fields(department_id = %id))]
    pub async fn update_name_and_parent(
        &self,
        id: Uuid,
        name: &str,
        parent_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<departments::Model, RepositoryError> {
        validate_required("name", name)?;

        let department = departments::ActiveModel {
            id: Set(id),
            name: Set(name.trim().to_string()),
            parent_id: Set(parent_id),
            updated_at: Set(now),
            ..Default::default()
        }
        .update(&self.db)
        .await?;

        info!(department_id = %department.id, "updated department name and parent");
        Ok(department)
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

    async fn test_repository() -> DepartmentRepository {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        DepartmentRepository::new(db)
    }

    #[tokio::test]
    async fn inserts_and_finds_department() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let parent_id = Uuid::new_v4();
        repository
            .insert_department(parent_id, "dingtalk", "1000", "总裁办", None, now)
            .await
            .expect("parent department should be created");

        let child_id = Uuid::new_v4();
        let child = repository
            .insert_department(child_id, "dingtalk", "1001", "秘书处", Some(parent_id), now)
            .await
            .expect("child department should be created");

        assert_eq!(child.id, child_id);
        assert_eq!(child.parent_id, Some(parent_id));
        assert_eq!(child.status, "active");

        let found = repository
            .find_by_source_and_external_id("dingtalk", "1001")
            .await
            .expect("department lookup should succeed")
            .expect("department should be found");
        assert_eq!(found.name, "秘书处");
        assert_eq!(found.parent_id, Some(parent_id));

        let listed = repository
            .list_by_source("dingtalk")
            .await
            .expect("departments should be listed");
        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn updates_name_and_parent_without_touching_status() {
        let repository = test_repository().await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let department = repository
            .insert_department(
                Uuid::new_v4(),
                "dingtalk",
                "1000",
                "总裁办",
                None,
                created_at,
            )
            .await
            .expect("department should be created");
        let new_parent = repository
            .insert_department(Uuid::new_v4(), "dingtalk", "2000", "集团", None, created_at)
            .await
            .expect("new parent should be created");
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();

        let updated = repository
            .update_name_and_parent(department.id, "总裁办公室", Some(new_parent.id), updated_at)
            .await
            .expect("department should be updated");

        assert_eq!(updated.id, department.id);
        assert_eq!(updated.name, "总裁办公室");
        assert_eq!(updated.parent_id, Some(new_parent.id));
        assert_eq!(updated.status, "active");
        assert_eq!(updated.created_at, created_at);
        assert_eq!(updated.updated_at, updated_at);
    }

    #[tokio::test]
    async fn rejects_duplicate_source_and_external_id() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        repository
            .insert_department(Uuid::new_v4(), "dingtalk", "1000", "总裁办", None, now)
            .await
            .expect("first department should be created");

        let result = repository
            .insert_department(Uuid::new_v4(), "dingtalk", "1000", "重复部门", None, now)
            .await;

        assert!(matches!(result, Err(RepositoryError::Database(_))));
    }

    #[tokio::test]
    async fn lists_departments_with_filters_and_pagination() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let parent = repository
            .insert_department(Uuid::new_v4(), "dingtalk", "10", "总裁办", None, now)
            .await
            .expect("parent should be created");
        let child = repository
            .insert_department(
                Uuid::new_v4(),
                "dingtalk",
                "11",
                "秘书处",
                Some(parent.id),
                now,
            )
            .await
            .expect("child should be created");
        repository
            .insert_department(Uuid::new_v4(), "manual", "m-1", "手动部门", None, now)
            .await
            .expect("manual department should be created");

        let (all, total_count) = repository
            .list_departments(None, None, None, 1, 50)
            .await
            .expect("departments should list");
        assert_eq!(total_count, 3);
        assert_eq!(all.len(), 3);

        let (dingtalk_only, total_count) = repository
            .list_departments(None, Some("dingtalk"), None, 1, 50)
            .await
            .expect("departments should list by source");
        assert_eq!(total_count, 2);
        assert!(dingtalk_only.iter().all(|d| d.source == "dingtalk"));

        let (children, total_count) = repository
            .list_departments(None, None, Some(parent.id), 1, 50)
            .await
            .expect("departments should list by parent");
        assert_eq!(total_count, 1);
        assert_eq!(children[0].id, child.id);

        let (active, total_count) = repository
            .list_departments(Some("active"), None, None, 1, 50)
            .await
            .expect("departments should list by status");
        assert_eq!(total_count, 3);
        assert_eq!(active.len(), 3);

        let (first_page, total_count) = repository
            .list_departments(None, None, None, 1, 2)
            .await
            .expect("first page should list");
        assert_eq!(total_count, 3);
        assert_eq!(first_page.len(), 2);
        let (second_page, _) = repository
            .list_departments(None, None, None, 2, 2)
            .await
            .expect("second page should list");
        assert_eq!(second_page.len(), 1);

        assert!(matches!(
            repository
                .list_departments(Some(" "), None, None, 1, 50)
                .await,
            Err(RepositoryError::MissingRequiredField {
                field: "status_filter"
            })
        ));
    }

    #[tokio::test]
    async fn allows_same_external_id_for_different_sources() {
        let repository = test_repository().await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        repository
            .insert_department(Uuid::new_v4(), "dingtalk", "1000", "总裁办", None, now)
            .await
            .expect("dingtalk department should be created");

        repository
            .insert_department(Uuid::new_v4(), "manual", "1000", "手动部门", None, now)
            .await
            .expect("manual department with same external id should be created");

        let listed = repository
            .list_by_source("manual")
            .await
            .expect("manual departments should be listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "手动部门");
    }
}

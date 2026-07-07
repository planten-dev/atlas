use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::systems, repositories::RepositoryError};

#[derive(Clone)]
pub struct SystemRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewSystem {
    pub name: String,
    pub department_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct SystemChanges {
    pub name: Option<String>,
    pub department_id: Option<Uuid>,
    pub status: Option<String>,
}

impl SystemChanges {
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.department_id.is_none() && self.status.is_none()
    }
}

impl SystemRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "info", skip(self, system), fields(name = %system.name, department_id = %system.department_id, status = %system.status))]
    pub async fn create_system(
        &self,
        system: NewSystem,
        now: DateTime<Utc>,
    ) -> Result<systems::Model, RepositoryError> {
        validate_required("name", &system.name)?;
        validate_required("status", &system.status)?;

        let system = systems::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(system.name),
            department_id: Set(system.department_id),
            status: Set(system.status),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(system_id = %system.id, "created system");
        Ok(system)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        system_id: Uuid,
    ) -> Result<Option<systems::Model>, RepositoryError> {
        let system = systems::Entity::find_by_id(system_id).one(&self.db).await?;

        debug!(found = system.is_some(), %system_id, "looked up system by id");
        Ok(system)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_systems(
        &self,
        status_filter: Option<&str>,
        department_id: Option<Uuid>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<systems::Model>, u64), RepositoryError> {
        let mut query = systems::Entity::find()
            .order_by_asc(systems::Column::CreatedAt)
            .order_by_asc(systems::Column::Name);

        if let Some(status_filter) = status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(systems::Column::Status.eq(status_filter.trim()));
        }
        if let Some(department_id) = department_id {
            query = query.filter(systems::Column::DepartmentId.eq(department_id));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let systems = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = systems.len(),
            total_count, page_number, page_size, "listed systems"
        );
        Ok((systems, total_count))
    }

    #[tracing::instrument(level = "info", skip(self, system, changes), fields(system_id = %system.id))]
    pub async fn update_system(
        &self,
        system: &systems::Model,
        changes: SystemChanges,
        now: DateTime<Utc>,
    ) -> Result<systems::Model, RepositoryError> {
        let mut active: systems::ActiveModel = system.clone().into();

        if let Some(name) = changes.name {
            validate_required("name", &name)?;
            active.name = Set(name);
        }
        if let Some(department_id) = changes.department_id {
            active.department_id = Set(department_id);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        active.updated_at = Set(now);

        let system = active.update(&self.db).await?;
        info!(system_id = %system.id, "updated system");
        Ok(system)
    }

    #[tracing::instrument(level = "info", skip(self), fields(system_id = %system.id, status = %status))]
    pub async fn update_status(
        &self,
        system: &systems::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<systems::Model, RepositoryError> {
        validate_required("status", status)?;

        let mut active: systems::ActiveModel = system.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let system = active.update(&self.db).await?;

        info!(system_id = %system.id, status = %system.status, "updated system status");
        Ok(system)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_by_id(&self, system_id: Uuid) -> Result<bool, RepositoryError> {
        let result = systems::Entity::delete_by_id(system_id)
            .exec(&self.db)
            .await?;
        let deleted = result.rows_affected > 0;

        info!(%system_id, deleted, "deleted system by id");
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
        repositories::departments::DepartmentRepository,
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

    async fn test_repositories() -> (DepartmentRepository, SystemRepository) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        (
            DepartmentRepository::new(db.clone()),
            SystemRepository::new(db),
        )
    }

    async fn department(repository: &DepartmentRepository, name: &str) -> Uuid {
        repository
            .insert_department(
                Uuid::new_v4(),
                "manual",
                name,
                name,
                None,
                Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap(),
            )
            .await
            .expect("department should be created")
            .id
    }

    fn new_system(name: &str, department_id: Uuid, status: &str) -> NewSystem {
        NewSystem {
            name: name.to_string(),
            department_id,
            status: status.to_string(),
        }
    }

    #[tokio::test]
    async fn creates_finds_and_lists_systems() {
        let (departments, systems) = test_repositories().await;
        let department_a = department(&departments, "dept-a").await;
        let department_b = department(&departments, "dept-b").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let active = systems
            .create_system(new_system("active system", department_a, "active"), now)
            .await
            .expect("active system should be created");
        systems
            .create_system(new_system("disabled system", department_b, "disabled"), now)
            .await
            .expect("disabled system should be created");

        let found = systems
            .find_by_id(active.id)
            .await
            .expect("system lookup should succeed")
            .expect("system should be found");
        assert_eq!(found.name, "active system");
        assert_eq!(found.department_id, department_a);

        let (disabled, total_count) = systems
            .list_systems(Some("disabled"), None, 1, 50)
            .await
            .expect("systems should list");
        assert_eq!(total_count, 1);
        assert_eq!(disabled.len(), 1);
        assert_eq!(disabled[0].name, "disabled system");

        let (department_a_systems, total_count) = systems
            .list_systems(None, Some(department_a), 1, 50)
            .await
            .expect("systems should list by department");
        assert_eq!(total_count, 1);
        assert_eq!(department_a_systems[0].id, active.id);
    }

    #[tokio::test]
    async fn updates_and_disables_system() {
        let (departments, systems) = test_repositories().await;
        let department_a = department(&departments, "dept-a").await;
        let department_b = department(&departments, "dept-b").await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let system = systems
            .create_system(new_system("original", department_a, "active"), created_at)
            .await
            .expect("system should be created");

        let updated = systems
            .update_system(
                &system,
                SystemChanges {
                    name: Some("updated".to_string()),
                    department_id: Some(department_b),
                    ..SystemChanges::default()
                },
                updated_at,
            )
            .await
            .expect("system should update");

        assert_eq!(updated.name, "updated");
        assert_eq!(updated.department_id, department_b);
        assert_eq!(updated.updated_at, updated_at);

        let disabled_at = Utc.with_ymd_and_hms(2026, 7, 7, 2, 0, 0).unwrap();
        let disabled = systems
            .update_status(&updated, "disabled", disabled_at)
            .await
            .expect("system should be disabled");
        assert_eq!(disabled.status, "disabled");
        assert_eq!(disabled.updated_at, disabled_at);
    }

    #[tokio::test]
    async fn deletes_system_by_id() {
        let (departments, systems) = test_repositories().await;
        let department_id = department(&departments, "dept-a").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let system = systems
            .create_system(new_system("delete me", department_id, "active"), now)
            .await
            .expect("system should be created");

        let deleted = systems
            .delete_by_id(system.id)
            .await
            .expect("system delete should succeed");

        assert!(deleted);
        assert!(
            systems
                .find_by_id(system.id)
                .await
                .expect("system lookup should succeed")
                .is_none()
        );
        assert!(
            !systems
                .delete_by_id(Uuid::new_v4())
                .await
                .expect("missing system delete should succeed")
        );
    }

    #[tokio::test]
    async fn rejects_required_fields_and_missing_department_fk() {
        let (departments, systems) = test_repositories().await;
        let department_id = department(&departments, "dept-a").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        assert!(matches!(
            systems
                .create_system(new_system(" ", department_id, "active"), now)
                .await,
            Err(RepositoryError::MissingRequiredField { field: "name" })
        ));

        assert!(
            systems
                .create_system(
                    new_system("missing department", Uuid::new_v4(), "active"),
                    now
                )
                .await
                .is_err()
        );
    }
}

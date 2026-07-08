use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{entities::customers, repositories::RepositoryError};

#[derive(Clone)]
pub struct CustomerRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewCustomer {
    pub name: String,
    pub creator_user_id: Uuid,
    pub department_id: Uuid,
    pub system_id: Uuid,
    pub store_id: Uuid,
    pub remark: Option<String>,
    pub status: String,
    pub attachments: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CustomerChanges {
    pub name: Option<String>,
    pub department_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub remark: Option<Option<String>>,
    pub status: Option<String>,
    pub attachments: Option<Option<String>>,
}

impl CustomerChanges {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.department_id.is_none()
            && self.system_id.is_none()
            && self.store_id.is_none()
            && self.remark.is_none()
            && self.status.is_none()
            && self.attachments.is_none()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CustomerFilters<'a> {
    pub status_filter: Option<&'a str>,
    pub department_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
    pub creator_user_id: Option<Uuid>,
    pub name_keyword: Option<&'a str>,
}

impl CustomerRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(
        level = "info",
        skip(self, customer),
        fields(
            name = %customer.name,
            creator_user_id = %customer.creator_user_id,
            department_id = %customer.department_id,
            system_id = %customer.system_id,
            store_id = %customer.store_id,
            status = %customer.status
        )
    )]
    pub async fn create_customer(
        &self,
        customer: NewCustomer,
        now: DateTime<Utc>,
    ) -> Result<customers::Model, RepositoryError> {
        validate_required("name", &customer.name)?;
        validate_required("status", &customer.status)?;

        let customer = customers::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(customer.name),
            creator_user_id: Set(customer.creator_user_id),
            department_id: Set(customer.department_id),
            system_id: Set(customer.system_id),
            store_id: Set(customer.store_id),
            remark: Set(customer.remark),
            status: Set(customer.status),
            attachments: Set(customer.attachments),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(customer_id = %customer.id, "created customer");
        Ok(customer)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<customers::Model>, RepositoryError> {
        let customer = customers::Entity::find_by_id(customer_id)
            .one(&self.db)
            .await?;

        debug!(found = customer.is_some(), %customer_id, "looked up customer by id");
        Ok(customer)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_customers(
        &self,
        filters: CustomerFilters<'_>,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<customers::Model>, u64), RepositoryError> {
        let mut query = customers::Entity::find()
            .order_by_asc(customers::Column::CreatedAt)
            .order_by_asc(customers::Column::Name);

        if let Some(status_filter) = filters.status_filter {
            validate_required("status_filter", status_filter)?;
            query = query.filter(customers::Column::Status.eq(status_filter.trim()));
        }
        if let Some(department_id) = filters.department_id {
            query = query.filter(customers::Column::DepartmentId.eq(department_id));
        }
        if let Some(system_id) = filters.system_id {
            query = query.filter(customers::Column::SystemId.eq(system_id));
        }
        if let Some(store_id) = filters.store_id {
            query = query.filter(customers::Column::StoreId.eq(store_id));
        }
        if let Some(creator_user_id) = filters.creator_user_id {
            query = query.filter(customers::Column::CreatorUserId.eq(creator_user_id));
        }
        if let Some(name_keyword) = filters.name_keyword {
            validate_required("name_keyword", name_keyword)?;
            query = query.filter(customers::Column::Name.contains(name_keyword.trim()));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let customers = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = customers.len(),
            total_count, page_number, page_size, "listed customers"
        );
        Ok((customers, total_count))
    }

    #[tracing::instrument(level = "info", skip(self, customer, changes), fields(customer_id = %customer.id))]
    pub async fn update_customer(
        &self,
        customer: &customers::Model,
        changes: CustomerChanges,
        now: DateTime<Utc>,
    ) -> Result<customers::Model, RepositoryError> {
        let mut active: customers::ActiveModel = customer.clone().into();

        if let Some(name) = changes.name {
            validate_required("name", &name)?;
            active.name = Set(name);
        }
        if let Some(department_id) = changes.department_id {
            active.department_id = Set(department_id);
        }
        if let Some(system_id) = changes.system_id {
            active.system_id = Set(system_id);
        }
        if let Some(store_id) = changes.store_id {
            active.store_id = Set(store_id);
        }
        if let Some(remark) = changes.remark {
            active.remark = Set(remark);
        }
        if let Some(status) = changes.status {
            validate_required("status", &status)?;
            active.status = Set(status);
        }
        if let Some(attachments) = changes.attachments {
            active.attachments = Set(attachments);
        }
        active.updated_at = Set(now);

        let customer = active.update(&self.db).await?;
        info!(customer_id = %customer.id, "updated customer");
        Ok(customer)
    }

    #[tracing::instrument(level = "info", skip(self), fields(customer_id = %customer.id, status = %status))]
    pub async fn update_status(
        &self,
        customer: &customers::Model,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<customers::Model, RepositoryError> {
        validate_required("status", status)?;

        let mut active: customers::ActiveModel = customer.clone().into();
        active.status = Set(status.trim().to_string());
        active.updated_at = Set(now);
        let customer = active.update(&self.db).await?;

        info!(customer_id = %customer.id, status = %customer.status, "updated customer status");
        Ok(customer)
    }

    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_by_id(&self, customer_id: Uuid) -> Result<bool, RepositoryError> {
        let result = customers::Entity::delete_by_id(customer_id)
            .exec(&self.db)
            .await?;
        let deleted = result.rows_affected > 0;

        info!(%customer_id, deleted, "deleted customer by id");
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
        repositories::{
            departments::DepartmentRepository,
            stores::{NewStore, StoreRepository},
            systems::{NewSystem, SystemRepository},
            users::UserRepository,
        },
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
        UserRepository,
        DepartmentRepository,
        SystemRepository,
        StoreRepository,
        CustomerRepository,
    ) {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        (
            UserRepository::new(db.clone()),
            DepartmentRepository::new(db.clone()),
            SystemRepository::new(db.clone()),
            StoreRepository::new(db.clone()),
            CustomerRepository::new(db),
        )
    }

    async fn scope(
        users: &UserRepository,
        departments: &DepartmentRepository,
        systems: &SystemRepository,
        stores: &StoreRepository,
        name: &str,
    ) -> (Uuid, Uuid, Uuid, Uuid) {
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let user = users
            .find_or_create_for_login(&format!("{name}-user"), now)
            .await
            .expect("user should be created");
        let department = departments
            .insert_department(Uuid::new_v4(), "manual", name, name, None, now)
            .await
            .expect("department should be created");
        let system = systems
            .create_system(
                NewSystem {
                    name: name.to_string(),
                    department_id: department.id,
                    status: "active".to_string(),
                },
                now,
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
                now,
            )
            .await
            .expect("store should be created");

        (user.id, department.id, system.id, store.id)
    }

    fn new_customer(
        name: &str,
        creator_user_id: Uuid,
        department_id: Uuid,
        system_id: Uuid,
        store_id: Uuid,
    ) -> NewCustomer {
        NewCustomer {
            name: name.to_string(),
            creator_user_id,
            department_id,
            system_id,
            store_id,
            remark: Some("remark".to_string()),
            status: "active".to_string(),
            attachments: Some(r#"[{"file_id":"img-1"}]"#.to_string()),
        }
    }

    #[tokio::test]
    async fn creates_finds_and_lists_customers() {
        let (users, departments, systems, stores, customers) = test_repositories().await;
        let (user_a, department_a, system_a, store_a) =
            scope(&users, &departments, &systems, &stores, "scope-a").await;
        let (user_b, department_b, system_b, store_b) =
            scope(&users, &departments, &systems, &stores, "scope-b").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        let active = customers
            .create_customer(
                new_customer("Alice", user_a, department_a, system_a, store_a),
                now,
            )
            .await
            .expect("active customer should be created");
        customers
            .create_customer(
                NewCustomer {
                    status: "disabled".to_string(),
                    ..new_customer("Bob", user_b, department_b, system_b, store_b)
                },
                now,
            )
            .await
            .expect("disabled customer should be created");

        let found = customers
            .find_by_id(active.id)
            .await
            .expect("customer lookup should succeed")
            .expect("customer should be found");
        assert_eq!(found.name, "Alice");
        assert_eq!(found.creator_user_id, user_a);

        let (listed, total_count) = customers
            .list_customers(
                CustomerFilters {
                    status_filter: Some("active"),
                    department_id: Some(department_a),
                    system_id: Some(system_a),
                    store_id: Some(store_a),
                    creator_user_id: Some(user_a),
                    name_keyword: Some("lic"),
                },
                1,
                50,
            )
            .await
            .expect("customers should list");
        assert_eq!(total_count, 1);
        assert_eq!(listed[0].id, active.id);

        let (disabled, total_count) = customers
            .list_customers(
                CustomerFilters {
                    status_filter: Some("disabled"),
                    ..CustomerFilters::default()
                },
                1,
                50,
            )
            .await
            .expect("disabled customers should list");
        assert_eq!(total_count, 1);
        assert_eq!(disabled[0].name, "Bob");
    }

    #[tokio::test]
    async fn updates_clears_disables_and_deletes_customer() {
        let (users, departments, systems, stores, customers) = test_repositories().await;
        let (user_a, department_a, system_a, store_a) =
            scope(&users, &departments, &systems, &stores, "scope-a").await;
        let (_, department_b, system_b, store_b) =
            scope(&users, &departments, &systems, &stores, "scope-b").await;
        let created_at = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let updated_at = Utc.with_ymd_and_hms(2026, 7, 7, 1, 0, 0).unwrap();
        let customer = customers
            .create_customer(
                new_customer("Alice", user_a, department_a, system_a, store_a),
                created_at,
            )
            .await
            .expect("customer should be created");

        let updated = customers
            .update_customer(
                &customer,
                CustomerChanges {
                    name: Some("Alice Updated".to_string()),
                    department_id: Some(department_b),
                    system_id: Some(system_b),
                    store_id: Some(store_b),
                    remark: Some(None),
                    attachments: Some(None),
                    ..CustomerChanges::default()
                },
                updated_at,
            )
            .await
            .expect("customer should update");

        assert_eq!(updated.name, "Alice Updated");
        assert_eq!(updated.department_id, department_b);
        assert_eq!(updated.system_id, system_b);
        assert_eq!(updated.store_id, store_b);
        assert_eq!(updated.remark, None);
        assert_eq!(updated.attachments, None);
        assert_eq!(updated.updated_at, updated_at);

        let disabled_at = Utc.with_ymd_and_hms(2026, 7, 7, 2, 0, 0).unwrap();
        let disabled = customers
            .update_status(&updated, "disabled", disabled_at)
            .await
            .expect("customer should be disabled");
        assert_eq!(disabled.status, "disabled");

        let deleted = customers
            .delete_by_id(disabled.id)
            .await
            .expect("customer delete should succeed");
        assert!(deleted);
        assert!(
            customers
                .find_by_id(disabled.id)
                .await
                .expect("customer lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn rejects_required_fields_and_missing_foreign_keys() {
        let (users, departments, systems, stores, customers) = test_repositories().await;
        let (user_id, department_id, system_id, store_id) =
            scope(&users, &departments, &systems, &stores, "scope-a").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        assert!(matches!(
            customers
                .create_customer(
                    new_customer(" ", user_id, department_id, system_id, store_id),
                    now
                )
                .await,
            Err(RepositoryError::MissingRequiredField { field: "name" })
        ));

        assert!(
            customers
                .create_customer(
                    new_customer(
                        "Missing User",
                        Uuid::new_v4(),
                        department_id,
                        system_id,
                        store_id
                    ),
                    now,
                )
                .await
                .is_err()
        );
    }
}

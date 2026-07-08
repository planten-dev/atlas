use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseTransaction};
use serde_json::Value;
use tracing::debug;
use uuid::Uuid;

use crate::{
    entities::events::{ApprovalStatus, EventType},
    repositories::{
        RepositoryError,
        events::{EventRepository, NewEvent},
    },
};

#[derive(Clone)]
pub struct AuditService {
    events: EventRepository,
}

impl AuditService {
    pub fn new(events: EventRepository) -> Self {
        Self { events }
    }

    pub async fn begin(&self) -> Result<DatabaseTransaction, RepositoryError> {
        self.events.begin().await
    }

    pub async fn record_create<C: ConnectionTrait>(
        &self,
        conn: &C,
        resource_type: &'static str,
        resource_id: Uuid,
        actor_user_id: Option<Uuid>,
        new_value: Value,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        self.record(
            conn,
            AuditEvent {
                resource_type,
                resource_id: Some(resource_id),
                actor_user_id,
                event_type: EventType::Create,
                old_value: None,
                new_value: Some(new_value),
                remark: None,
            },
            now,
        )
        .await
    }

    pub async fn record_update<C: ConnectionTrait>(
        &self,
        conn: &C,
        resource_type: &'static str,
        resource_id: Uuid,
        actor_user_id: Option<Uuid>,
        old_value: Value,
        new_value: Value,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        self.record(
            conn,
            AuditEvent {
                resource_type,
                resource_id: Some(resource_id),
                actor_user_id,
                event_type: EventType::Update,
                old_value: Some(old_value),
                new_value: Some(new_value),
                remark: None,
            },
            now,
        )
        .await
    }

    pub async fn record_delete<C: ConnectionTrait>(
        &self,
        conn: &C,
        resource_type: &'static str,
        resource_id: Uuid,
        actor_user_id: Option<Uuid>,
        old_value: Value,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        self.record(
            conn,
            AuditEvent {
                resource_type,
                resource_id: Some(resource_id),
                actor_user_id,
                event_type: EventType::Delete,
                old_value: Some(old_value),
                new_value: None,
                remark: None,
            },
            now,
        )
        .await
    }

    pub async fn record<C: ConnectionTrait>(
        &self,
        conn: &C,
        event: AuditEvent,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let resource_type = event.resource_type;
        let resource_id = event.resource_id;
        let actor_user_id = event.actor_user_id;
        let event_type = event.event_type;

        self.events
            .insert_event(
                conn,
                NewEvent {
                    resource_type: event.resource_type.to_string(),
                    resource_id: event.resource_id,
                    actor_user_id: event.actor_user_id,
                    event_type: event.event_type,
                    approval_status: ApprovalStatus::None,
                    required_approval_count: None,
                    target_event_id: None,
                    old_value: event.old_value,
                    new_value: event.new_value,
                    remark: event.remark,
                },
                now,
            )
            .await?;
        debug!(
            resource_type,
            resource_id = ?resource_id,
            actor_user_id = ?actor_user_id,
            event_type = ?event_type,
            "recorded audit event"
        );
        Ok(())
    }
}

pub struct AuditEvent {
    pub resource_type: &'static str,
    pub resource_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub event_type: EventType,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub remark: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
        entities::events,
    };
    use chrono::TimeZone;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    use serde_json::json;
    use std::path::PathBuf;

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    async fn audit_service() -> AuditService {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        AuditService::new(EventRepository::new(db))
    }

    #[tokio::test]
    async fn records_create_update_and_delete_events_without_review_status() {
        let audit = audit_service().await;
        let actor = Uuid::new_v4();
        let resource = Uuid::new_v4();
        let now = Utc.with_ymd_and_hms(2026, 7, 8, 1, 2, 3).unwrap();
        let tx = audit.begin().await.expect("transaction should begin");

        audit
            .record_create(
                &tx,
                "test_resources",
                resource,
                Some(actor),
                json!({"name": "created"}),
                now,
            )
            .await
            .expect("create audit should write");
        audit
            .record_update(
                &tx,
                "test_resources",
                resource,
                Some(actor),
                json!({"name": "created"}),
                json!({"name": "updated"}),
                now,
            )
            .await
            .expect("update audit should write");
        audit
            .record_delete(
                &tx,
                "test_resources",
                resource,
                Some(actor),
                json!({"name": "updated"}),
                now,
            )
            .await
            .expect("delete audit should write");
        tx.commit().await.expect("transaction should commit");

        let rows = events::Entity::find()
            .filter(events::Column::ResourceId.eq(resource))
            .all(&audit.events.db)
            .await
            .expect("events should load");

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].event_type, EventType::Create);
        assert_eq!(rows[0].approval_status, ApprovalStatus::None);
        assert_eq!(rows[0].required_approval_count, None);
        assert_eq!(rows[0].target_event_id, None);
        assert_eq!(rows[0].old_value, None);
        assert_eq!(rows[0].new_value, Some(json!({"name": "created"})));
        assert_eq!(rows[1].event_type, EventType::Update);
        assert_eq!(rows[1].old_value, Some(json!({"name": "created"})));
        assert_eq!(rows[1].new_value, Some(json!({"name": "updated"})));
        assert_eq!(rows[2].event_type, EventType::Delete);
        assert_eq!(rows[2].old_value, Some(json!({"name": "updated"})));
        assert_eq!(rows[2].new_value, None);
        assert!(rows.iter().all(|row| row.actor_user_id == Some(actor)));
    }

    #[tokio::test]
    async fn rolled_back_transaction_does_not_keep_audit_event() {
        let audit = audit_service().await;
        let resource = Uuid::new_v4();
        let now = Utc.with_ymd_and_hms(2026, 7, 8, 1, 2, 3).unwrap();
        let tx = audit.begin().await.expect("transaction should begin");

        audit
            .record_create(
                &tx,
                "test_resources",
                resource,
                None,
                json!({"name": "created"}),
                now,
            )
            .await
            .expect("create audit should write");
        tx.rollback().await.expect("transaction should roll back");

        let count = events::Entity::find()
            .filter(events::Column::ResourceId.eq(resource))
            .count(&audit.events.db)
            .await
            .expect("event count should load");
        assert_eq!(count, 0);
    }
}

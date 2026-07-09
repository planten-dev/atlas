use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    entities::events::{self, ApprovalStatus, EventType},
    repositories::RepositoryError,
};

#[derive(Clone)]
pub struct EventRepository {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct NewEvent {
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub event_type: EventType,
    pub approval_status: ApprovalStatus,
    pub required_approval_count: Option<i16>,
    /// Approvers whose votes are all required before the event applies.
    /// Empty for events with no designated approvers, and always empty for
    /// review and audit-only events.
    pub required_approver_ids: Vec<Uuid>,
    /// Names the caller-defined kind for EventType::Custom events.
    pub custom_type: Option<String>,
    pub target_event_id: Option<Uuid>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub remark: Option<String>,
    /// When the event is already final at insertion time (audit-only
    /// events recorded with approval_status = None), set this so the
    /// retention sweeper picks it up. Reviewable submissions leave it None
    /// until they are finalized.
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub event_type: Option<EventType>,
    pub approval_status: Option<ApprovalStatus>,
    pub custom_type: Option<String>,
    pub target_event_id: Option<Uuid>,
}

impl EventRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn begin(&self) -> Result<DatabaseTransaction, RepositoryError> {
        Ok(self.db.begin().await?)
    }

    #[tracing::instrument(level = "info", skip(self, conn, event), fields(resource_type = %event.resource_type))]
    pub async fn insert_event<C: ConnectionTrait>(
        &self,
        conn: &C,
        event: NewEvent,
        now: DateTime<Utc>,
    ) -> Result<events::Model, RepositoryError> {
        let event = events::ActiveModel {
            id: Set(Uuid::new_v4()),
            resource_type: Set(event.resource_type),
            resource_id: Set(event.resource_id),
            actor_user_id: Set(event.actor_user_id),
            event_type: Set(event.event_type),
            approval_status: Set(event.approval_status),
            required_approval_count: Set(event.required_approval_count),
            required_approver_ids: Set(events::RequiredApproverIds(event.required_approver_ids)),
            custom_type: Set(event.custom_type),
            target_event_id: Set(event.target_event_id),
            old_value: Set(event.old_value),
            new_value: Set(event.new_value),
            remark: Set(event.remark),
            created_at: Set(now),
            updated_at: Set(event.updated_at),
        }
        .insert(conn)
        .await?;

        info!(event_id = %event.id, "inserted event");
        Ok(event)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_by_id(
        &self,
        event_id: Uuid,
    ) -> Result<Option<events::Model>, RepositoryError> {
        let event = events::Entity::find_by_id(event_id).one(&self.db).await?;

        debug!(found = event.is_some(), %event_id, "looked up event by id");
        Ok(event)
    }

    /// Loads the event with a row lock so racing reviews serialize on it.
    /// Renders `SELECT ... FOR UPDATE` on Postgres; sea-query emits no lock
    /// clause on SQLite, which is safe there because SQLite serializes
    /// writers anyway.
    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn find_by_id_for_update(
        &self,
        tx: &DatabaseTransaction,
        event_id: Uuid,
    ) -> Result<Option<events::Model>, RepositoryError> {
        let event = events::Entity::find_by_id(event_id)
            .lock_exclusive()
            .one(tx)
            .await?;

        debug!(found = event.is_some(), %event_id, "locked event by id");
        Ok(event)
    }

    /// Loads every approve/reject event pointing at `target_event_id`. The
    /// result is bounded by the target's required_approval_count, so the
    /// caller can count distinct approvers in memory.
    #[tracing::instrument(level = "debug", skip(self, tx))]
    pub async fn list_reviews_for_target(
        &self,
        tx: &DatabaseTransaction,
        target_event_id: Uuid,
    ) -> Result<Vec<events::Model>, RepositoryError> {
        let reviews = events::Entity::find()
            .filter(events::Column::TargetEventId.eq(target_event_id))
            .all(tx)
            .await?;

        debug!(count = reviews.len(), %target_event_id, "listed reviews for target");
        Ok(reviews)
    }

    #[tracing::instrument(level = "debug", skip(self, filter))]
    pub async fn list_events(
        &self,
        filter: EventFilter,
        page_number: u64,
        page_size: u64,
    ) -> Result<(Vec<events::Model>, u64), RepositoryError> {
        let mut query = events::Entity::find()
            .order_by_desc(events::Column::CreatedAt)
            .order_by_asc(events::Column::Id);

        if let Some(resource_type) = filter.resource_type {
            query = query.filter(events::Column::ResourceType.eq(resource_type));
        }
        if let Some(resource_id) = filter.resource_id {
            query = query.filter(events::Column::ResourceId.eq(resource_id));
        }
        if let Some(event_type) = filter.event_type {
            query = query.filter(events::Column::EventType.eq(event_type));
        }
        if let Some(approval_status) = filter.approval_status {
            query = query.filter(events::Column::ApprovalStatus.eq(approval_status));
        }
        if let Some(custom_type) = filter.custom_type {
            query = query.filter(events::Column::CustomType.eq(custom_type));
        }
        if let Some(target_event_id) = filter.target_event_id {
            query = query.filter(events::Column::TargetEventId.eq(target_event_id));
        }

        let paginator = query.paginate(&self.db, page_size);
        let total_count = paginator.num_items().await?;
        let events = paginator.fetch_page(page_number.saturating_sub(1)).await?;

        debug!(
            count = events.len(),
            total_count, page_number, page_size, "listed events"
        );
        Ok((events, total_count))
    }

    /// Finalizes a reviewable event: sets its approval status and
    /// updated_at, and backfills resource_id for applied create events.
    #[tracing::instrument(level = "info", skip(self, tx, event), fields(event_id = %event.id))]
    pub async fn finalize_event(
        &self,
        tx: &DatabaseTransaction,
        event: &events::Model,
        status: ApprovalStatus,
        resource_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<events::Model, RepositoryError> {
        let mut active: events::ActiveModel = event.clone().into();
        active.approval_status = Set(status);
        active.updated_at = Set(Some(now));
        if let Some(resource_id) = resource_id {
            active.resource_id = Set(Some(resource_id));
        }

        let event = active.update(tx).await?;
        info!(event_id = %event.id, "finalized event");
        Ok(event)
    }

    /// Deletes one batch of finalized (approved/rejected) events whose
    /// updated_at is older than `cutoff`, together with the review events
    /// targeting them. Returns the total number of rows deleted; a return
    /// value of 0 means nothing was expired.
    #[tracing::instrument(level = "info", skip(self))]
    pub async fn delete_expired_finalized(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u64,
    ) -> Result<u64, RepositoryError> {
        let tx = self.db.begin().await?;

        // ApprovalStatus::None covers audit-only events, which are final
        // from creation (updated_at set on insert). Approve/reject review
        // events also carry status None but keep updated_at NULL, so they
        // never match the cutoff and are only removed by the cascade below.
        let expired_ids: Vec<Uuid> = events::Entity::find()
            .select_only()
            .column(events::Column::Id)
            .filter(events::Column::ApprovalStatus.is_in([
                ApprovalStatus::Approved,
                ApprovalStatus::Rejected,
                ApprovalStatus::None,
            ]))
            .filter(events::Column::UpdatedAt.lt(cutoff))
            .limit(batch_size)
            .into_tuple()
            .all(&tx)
            .await?;

        if expired_ids.is_empty() {
            tx.rollback().await?;
            return Ok(0);
        }

        let reviews_deleted = events::Entity::delete_many()
            .filter(events::Column::TargetEventId.is_in(expired_ids.clone()))
            .exec(&tx)
            .await?
            .rows_affected;
        let events_deleted = events::Entity::delete_many()
            .filter(events::Column::Id.is_in(expired_ids))
            .exec(&tx)
            .await?
            .rows_affected;

        tx.commit().await?;

        let deleted = reviews_deleted + events_deleted;
        info!(
            deleted,
            reviews_deleted, events_deleted, "deleted expired finalized events"
        );
        Ok(deleted)
    }
}

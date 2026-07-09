use std::collections::HashSet;
use std::sync::Arc;

use chrono::{Duration, Utc};
use sea_orm::{ActiveEnum, SqlErr};
use serde::Serialize;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::events::{EventResponse, ListEventsQuery, ListEventsResponse, ReviewEventRequest},
    entities::events::{ApprovalStatus, EventType},
    repositories::{
        RepositoryError,
        events::{EventFilter, EventRepository, NewEvent},
    },
    services::{
        authz::{AuthzError, AuthzService},
        review::{ApplierRegistry, ApplyError, ReviewableResource},
    },
};

const DEFAULT_PAGE_NUMBER: u64 = 1;
const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 200;

const MAX_RESOURCE_TYPE_LENGTH: usize = 64;
const MAX_CUSTOM_TYPE_LENGTH: usize = 64;
const MAX_REMARK_LENGTH: usize = 2000;
const MAX_REQUIRED_APPROVERS: usize = 32;

/// Rows removed per sweeper transaction; keeps each delete short-lived.
const SWEEP_BATCH_SIZE: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    Approve,
    Reject,
}

/// A create/update/delete submission. Built by the `submit_*` helpers or
/// directly by callers that already hold serialized payloads. The reviewer
/// permission is not part of the submission: it is declared once per
/// resource type via `ReviewableResource::APPROVAL_PERMISSION` and looked
/// up from the registry at review time.
#[derive(Debug, Clone)]
pub struct SubmitEvent {
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub event_type: EventType,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub required_approval_count: i16,
    /// Reviewers whose approve votes are all required, in addition to the
    /// `required_approval_count` threshold, before the event applies.
    /// Empty means no designated approvers. Listed users may review the
    /// event without holding the resource type's approval permission.
    pub required_approver_ids: Vec<Uuid>,
}

/// An audit-only submission: recorded as already-final (approval_status =
/// None (0)), never reviewed, never applied. Use it when the caller has
/// performed (or does not need) the actual mutation itself and only wants
/// the audit trail. The resource_type does not have to be registered in
/// the ApplierRegistry, and old/new_value are free-form.
#[derive(Debug, Clone)]
pub struct RecordEvent {
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub event_type: EventType,
    /// Names the event kind when `event_type` is [`EventType::Custom`]
    /// (required there, forbidden otherwise), e.g. `login` or `export`.
    pub custom_type: Option<String>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct EventService {
    events: EventRepository,
    authz: AuthzService,
    registry: Arc<ApplierRegistry>,
    retention_days: u32,
}

impl EventService {
    pub fn new(
        events: EventRepository,
        authz: AuthzService,
        registry: Arc<ApplierRegistry>,
        retention_days: u32,
    ) -> Self {
        Self {
            events,
            authz,
            registry,
            retention_days,
        }
    }

    // --- submission (internal API used by business handlers) ---

    /// Records a pending create event for a registered resource.
    pub async fn submit_create<R: ReviewableResource>(
        &self,
        actor_user_id: Uuid,
        new_value: &R,
        required_approval_count: i16,
        required_approver_ids: Vec<Uuid>,
    ) -> Result<EventResponse, EventError> {
        self.submit(SubmitEvent {
            resource_type: R::RESOURCE_TYPE.to_string(),
            resource_id: None,
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Create,
            old_value: None,
            new_value: Some(to_json(new_value)?),
            required_approval_count,
            required_approver_ids,
        })
        .await
    }

    /// Records a pending update event for a registered resource.
    pub async fn submit_update<R: ReviewableResource>(
        &self,
        actor_user_id: Uuid,
        resource_id: Uuid,
        old_value: &R,
        new_value: &R,
        required_approval_count: i16,
        required_approver_ids: Vec<Uuid>,
    ) -> Result<EventResponse, EventError> {
        self.submit(SubmitEvent {
            resource_type: R::RESOURCE_TYPE.to_string(),
            resource_id: Some(resource_id),
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Update,
            old_value: Some(to_json(old_value)?),
            new_value: Some(to_json(new_value)?),
            required_approval_count,
            required_approver_ids,
        })
        .await
    }

    /// Records a pending delete event for a registered resource.
    pub async fn submit_delete<R: ReviewableResource>(
        &self,
        actor_user_id: Uuid,
        resource_id: Uuid,
        old_value: &R,
        required_approval_count: i16,
        required_approver_ids: Vec<Uuid>,
    ) -> Result<EventResponse, EventError> {
        self.submit(SubmitEvent {
            resource_type: R::RESOURCE_TYPE.to_string(),
            resource_id: Some(resource_id),
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Delete,
            old_value: Some(to_json(old_value)?),
            new_value: None,
            required_approval_count,
            required_approver_ids,
        })
        .await
    }

    #[tracing::instrument(level = "info", skip(self, submission), fields(resource_type = %submission.resource_type))]
    pub async fn submit(&self, submission: SubmitEvent) -> Result<EventResponse, EventError> {
        validate_submission(&submission)?;
        if !self.registry.contains(&submission.resource_type) {
            return Err(EventError::UnknownResourceType {
                resource_type: submission.resource_type,
            });
        }

        let event = self
            .events
            .insert_event(
                &self.events.db,
                NewEvent {
                    resource_type: submission.resource_type,
                    resource_id: submission.resource_id,
                    actor_user_id: submission.actor_user_id,
                    event_type: submission.event_type,
                    approval_status: ApprovalStatus::Pending,
                    required_approval_count: Some(submission.required_approval_count),
                    required_approver_ids: submission.required_approver_ids,
                    custom_type: None,
                    target_event_id: None,
                    old_value: submission.old_value,
                    new_value: submission.new_value,
                    remark: None,
                    updated_at: None,
                },
                Utc::now(),
            )
            .await?;

        info!(event_id = %event.id, "submitted event for review");
        Ok(EventResponse::from(event))
    }

    // --- audit-only recording (no review, no apply) ---

    /// Records an audit-only create event. `resource_id` may carry the id
    /// of the row the caller already inserted.
    pub async fn record_create<T: Serialize>(
        &self,
        resource_type: &str,
        actor_user_id: Uuid,
        resource_id: Option<Uuid>,
        new_value: &T,
    ) -> Result<EventResponse, EventError> {
        self.record(RecordEvent {
            resource_type: resource_type.to_string(),
            resource_id,
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Create,
            custom_type: None,
            old_value: None,
            new_value: Some(to_json(new_value)?),
        })
        .await
    }

    /// Records an audit-only update event.
    pub async fn record_update<T: Serialize>(
        &self,
        resource_type: &str,
        actor_user_id: Uuid,
        resource_id: Uuid,
        old_value: &T,
        new_value: &T,
    ) -> Result<EventResponse, EventError> {
        self.record(RecordEvent {
            resource_type: resource_type.to_string(),
            resource_id: Some(resource_id),
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Update,
            custom_type: None,
            old_value: Some(to_json(old_value)?),
            new_value: Some(to_json(new_value)?),
        })
        .await
    }

    /// Records an audit-only delete event.
    pub async fn record_delete<T: Serialize>(
        &self,
        resource_type: &str,
        actor_user_id: Uuid,
        resource_id: Uuid,
        old_value: &T,
    ) -> Result<EventResponse, EventError> {
        self.record(RecordEvent {
            resource_type: resource_type.to_string(),
            resource_id: Some(resource_id),
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Delete,
            custom_type: None,
            old_value: Some(to_json(old_value)?),
            new_value: None,
        })
        .await
    }

    /// Records an audit-only event of a caller-defined kind
    /// (EventType::Custom). `custom_type` names the kind, e.g. `login` or
    /// `export`; old/new_value are free-form context and may be None.
    pub async fn record_custom<T: Serialize>(
        &self,
        resource_type: &str,
        custom_type: &str,
        actor_user_id: Uuid,
        resource_id: Option<Uuid>,
        payload: Option<&T>,
    ) -> Result<EventResponse, EventError> {
        self.record(RecordEvent {
            resource_type: resource_type.to_string(),
            resource_id,
            actor_user_id: Some(actor_user_id),
            event_type: EventType::Custom,
            custom_type: Some(custom_type.to_string()),
            old_value: None,
            new_value: payload.map(to_json).transpose()?,
        })
        .await
    }

    /// Inserts an audit-only event: approval_status = None so it can never
    /// be reviewed, and updated_at is set immediately (the event is final
    /// from creation) so the retention sweeper treats it like a finalized
    /// event.
    #[tracing::instrument(level = "info", skip(self, record), fields(resource_type = %record.resource_type))]
    pub async fn record(&self, record: RecordEvent) -> Result<EventResponse, EventError> {
        validate_resource_type(&record.resource_type)?;
        match record.event_type {
            EventType::Approve | EventType::Reject => {
                return Err(EventError::InvalidInput(
                    "approve/reject events are created through review, not record",
                ));
            }
            EventType::Custom => {
                validate_custom_type(record.custom_type.as_deref())?;
            }
            EventType::Create | EventType::Update | EventType::Delete => {
                if record.custom_type.is_some() {
                    return Err(EventError::InvalidInput(
                        "custom_type is only allowed on custom events",
                    ));
                }
            }
        }

        let now = Utc::now();
        let event = self
            .events
            .insert_event(
                &self.events.db,
                NewEvent {
                    resource_type: record.resource_type,
                    resource_id: record.resource_id,
                    actor_user_id: record.actor_user_id,
                    event_type: record.event_type,
                    approval_status: ApprovalStatus::None,
                    required_approval_count: None,
                    required_approver_ids: Vec::new(),
                    custom_type: record.custom_type,
                    target_event_id: None,
                    old_value: record.old_value,
                    new_value: record.new_value,
                    remark: None,
                    updated_at: Some(now),
                },
                now,
            )
            .await?;

        info!(event_id = %event.id, "recorded audit-only event");
        Ok(EventResponse::from(event))
    }

    // --- review ---

    /// Records an approve/reject event for `event_id` and, when the review
    /// finalizes the target, applies the stored change inside the same
    /// transaction. Returns the refreshed target event.
    #[tracing::instrument(level = "info", skip(self, request), fields(event_id = %event_id, reviewer = %reviewer_user_id))]
    pub async fn review(
        &self,
        event_id: Uuid,
        reviewer_user_id: Uuid,
        decision: ReviewDecision,
        request: ReviewEventRequest,
    ) -> Result<EventResponse, EventError> {
        let remark = normalize_remark(request.remark)?;
        let now = Utc::now();
        let tx = self.events.begin().await?;

        let target = self
            .events
            .find_by_id_for_update(&tx, event_id)
            .await?
            .ok_or(EventError::EventNotFound)?;
        if matches!(
            target.event_type,
            EventType::Approve | EventType::Reject | EventType::Custom
        ) {
            return Err(EventError::EventNotReviewable);
        }
        if target.approval_status != ApprovalStatus::Pending {
            return Err(EventError::EventNotPending);
        }

        let reviews = self.events.list_reviews_for_target(&tx, target.id).await?;
        if reviews
            .iter()
            .any(|review| review.actor_user_id == Some(reviewer_user_id))
        {
            return Err(EventError::DuplicateReview);
        }

        // The reviewer permission is declared statically on the payload
        // type (ReviewableResource::APPROVAL_PERMISSION) and resolved
        // through the registry, so pending events always follow the
        // current declaration. Users designated in required_approver_ids
        // may review this event without holding that permission.
        let registration = self.registry.get(&target.resource_type).ok_or_else(|| {
            EventError::UnknownResourceType {
                resource_type: target.resource_type.clone(),
            }
        })?;
        if target.required_approver_ids.0.contains(&reviewer_user_id) {
            debug!("reviewer is a required approver; skipping the approval permission check");
        } else {
            let (object, action) = registration.approval_permission();
            if !self.authz.check(reviewer_user_id, object, action).await? {
                warn!(
                    object,
                    action, "reviewer lacks the required approval permission"
                );
                return Err(EventError::PermissionDenied {
                    permission: format!("{object}:{action}"),
                });
            }
        }

        let review_type = match decision {
            ReviewDecision::Approve => EventType::Approve,
            ReviewDecision::Reject => EventType::Reject,
        };
        let insert_result = self
            .events
            .insert_event(
                &tx,
                NewEvent {
                    resource_type: target.resource_type.clone(),
                    resource_id: target.resource_id,
                    actor_user_id: Some(reviewer_user_id),
                    event_type: review_type,
                    approval_status: ApprovalStatus::None,
                    required_approval_count: None,
                    required_approver_ids: Vec::new(),
                    custom_type: None,
                    target_event_id: Some(target.id),
                    old_value: None,
                    new_value: None,
                    remark,
                    updated_at: None,
                },
                now,
            )
            .await;
        if let Err(RepositoryError::Database(error)) = &insert_result
            && matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_)))
        {
            // Backstop for two concurrent reviews by the same user racing
            // past the in-transaction duplicate check.
            return Err(EventError::DuplicateReview);
        }
        insert_result?;

        let target = match decision {
            ReviewDecision::Reject => {
                // Any single reject vetoes the event.
                self.events
                    .finalize_event(&tx, &target, ApprovalStatus::Rejected, None, now)
                    .await?
            }
            ReviewDecision::Approve => {
                let mut approvers: HashSet<Uuid> = reviews
                    .iter()
                    .filter(|review| review.event_type == EventType::Approve)
                    .filter_map(|review| review.actor_user_id)
                    .collect();
                approvers.insert(reviewer_user_id);

                let required = target.required_approval_count.unwrap_or(1).max(1) as usize;
                let all_required_approved = target
                    .required_approver_ids
                    .0
                    .iter()
                    .all(|id| approvers.contains(id));
                if approvers.len() >= required && all_required_approved {
                    let resource_id = self.apply_target(&tx, &target, now).await?;
                    self.events
                        .finalize_event(&tx, &target, ApprovalStatus::Approved, resource_id, now)
                        .await?
                } else {
                    debug!(
                        approvers = approvers.len(),
                        required, all_required_approved, "approval conditions not yet met"
                    );
                    target
                }
            }
        };

        tx.commit().await.map_err(RepositoryError::from)?;
        info!(
            event_id = %target.id,
            approval_status = target.approval_status.into_value(),
            "recorded review"
        );
        Ok(EventResponse::from(target))
    }

    /// Dispatches the target event's stored payload to the registered
    /// applier. Returns the new row id for create events.
    async fn apply_target(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        target: &crate::entities::events::Model,
        now: chrono::DateTime<Utc>,
    ) -> Result<Option<Uuid>, EventError> {
        let applier = self
            .registry
            .get(&target.resource_type)
            .ok_or_else(|| EventError::UnknownResourceType {
                resource_type: target.resource_type.clone(),
            })?
            .applier();

        match target.event_type {
            EventType::Create => {
                let new_value = target
                    .new_value
                    .clone()
                    .ok_or(EventError::MissingPayload { field: "new_value" })?;
                let resource_id = applier.apply_create(tx, new_value, now).await?;
                Ok(Some(resource_id))
            }
            EventType::Update => {
                let resource_id = target.resource_id.ok_or(EventError::MissingPayload {
                    field: "resource_id",
                })?;
                let new_value = target
                    .new_value
                    .clone()
                    .ok_or(EventError::MissingPayload { field: "new_value" })?;
                applier
                    .apply_update(tx, resource_id, new_value, now)
                    .await?;
                Ok(None)
            }
            EventType::Delete => {
                let resource_id = target.resource_id.ok_or(EventError::MissingPayload {
                    field: "resource_id",
                })?;
                applier.apply_delete(tx, resource_id, now).await?;
                Ok(None)
            }
            // Custom events are audit-only (created via record with
            // approval_status = None) and can never reach the apply path.
            EventType::Approve | EventType::Reject | EventType::Custom => {
                Err(EventError::EventNotReviewable)
            }
        }
    }

    // --- queries ---

    #[tracing::instrument(level = "debug", skip(self, query))]
    pub async fn list_events(
        &self,
        query: ListEventsQuery,
    ) -> Result<ListEventsResponse, EventError> {
        let page_number = query.page_number.unwrap_or(DEFAULT_PAGE_NUMBER);
        let page_size = query.page_size.unwrap_or(DEFAULT_PAGE_SIZE);
        validate_page_number(page_number)?;
        validate_page_size(page_size)?;

        let filter = EventFilter {
            resource_type: query.resource_type,
            resource_id: query.resource_id,
            event_type: query
                .event_type
                .map(|value| parse_enum::<EventType>("event_type", value))
                .transpose()?,
            approval_status: query
                .approval_status
                .map(|value| parse_enum::<ApprovalStatus>("approval_status", value))
                .transpose()?,
            custom_type: query.custom_type,
            target_event_id: query.target_event_id,
        };
        let (events, total_count) = self
            .events
            .list_events(filter, page_number, page_size)
            .await?;

        debug!(
            count = events.len(),
            total_count, page_number, page_size, "listed events through service"
        );
        Ok(ListEventsResponse {
            events: events.into_iter().map(EventResponse::from).collect(),
            page_number,
            page_size,
            total_count,
        })
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn event_detail(&self, event_id: Uuid) -> Result<EventResponse, EventError> {
        let event = self
            .events
            .find_by_id(event_id)
            .await?
            .ok_or(EventError::EventNotFound)?;

        debug!(%event_id, "loaded event detail");
        Ok(EventResponse::from(event))
    }

    // --- retention ---

    /// Deletes finalized events older than the retention window, together
    /// with the review events targeting them. Pending events are never
    /// removed. Returns the total number of rows deleted.
    #[tracing::instrument(level = "info", skip(self))]
    pub async fn sweep_expired(&self, now: chrono::DateTime<Utc>) -> Result<u64, EventError> {
        let cutoff = now - Duration::days(i64::from(self.retention_days));
        let mut total_deleted = 0;

        loop {
            let deleted = self
                .events
                .delete_expired_finalized(cutoff, SWEEP_BATCH_SIZE)
                .await?;
            total_deleted += deleted;
            if deleted == 0 {
                break;
            }
        }

        info!(total_deleted, %cutoff, "swept expired events");
        Ok(total_deleted)
    }
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Authz(#[from] AuthzError),
    #[error("event was not found")]
    EventNotFound,
    #[error("approve and reject events cannot be reviewed")]
    EventNotReviewable,
    #[error("event is no longer pending review")]
    EventNotPending,
    #[error("this user has already reviewed the event")]
    DuplicateReview,
    #[error("reviewer lacks the `{permission}` permission")]
    PermissionDenied { permission: String },
    #[error("resource type `{resource_type}` is not registered for review")]
    UnknownResourceType { resource_type: String },
    #[error("event is missing its {field} and cannot be applied")]
    MissingPayload { field: &'static str },
    #[error("failed to apply the approved change")]
    Apply(#[from] ApplyError),
    #[error("failed to serialize the event payload")]
    Serialization(#[from] serde_json::Error),
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error("{field} must be greater than or equal to {minimum}")]
    InvalidPaginationMinimum { field: &'static str, minimum: u64 },
    #[error("{field} must be less than or equal to {maximum}")]
    InvalidPaginationMaximum { field: &'static str, maximum: u64 },
    #[error("{field} must be one of the documented enum values")]
    InvalidEnumValue { field: &'static str, value: i16 },
}

impl EventError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::Authz(_) => "authorization_engine_error",
            Self::EventNotFound => "event_not_found",
            Self::EventNotReviewable => "event_not_reviewable",
            Self::EventNotPending => "event_not_pending",
            Self::DuplicateReview => "duplicate_review",
            Self::PermissionDenied { .. } => "permission_denied",
            Self::UnknownResourceType { .. } => "unknown_resource_type",
            Self::MissingPayload { .. } => "invalid_event_state",
            Self::Apply(_) => "apply_failed",
            Self::Serialization(_) => "serialization_error",
            Self::InvalidInput(_)
            | Self::InvalidPaginationMinimum { .. }
            | Self::InvalidPaginationMaximum { .. }
            | Self::InvalidEnumValue { .. } => "validation_error",
        }
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<serde_json::Value, EventError> {
    Ok(serde_json::to_value(value)?)
}

fn validate_resource_type(resource_type: &str) -> Result<(), EventError> {
    let resource_type = resource_type.trim();
    if resource_type.is_empty() {
        return Err(EventError::InvalidInput("resource_type must not be empty"));
    }
    if resource_type.chars().count() > MAX_RESOURCE_TYPE_LENGTH {
        return Err(EventError::InvalidInput(
            "resource_type must be at most 64 characters",
        ));
    }

    Ok(())
}

fn validate_custom_type(custom_type: Option<&str>) -> Result<(), EventError> {
    let Some(custom_type) = custom_type else {
        return Err(EventError::InvalidInput(
            "custom events require a custom_type",
        ));
    };
    let custom_type = custom_type.trim();
    if custom_type.is_empty() {
        return Err(EventError::InvalidInput(
            "custom events require a custom_type",
        ));
    }
    if custom_type.chars().count() > MAX_CUSTOM_TYPE_LENGTH {
        return Err(EventError::InvalidInput(
            "custom_type must be at most 64 characters",
        ));
    }

    Ok(())
}

fn validate_submission(submission: &SubmitEvent) -> Result<(), EventError> {
    validate_resource_type(&submission.resource_type)?;
    if submission.required_approval_count < 1 {
        return Err(EventError::InvalidInput(
            "required_approval_count must be at least 1",
        ));
    }
    validate_required_approver_ids(&submission.required_approver_ids)?;

    match submission.event_type {
        EventType::Create => {
            if submission.new_value.is_none() {
                return Err(EventError::InvalidInput(
                    "create events require a new_value",
                ));
            }
            if submission.resource_id.is_some() {
                return Err(EventError::InvalidInput(
                    "create events must not set resource_id",
                ));
            }
            if submission.old_value.is_some() {
                return Err(EventError::InvalidInput(
                    "create events must not set old_value",
                ));
            }
        }
        EventType::Update => {
            if submission.resource_id.is_none() {
                return Err(EventError::InvalidInput(
                    "update events require a resource_id",
                ));
            }
            if submission.new_value.is_none() {
                return Err(EventError::InvalidInput(
                    "update events require a new_value",
                ));
            }
        }
        EventType::Delete => {
            if submission.resource_id.is_none() {
                return Err(EventError::InvalidInput(
                    "delete events require a resource_id",
                ));
            }
            if submission.new_value.is_some() {
                return Err(EventError::InvalidInput(
                    "delete events must not set new_value",
                ));
            }
        }
        EventType::Approve | EventType::Reject => {
            return Err(EventError::InvalidInput(
                "approve/reject events are created through review, not submit",
            ));
        }
        EventType::Custom => {
            return Err(EventError::InvalidInput(
                "custom events are audit-only; use record instead of submit",
            ));
        }
    }

    Ok(())
}

fn validate_required_approver_ids(required_approver_ids: &[Uuid]) -> Result<(), EventError> {
    if required_approver_ids.len() > MAX_REQUIRED_APPROVERS {
        return Err(EventError::InvalidInput(
            "required_approver_ids must contain at most 32 user ids",
        ));
    }
    let mut seen = HashSet::with_capacity(required_approver_ids.len());
    for id in required_approver_ids {
        if id.is_nil() {
            return Err(EventError::InvalidInput(
                "required_approver_ids must not contain the nil uuid",
            ));
        }
        if !seen.insert(*id) {
            return Err(EventError::InvalidInput(
                "required_approver_ids must not contain duplicate user ids",
            ));
        }
    }

    Ok(())
}

fn normalize_remark(remark: Option<String>) -> Result<Option<String>, EventError> {
    let Some(remark) = remark else {
        return Ok(None);
    };
    let remark = remark.trim();
    if remark.is_empty() {
        return Ok(None);
    }
    if remark.chars().count() > MAX_REMARK_LENGTH {
        return Err(EventError::InvalidInput(
            "remark must be at most 2000 characters",
        ));
    }
    Ok(Some(remark.to_string()))
}

fn parse_enum<E: ActiveEnum<Value = i16>>(
    field: &'static str,
    value: i16,
) -> Result<E, EventError> {
    E::try_from_value(&value).map_err(|_| EventError::InvalidEnumValue { field, value })
}

fn validate_page_number(page_number: u64) -> Result<(), EventError> {
    if page_number == 0 {
        return Err(EventError::InvalidPaginationMinimum {
            field: "page_number",
            minimum: 1,
        });
    }

    Ok(())
}

fn validate_page_size(page_size: u64) -> Result<(), EventError> {
    if page_size == 0 {
        return Err(EventError::InvalidPaginationMinimum {
            field: "page_size",
            minimum: 1,
        });
    }

    if page_size > MAX_PAGE_SIZE {
        return Err(EventError::InvalidPaginationMaximum {
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
        entities::{product_category, products},
        repositories::{authz::AuthzRepository, users::UserRepository},
        services::review::ReviewableResource,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone};
    use sea_orm::entity::prelude::Decimal;
    use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait, Set};
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;
    use std::str::FromStr;

    /// Test payload wired to the real products table: proves the stored
    /// JSON round-trips back into a struct and lands in a business table.
    #[derive(Debug, Serialize, Deserialize)]
    struct TestProduct {
        name: String,
        unit_price: String,
    }

    #[async_trait]
    impl ReviewableResource for TestProduct {
        const RESOURCE_TYPE: &'static str = "products";
        const APPROVAL_PERMISSION: &'static str = "products:approve";

        async fn apply_insert(
            self,
            tx: &DatabaseTransaction,
            now: DateTime<Utc>,
        ) -> Result<Uuid, ApplyError> {
            // Products require a category; use one of the seeded defaults.
            let category = product_category::Entity::find()
                .one(tx)
                .await?
                .ok_or(ApplyError::ResourceMissing)?;
            let product = products::ActiveModel {
                id: Set(Uuid::new_v4()),
                name: Set(self.name),
                category_id: Set(category.id),
                series: Set(None),
                brand_name: Set(None),
                specification: Set(None),
                unit: Set(None),
                unit_price: Set(
                    Decimal::from_str(&self.unit_price).map_err(|_| ApplyError::ResourceMissing)?
                ),
                status: Set("active".to_string()),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(tx)
            .await?;
            Ok(product.id)
        }

        async fn apply_update(
            self,
            tx: &DatabaseTransaction,
            resource_id: Uuid,
            now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            let product = products::Entity::find_by_id(resource_id)
                .one(tx)
                .await?
                .ok_or(ApplyError::ResourceMissing)?;
            let mut active: products::ActiveModel = product.into();
            active.name = Set(self.name);
            active.unit_price =
                Set(Decimal::from_str(&self.unit_price).map_err(|_| ApplyError::ResourceMissing)?);
            active.updated_at = Set(now);
            active.update(tx).await?;
            Ok(())
        }

        async fn apply_delete(
            tx: &DatabaseTransaction,
            resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            let result = products::Entity::delete_by_id(resource_id).exec(tx).await?;
            if result.rows_affected == 0 {
                return Err(ApplyError::ResourceMissing);
            }
            Ok(())
        }
    }

    /// Second registered type with its own approval permission, used to
    /// prove reviews are gated per resource type. Never applied in tests
    /// (its events stay below their approval threshold).
    #[derive(Debug, Serialize, Deserialize)]
    struct TestDocument {
        title: String,
    }

    #[async_trait]
    impl ReviewableResource for TestDocument {
        const RESOURCE_TYPE: &'static str = "finance_docs";
        const APPROVAL_PERMISSION: &'static str = "finance:docs:approve";

        async fn apply_insert(
            self,
            _tx: &DatabaseTransaction,
            _now: DateTime<Utc>,
        ) -> Result<Uuid, ApplyError> {
            unimplemented!("TestDocument events are never applied in tests")
        }

        async fn apply_update(
            self,
            _tx: &DatabaseTransaction,
            _resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            unimplemented!("TestDocument events are never applied in tests")
        }

        async fn apply_delete(
            _tx: &DatabaseTransaction,
            _resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            unimplemented!("TestDocument events are never applied in tests")
        }
    }

    struct Harness {
        service: EventService,
        events: EventRepository,
        authz: AuthzService,
        users: UserRepository,
        db: sea_orm::DatabaseConnection,
    }

    impl Harness {
        async fn new() -> Self {
            let database = DatabaseConfig {
                kind: DatabaseKind::SqliteMemory,
                url: "postgres://unused".to_string(),
                sqlite_file: PathBuf::from("unused.sqlite"),
            };
            let db = db::connect_and_migrate(&database)
                .await
                .expect("sqlite memory database should initialize");
            let users = UserRepository::new(db.clone());
            let mut registry = ApplierRegistry::new();
            registry.register::<TestProduct>();
            registry.register::<TestDocument>();
            // Merge approval permissions into the catalog, mirroring the
            // startup wiring in main.rs, so reviewer policies validate.
            let mut catalog = crate::services::authz_catalog::PermissionCatalog::builtin();
            for (resource_type, object, action) in registry.approval_permissions() {
                catalog.add_permission(object, action, "审核", resource_type);
            }
            let authz = AuthzService::with_catalog(AuthzRepository::new(db.clone()), catalog)
                .await
                .expect("authz service should initialize");
            let events = EventRepository::new(db.clone());
            let service = EventService::new(events.clone(), authz.clone(), Arc::new(registry), 180);
            Self {
                service,
                events,
                authz,
                users,
                db,
            }
        }

        async fn user(&self, dingtalk_id: &str) -> Uuid {
            self.users
                .find_or_create_for_login(dingtalk_id, Utc::now())
                .await
                .expect("user should be created")
                .id
        }

        /// Creates a user holding the given `object:action` permission.
        async fn reviewer(&self, dingtalk_id: &str, permission: &str) -> Uuid {
            let user = self.user(dingtalk_id).await;
            let (object, action) = permission
                .rsplit_once(':')
                .expect("permission should be object:action");
            self.authz
                .create_policy(
                    "user".to_string(),
                    user,
                    object.to_string(),
                    action.to_string(),
                    "allow".to_string(),
                )
                .await
                .expect("policy should be created");
            user
        }

        async fn approve(
            &self,
            event_id: Uuid,
            reviewer: Uuid,
        ) -> Result<EventResponse, EventError> {
            self.service
                .review(
                    event_id,
                    reviewer,
                    ReviewDecision::Approve,
                    ReviewEventRequest::default(),
                )
                .await
        }

        async fn reject(
            &self,
            event_id: Uuid,
            reviewer: Uuid,
            remark: Option<&str>,
        ) -> Result<EventResponse, EventError> {
            self.service
                .review(
                    event_id,
                    reviewer,
                    ReviewDecision::Reject,
                    ReviewEventRequest {
                        remark: remark.map(str::to_string),
                    },
                )
                .await
        }
    }

    fn test_product(name: &str) -> TestProduct {
        TestProduct {
            name: name.to_string(),
            unit_price: "12.30".to_string(),
        }
    }

    #[tokio::test]
    async fn create_event_applies_after_enough_approvals() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer_a = h.reviewer("reviewer-a", "products:approve").await;
        let reviewer_b = h.reviewer("reviewer-b", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("reviewed product"), 2, vec![])
            .await
            .expect("event should be submitted");
        assert_eq!(event.event_type, 0);
        assert_eq!(event.approval_status, 1);
        assert_eq!(event.required_approval_count, Some(2));
        assert_eq!(event.resource_id, None);
        assert_eq!(event.updated_at, None);

        let after_first = h
            .approve(event.id, reviewer_a)
            .await
            .expect("first approval should succeed");
        assert_eq!(after_first.approval_status, 1);
        assert!(
            products::Entity::find()
                .all(&h.db)
                .await
                .expect("products should list")
                .is_empty(),
            "nothing must be applied before the threshold"
        );

        let approved = h
            .approve(event.id, reviewer_b)
            .await
            .expect("second approval should finalize");
        assert_eq!(approved.approval_status, 2);
        assert!(approved.updated_at.is_some());
        let resource_id = approved
            .resource_id
            .expect("resource_id should be backfilled");

        let product = products::Entity::find_by_id(resource_id)
            .one(&h.db)
            .await
            .expect("product lookup should succeed")
            .expect("approved product should exist");
        assert_eq!(product.name, "reviewed product");
        assert_eq!(product.unit_price, Decimal::new(1230, 2));

        // Review events stay approval_status = 0 with updated_at = NULL.
        let reviews = h
            .service
            .list_events(ListEventsQuery {
                target_event_id: Some(event.id),
                ..ListEventsQuery::default()
            })
            .await
            .expect("reviews should list");
        assert_eq!(reviews.total_count, 2);
        for review in &reviews.events {
            assert_eq!(review.event_type, 3);
            assert_eq!(review.approval_status, 0);
            assert_eq!(review.updated_at, None);
            assert_eq!(review.required_approval_count, None);
        }
    }

    #[tokio::test]
    async fn update_and_delete_events_apply_round_trip() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer = h.reviewer("reviewer", "products:approve").await;

        let create = h
            .service
            .submit_create(actor, &test_product("original"), 1, vec![])
            .await
            .expect("create event should be submitted");
        let created = h
            .approve(create.id, reviewer)
            .await
            .expect("create should be approved");
        let resource_id = created.resource_id.expect("resource id");

        let update = h
            .service
            .submit_update(
                actor,
                resource_id,
                &test_product("original"),
                &TestProduct {
                    name: "renamed".to_string(),
                    unit_price: "99.99".to_string(),
                },
                1,
                vec![],
            )
            .await
            .expect("update event should be submitted");
        assert!(update.old_value.is_some());
        h.approve(update.id, reviewer)
            .await
            .expect("update should be approved");
        let product = products::Entity::find_by_id(resource_id)
            .one(&h.db)
            .await
            .expect("product lookup should succeed")
            .expect("product should exist");
        assert_eq!(product.name, "renamed");
        assert_eq!(product.unit_price, Decimal::new(9999, 2));

        let delete = h
            .service
            .submit_delete(actor, resource_id, &test_product("renamed"), 1, vec![])
            .await
            .expect("delete event should be submitted");
        h.approve(delete.id, reviewer)
            .await
            .expect("delete should be approved");
        assert!(
            products::Entity::find_by_id(resource_id)
                .one(&h.db)
                .await
                .expect("product lookup should succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn single_reject_vetoes_the_event() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer_a = h.reviewer("reviewer-a", "products:approve").await;
        let reviewer_b = h.reviewer("reviewer-b", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("vetoed"), 3, vec![])
            .await
            .expect("event should be submitted");

        let rejected = h
            .reject(event.id, reviewer_a, Some("looks wrong"))
            .await
            .expect("reject should succeed");
        assert_eq!(rejected.approval_status, 3);
        assert!(rejected.updated_at.is_some());
        assert!(
            products::Entity::find()
                .all(&h.db)
                .await
                .expect("products should list")
                .is_empty()
        );

        // Finalized events accept no further reviews.
        assert!(matches!(
            h.approve(event.id, reviewer_b).await,
            Err(EventError::EventNotPending)
        ));

        let reviews = h
            .service
            .list_events(ListEventsQuery {
                target_event_id: Some(event.id),
                ..ListEventsQuery::default()
            })
            .await
            .expect("reviews should list");
        assert_eq!(reviews.total_count, 1);
        assert_eq!(reviews.events[0].event_type, 4);
        assert_eq!(reviews.events[0].remark.as_deref(), Some("looks wrong"));
    }

    #[tokio::test]
    async fn actor_may_review_their_own_event() {
        // Self-review is explicitly allowed: an actor holding the approval
        // permission counts toward the threshold of their own event.
        let h = Harness::new().await;
        let actor = h.reviewer("actor-reviewer", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("self reviewed"), 1, vec![])
            .await
            .expect("event should be submitted");
        let approved = h
            .approve(event.id, actor)
            .await
            .expect("self review should be allowed");
        assert_eq!(approved.approval_status, 2);
        assert!(approved.resource_id.is_some());
    }

    #[tokio::test]
    async fn approval_waits_for_every_required_approver() {
        // Threshold met but a designated approver has not voted yet: the
        // event stays pending until every listed user has approved.
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer_a = h.reviewer("reviewer-a", "products:approve").await;
        let reviewer_b = h.reviewer("reviewer-b", "products:approve").await;
        let designated = h.reviewer("designated", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("designated"), 2, vec![designated])
            .await
            .expect("event should be submitted");
        assert_eq!(event.required_approver_ids, vec![designated]);

        h.approve(event.id, reviewer_a)
            .await
            .expect("first approval should succeed");
        let after_threshold = h
            .approve(event.id, reviewer_b)
            .await
            .expect("second approval should succeed");
        assert_eq!(
            after_threshold.approval_status, 1,
            "threshold alone must not finalize while a required approver is missing"
        );
        assert!(
            products::Entity::find()
                .all(&h.db)
                .await
                .expect("products should list")
                .is_empty(),
            "nothing must be applied before every required approver votes"
        );

        let approved = h
            .approve(event.id, designated)
            .await
            .expect("designated approval should finalize");
        assert_eq!(approved.approval_status, 2);
        assert!(approved.resource_id.is_some());
    }

    #[tokio::test]
    async fn required_approvers_alone_do_not_satisfy_the_threshold() {
        // Every designated approver has voted but the count threshold is
        // still short: both conditions must hold.
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let designated = h.reviewer("designated", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("threshold"), 2, vec![designated])
            .await
            .expect("event should be submitted");

        let after_designated = h
            .approve(event.id, designated)
            .await
            .expect("designated approval should succeed");
        assert_eq!(
            after_designated.approval_status, 1,
            "one vote must not finalize a two-vote threshold"
        );
        assert!(
            products::Entity::find()
                .all(&h.db)
                .await
                .expect("products should list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn required_approver_reviews_without_the_approval_permission() {
        // Designated approvers are exempt from the approval permission on
        // their event; their votes count toward the threshold too.
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let designated = h.user("designated-no-permission").await;

        let event = h
            .service
            .submit_create(actor, &test_product("exempt"), 1, vec![designated])
            .await
            .expect("event should be submitted");
        let approved = h
            .approve(event.id, designated)
            .await
            .expect("designated approver should review without the permission");
        assert_eq!(approved.approval_status, 2);
        assert!(approved.resource_id.is_some());

        // The exemption is per event: the same user still needs the
        // permission on an event that does not list them.
        let other = h
            .service
            .submit_create(actor, &test_product("not exempt"), 1, vec![])
            .await
            .expect("event should be submitted");
        assert!(matches!(
            h.approve(other.id, designated).await,
            Err(EventError::PermissionDenied { .. })
        ));
    }

    #[tokio::test]
    async fn required_approver_reject_vetoes_without_the_approval_permission() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let designated = h.user("designated-no-permission").await;

        let event = h
            .service
            .submit_create(
                actor,
                &test_product("vetoed by designated"),
                3,
                vec![designated],
            )
            .await
            .expect("event should be submitted");

        let rejected = h
            .reject(event.id, designated, Some("no"))
            .await
            .expect("designated approver should reject without the permission");
        assert_eq!(rejected.approval_status, 3);
        assert!(
            products::Entity::find()
                .all(&h.db)
                .await
                .expect("products should list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn outsiders_still_need_the_permission_on_designated_events() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let designated = h.user("designated").await;
        let outsider = h.user("outsider").await;

        let event = h
            .service
            .submit_create(actor, &test_product("guarded"), 1, vec![designated])
            .await
            .expect("event should be submitted");
        assert!(matches!(
            h.approve(event.id, outsider).await,
            Err(EventError::PermissionDenied { .. })
        ));
    }

    #[tokio::test]
    async fn required_approver_duplicate_review_is_rejected() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let designated = h.user("designated").await;

        let event = h
            .service
            .submit_create(actor, &test_product("dup designated"), 2, vec![designated])
            .await
            .expect("event should be submitted");
        h.approve(event.id, designated)
            .await
            .expect("first review should succeed");
        assert!(matches!(
            h.approve(event.id, designated).await,
            Err(EventError::DuplicateReview)
        ));
    }

    #[tokio::test]
    async fn submit_validates_required_approver_ids() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let user = Uuid::new_v4();

        // Duplicate ids are a caller bug and must fail loudly.
        assert!(matches!(
            h.service
                .submit_create(actor, &test_product("dup ids"), 1, vec![user, user])
                .await,
            Err(EventError::InvalidInput(_))
        ));

        // The nil uuid can never approve anything.
        assert!(matches!(
            h.service
                .submit_create(actor, &test_product("nil id"), 1, vec![Uuid::nil()])
                .await,
            Err(EventError::InvalidInput(_))
        ));

        // At most 32 designated approvers.
        let too_many: Vec<Uuid> = (0..33).map(|_| Uuid::new_v4()).collect();
        assert!(matches!(
            h.service
                .submit_create(actor, &test_product("too many"), 1, too_many)
                .await,
            Err(EventError::InvalidInput(_))
        ));

        // Exactly 32 is accepted and round-trips through the database.
        let at_limit: Vec<Uuid> = (0..32).map(|_| Uuid::new_v4()).collect();
        let event = h
            .service
            .submit_create(actor, &test_product("at limit"), 1, at_limit.clone())
            .await
            .expect("event should be submitted");
        let detail = h
            .service
            .event_detail(event.id)
            .await
            .expect("event should load");
        assert_eq!(detail.required_approver_ids, at_limit);
    }

    #[tokio::test]
    async fn review_and_audit_events_carry_no_required_approvers() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let designated = h.reviewer("designated", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("empty lists"), 1, vec![designated])
            .await
            .expect("event should be submitted");
        h.approve(event.id, designated)
            .await
            .expect("approval should finalize");

        let reviews = h
            .service
            .list_events(ListEventsQuery {
                target_event_id: Some(event.id),
                ..ListEventsQuery::default()
            })
            .await
            .expect("reviews should list");
        assert_eq!(reviews.total_count, 1);
        assert!(reviews.events[0].required_approver_ids.is_empty());

        let audit = h
            .service
            .record_create(
                "login_audit",
                actor,
                None,
                &serde_json::json!({"ip": "10.0.0.1"}),
            )
            .await
            .expect("audit event should be recorded");
        assert!(audit.required_approver_ids.is_empty());
    }

    #[tokio::test]
    async fn duplicate_review_by_same_user_is_rejected() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer = h.reviewer("reviewer", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("dup"), 2, vec![])
            .await
            .expect("event should be submitted");
        h.approve(event.id, reviewer)
            .await
            .expect("first review should succeed");
        assert!(matches!(
            h.approve(event.id, reviewer).await,
            Err(EventError::DuplicateReview)
        ));
        assert!(matches!(
            h.reject(event.id, reviewer, None).await,
            Err(EventError::DuplicateReview)
        ));
    }

    #[tokio::test]
    async fn reviewer_without_permission_is_denied() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let outsider = h.user("outsider").await;

        let event = h
            .service
            .submit_create(actor, &test_product("guarded"), 1, vec![])
            .await
            .expect("event should be submitted");
        assert!(matches!(
            h.approve(event.id, outsider).await,
            Err(EventError::PermissionDenied { .. })
        ));
    }

    #[tokio::test]
    async fn review_checks_the_permission_declared_on_the_resource_type() {
        // Each resource type's APPROVAL_PERMISSION gates its reviews; a
        // permission on one type grants nothing on another.
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let doc_reviewer = h.reviewer("doc-reviewer", "finance:docs:approve").await;
        let product_reviewer = h.reviewer("product-reviewer", "products:approve").await;

        let product_event = h
            .service
            .submit_create(actor, &test_product("product perm"), 2, vec![])
            .await
            .expect("event should be submitted");
        let doc_event = h
            .service
            .submit_create(
                actor,
                &TestDocument {
                    title: "q3".to_string(),
                },
                2,
                vec![],
            )
            .await
            .expect("event should be submitted");

        // Holding the other type's permission is not enough.
        assert!(matches!(
            h.approve(product_event.id, doc_reviewer).await,
            Err(EventError::PermissionDenied { .. })
        ));
        assert!(matches!(
            h.approve(doc_event.id, product_reviewer).await,
            Err(EventError::PermissionDenied { .. })
        ));

        // The type's own declared permission authorizes the review.
        let reviewed = h
            .approve(doc_event.id, doc_reviewer)
            .await
            .expect("declared permission should authorize the review");
        assert_eq!(reviewed.approval_status, 1);
    }

    #[tokio::test]
    async fn review_events_themselves_cannot_be_reviewed() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer_a = h.reviewer("reviewer-a", "products:approve").await;
        let reviewer_b = h.reviewer("reviewer-b", "products:approve").await;

        let event = h
            .service
            .submit_create(actor, &test_product("meta"), 2, vec![])
            .await
            .expect("event should be submitted");
        h.approve(event.id, reviewer_a)
            .await
            .expect("review should succeed");

        let reviews = h
            .service
            .list_events(ListEventsQuery {
                target_event_id: Some(event.id),
                ..ListEventsQuery::default()
            })
            .await
            .expect("reviews should list");
        let review_id = reviews.events[0].id;
        assert!(matches!(
            h.approve(review_id, reviewer_b).await,
            Err(EventError::EventNotReviewable)
        ));

        assert!(matches!(
            h.approve(Uuid::new_v4(), reviewer_b).await,
            Err(EventError::EventNotFound)
        ));
    }

    #[tokio::test]
    async fn submit_validates_inputs() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;

        // Unregistered resource types fail fast.
        assert!(matches!(
            h.service
                .submit(SubmitEvent {
                    resource_type: "unknown_table".to_string(),
                    resource_id: None,
                    actor_user_id: Some(actor),
                    event_type: EventType::Create,
                    old_value: None,
                    new_value: Some(serde_json::json!({})),
                    required_approval_count: 1,
                    required_approver_ids: Vec::new(),
                })
                .await,
            Err(EventError::UnknownResourceType { .. })
        ));

        // Count must be >= 1.
        assert!(matches!(
            h.service
                .submit_create(actor, &test_product("x"), 0, vec![])
                .await,
            Err(EventError::InvalidInput(_))
        ));

        // Shape rules per event type.
        let base = SubmitEvent {
            resource_type: TestProduct::RESOURCE_TYPE.to_string(),
            resource_id: None,
            actor_user_id: Some(actor),
            event_type: EventType::Create,
            old_value: None,
            new_value: None,
            required_approval_count: 1,
            required_approver_ids: Vec::new(),
        };
        assert!(matches!(
            h.service.submit(base.clone()).await,
            Err(EventError::InvalidInput(_))
        )); // create without new_value
        assert!(matches!(
            h.service
                .submit(SubmitEvent {
                    event_type: EventType::Update,
                    new_value: Some(serde_json::json!({})),
                    ..base.clone()
                })
                .await,
            Err(EventError::InvalidInput(_))
        )); // update without resource_id
        assert!(matches!(
            h.service
                .submit(SubmitEvent {
                    event_type: EventType::Delete,
                    resource_id: Some(Uuid::new_v4()),
                    new_value: Some(serde_json::json!({})),
                    ..base.clone()
                })
                .await,
            Err(EventError::InvalidInput(_))
        )); // delete with new_value
        assert!(matches!(
            h.service
                .submit(SubmitEvent {
                    event_type: EventType::Approve,
                    ..base
                })
                .await,
            Err(EventError::InvalidInput(_))
        )); // approve via submit
    }

    #[tokio::test]
    async fn apply_failure_rolls_back_the_vote() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer = h.reviewer("reviewer", "products:approve").await;

        // Update pointing at a row that does not exist: the final approval
        // fails to apply and everything (including the vote) rolls back.
        let event = h
            .service
            .submit_update(
                actor,
                Uuid::new_v4(),
                &test_product("ghost"),
                &test_product("ghost"),
                1,
                vec![],
            )
            .await
            .expect("event should be submitted");

        assert!(matches!(
            h.approve(event.id, reviewer).await,
            Err(EventError::Apply(ApplyError::ResourceMissing))
        ));

        let detail = h
            .service
            .event_detail(event.id)
            .await
            .expect("event should still exist");
        assert_eq!(detail.approval_status, 1, "event must stay pending");
        let reviews = h
            .service
            .list_events(ListEventsQuery {
                target_event_id: Some(event.id),
                ..ListEventsQuery::default()
            })
            .await
            .expect("reviews should list");
        assert_eq!(reviews.total_count, 0, "the vote must not persist");
    }

    #[tokio::test]
    async fn sweeper_deletes_expired_finalized_events_only() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();

        // Insert events directly through the repository so updated_at can
        // be pinned to controlled values.
        let finalized_old = |days: i64, status: ApprovalStatus| {
            let events = h.events.clone();
            let finalized_at = now - Duration::days(days);
            async move {
                let event = events
                    .insert_event(
                        &events.db,
                        NewEvent {
                            resource_type: "products".to_string(),
                            resource_id: Some(Uuid::new_v4()),
                            actor_user_id: Some(actor),
                            event_type: EventType::Update,
                            approval_status: ApprovalStatus::Pending,
                            required_approval_count: Some(1),
                            required_approver_ids: Vec::new(),
                            custom_type: None,
                            target_event_id: None,
                            old_value: Some(serde_json::json!({})),
                            new_value: Some(serde_json::json!({})),
                            remark: None,
                            updated_at: None,
                        },
                        finalized_at - Duration::days(1),
                    )
                    .await
                    .expect("event should insert");
                let tx = events.begin().await.expect("tx should begin");
                let event = events
                    .finalize_event(&tx, &event, status, None, finalized_at)
                    .await
                    .expect("event should finalize");
                // A review event pointing at it, which must be cascaded.
                events
                    .insert_event(
                        &tx,
                        NewEvent {
                            resource_type: "products".to_string(),
                            resource_id: None,
                            actor_user_id: Some(Uuid::new_v4()),
                            event_type: EventType::Approve,
                            approval_status: ApprovalStatus::None,
                            required_approval_count: None,
                            required_approver_ids: Vec::new(),
                            custom_type: None,
                            target_event_id: Some(event.id),
                            old_value: None,
                            new_value: None,
                            remark: None,
                            updated_at: None,
                        },
                        finalized_at,
                    )
                    .await
                    .expect("review should insert");
                tx.commit().await.expect("tx should commit");
                event.id
            }
        };

        let expired_id = finalized_old(181, ApprovalStatus::Approved).await;
        let fresh_id = finalized_old(179, ApprovalStatus::Rejected).await;

        // An old pending event must never be swept.
        let pending = h
            .events
            .insert_event(
                &h.events.db,
                NewEvent {
                    resource_type: "products".to_string(),
                    resource_id: None,
                    actor_user_id: Some(actor),
                    event_type: EventType::Create,
                    approval_status: ApprovalStatus::Pending,
                    required_approval_count: Some(1),
                    required_approver_ids: Vec::new(),
                    custom_type: None,
                    target_event_id: None,
                    old_value: None,
                    new_value: Some(serde_json::json!({})),
                    remark: None,
                    updated_at: None,
                },
                now - Duration::days(400),
            )
            .await
            .expect("pending event should insert");

        // Audit-only events (status None, updated_at set on insert) age
        // out on the same schedule as finalized events.
        let audit_event = |days: i64| {
            let events = h.events.clone();
            let created_at = now - Duration::days(days);
            async move {
                events
                    .insert_event(
                        &events.db,
                        NewEvent {
                            resource_type: "login_audit".to_string(),
                            resource_id: Some(Uuid::new_v4()),
                            actor_user_id: Some(actor),
                            event_type: EventType::Update,
                            approval_status: ApprovalStatus::None,
                            required_approval_count: None,
                            required_approver_ids: Vec::new(),
                            custom_type: None,
                            target_event_id: None,
                            old_value: None,
                            new_value: Some(serde_json::json!({})),
                            remark: None,
                            updated_at: Some(created_at),
                        },
                        created_at,
                    )
                    .await
                    .expect("audit event should insert")
                    .id
            }
        };
        let expired_audit_id = audit_event(181).await;
        let fresh_audit_id = audit_event(179).await;

        let deleted = h
            .service
            .sweep_expired(now)
            .await
            .expect("sweep should succeed");
        assert_eq!(
            deleted, 3,
            "expired event, its review event, and the expired audit event"
        );

        assert!(matches!(
            h.service.event_detail(expired_id).await,
            Err(EventError::EventNotFound)
        ));
        assert!(matches!(
            h.service.event_detail(expired_audit_id).await,
            Err(EventError::EventNotFound)
        ));
        assert!(h.service.event_detail(fresh_id).await.is_ok());
        assert!(h.service.event_detail(fresh_audit_id).await.is_ok());
        assert!(h.service.event_detail(pending.id).await.is_ok());

        // Second sweep is a no-op.
        assert_eq!(
            h.service
                .sweep_expired(now)
                .await
                .expect("sweep should succeed"),
            0
        );
    }

    #[tokio::test]
    async fn record_creates_final_audit_events_without_review_or_apply() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer = h.reviewer("reviewer", "products:approve").await;

        // resource_type does not need to be registered in the registry.
        let recorded = h
            .service
            .record_create(
                "login_audit",
                actor,
                Some(Uuid::new_v4()),
                &serde_json::json!({"ip": "10.0.0.1"}),
            )
            .await
            .expect("audit event should be recorded");
        assert_eq!(recorded.event_type, 0);
        assert_eq!(recorded.approval_status, 0);
        assert_eq!(recorded.required_approval_count, None);
        assert_eq!(
            recorded.updated_at,
            Some(recorded.created_at),
            "audit events are final from creation"
        );
        assert!(
            products::Entity::find()
                .all(&h.db)
                .await
                .expect("products should list")
                .is_empty(),
            "record must never apply anything"
        );

        // Audit events can never be reviewed.
        assert!(matches!(
            h.approve(recorded.id, reviewer).await,
            Err(EventError::EventNotPending)
        ));

        // update/delete helpers and validation.
        let updated = h
            .service
            .record_update(
                "login_audit",
                actor,
                Uuid::new_v4(),
                &serde_json::json!({"before": 1}),
                &serde_json::json!({"after": 2}),
            )
            .await
            .expect("audit update should be recorded");
        assert_eq!(updated.event_type, 1);
        let deleted = h
            .service
            .record_delete(
                "login_audit",
                actor,
                Uuid::new_v4(),
                &serde_json::json!({"gone": true}),
            )
            .await
            .expect("audit delete should be recorded");
        assert_eq!(deleted.event_type, 2);

        assert!(matches!(
            h.service
                .record(RecordEvent {
                    resource_type: " ".to_string(),
                    resource_id: None,
                    actor_user_id: Some(actor),
                    event_type: EventType::Create,
                    custom_type: None,
                    old_value: None,
                    new_value: None,
                })
                .await,
            Err(EventError::InvalidInput(_))
        ));
        assert!(matches!(
            h.service
                .record(RecordEvent {
                    resource_type: "login_audit".to_string(),
                    resource_id: None,
                    actor_user_id: Some(actor),
                    event_type: EventType::Approve,
                    custom_type: None,
                    old_value: None,
                    new_value: None,
                })
                .await,
            Err(EventError::InvalidInput(_))
        ));

        // Audit events are visible through the normal listing filters.
        let audits = h
            .service
            .list_events(ListEventsQuery {
                resource_type: Some("login_audit".to_string()),
                approval_status: Some(0),
                ..ListEventsQuery::default()
            })
            .await
            .expect("audit events should list");
        assert_eq!(audits.total_count, 3);
    }

    #[tokio::test]
    async fn record_custom_events_carry_their_kind() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;
        let reviewer = h.reviewer("reviewer", "products:approve").await;

        let login = h
            .service
            .record_custom(
                "users",
                "login",
                actor,
                Some(actor),
                Some(&serde_json::json!({"ip": "10.0.0.1"})),
            )
            .await
            .expect("custom event should be recorded");
        assert_eq!(login.event_type, 5);
        assert_eq!(login.approval_status, 0);
        assert_eq!(login.custom_type.as_deref(), Some("login"));
        assert_eq!(login.updated_at, Some(login.created_at));

        // Payload is optional for custom events.
        let export = h
            .service
            .record_custom::<serde_json::Value>("reports", "export", actor, None, None)
            .await
            .expect("payload-less custom event should be recorded");
        assert_eq!(export.new_value, None);
        assert_eq!(export.custom_type.as_deref(), Some("export"));

        // Custom events can never be reviewed.
        assert!(matches!(
            h.approve(login.id, reviewer).await,
            Err(EventError::EventNotReviewable)
        ));

        // custom_type is required on custom events and forbidden elsewhere.
        assert!(matches!(
            h.service
                .record(RecordEvent {
                    resource_type: "users".to_string(),
                    resource_id: None,
                    actor_user_id: Some(actor),
                    event_type: EventType::Custom,
                    custom_type: None,
                    old_value: None,
                    new_value: None,
                })
                .await,
            Err(EventError::InvalidInput(_))
        ));
        assert!(matches!(
            h.service
                .record(RecordEvent {
                    resource_type: "users".to_string(),
                    resource_id: None,
                    actor_user_id: Some(actor),
                    event_type: EventType::Create,
                    custom_type: Some("login".to_string()),
                    old_value: None,
                    new_value: Some(serde_json::json!({})),
                })
                .await,
            Err(EventError::InvalidInput(_))
        ));

        // Filterable by custom_type and by event_type = 5.
        let logins = h
            .service
            .list_events(ListEventsQuery {
                custom_type: Some("login".to_string()),
                ..ListEventsQuery::default()
            })
            .await
            .expect("custom events should list");
        assert_eq!(logins.total_count, 1);
        assert_eq!(logins.events[0].id, login.id);
        let customs = h
            .service
            .list_events(ListEventsQuery {
                event_type: Some(5),
                ..ListEventsQuery::default()
            })
            .await
            .expect("custom events should list");
        assert_eq!(customs.total_count, 2);
    }

    #[tokio::test]
    async fn list_events_filters_and_validates() {
        let h = Harness::new().await;
        let actor = h.user("actor").await;

        h.service
            .submit_create(actor, &test_product("a"), 1, vec![])
            .await
            .expect("event should be submitted");
        h.service
            .submit_create(actor, &test_product("b"), 1, vec![])
            .await
            .expect("event should be submitted");

        let all = h
            .service
            .list_events(ListEventsQuery::default())
            .await
            .expect("events should list");
        assert_eq!(all.total_count, 2);

        let pending = h
            .service
            .list_events(ListEventsQuery {
                resource_type: Some("products".to_string()),
                approval_status: Some(1),
                ..ListEventsQuery::default()
            })
            .await
            .expect("events should list");
        assert_eq!(pending.total_count, 2);

        assert!(matches!(
            h.service
                .list_events(ListEventsQuery {
                    approval_status: Some(9),
                    ..ListEventsQuery::default()
                })
                .await,
            Err(EventError::InvalidEnumValue { .. })
        ));
        assert!(matches!(
            h.service
                .list_events(ListEventsQuery {
                    page_number: Some(0),
                    ..ListEventsQuery::default()
                })
                .await,
            Err(EventError::InvalidPaginationMinimum { .. })
        ));
    }
}

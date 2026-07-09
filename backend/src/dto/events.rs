use chrono::{DateTime, Utc};
use sea_orm::ActiveEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::events;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EventResponse {
    pub id: Uuid,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    /// 0 create, 1 update, 2 delete, 3 approve, 4 reject, 5 custom
    pub event_type: i16,
    /// 0 none, 1 pending, 2 approved, 3 rejected
    pub approval_status: i16,
    pub required_approval_count: Option<i16>,
    /// Users whose approve votes are all required (in addition to
    /// required_approval_count) before the event applies. Empty when no
    /// approvers are designated, and always empty for review and audit
    /// events. Listed users may review the event without holding the
    /// resource type's approval permission.
    pub required_approver_ids: Vec<Uuid>,
    /// Caller-defined kind name; only present when event_type is 5.
    pub custom_type: Option<String>,
    pub target_event_id: Option<Uuid>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<events::Model> for EventResponse {
    fn from(event: events::Model) -> Self {
        Self {
            id: event.id,
            resource_type: event.resource_type,
            resource_id: event.resource_id,
            actor_user_id: event.actor_user_id,
            event_type: event.event_type.into_value(),
            approval_status: event.approval_status.into_value(),
            required_approval_count: event.required_approval_count,
            required_approver_ids: event.required_approver_ids.0,
            custom_type: event.custom_type,
            target_event_id: event.target_event_id,
            old_value: event.old_value,
            new_value: event.new_value,
            remark: event.remark,
            created_at: event.created_at,
            updated_at: event.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ListEventsQuery {
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub resource_id: Option<Uuid>,
    #[serde(default)]
    pub event_type: Option<i16>,
    #[serde(default)]
    pub approval_status: Option<i16>,
    #[serde(default)]
    pub custom_type: Option<String>,
    #[serde(default)]
    pub target_event_id: Option<Uuid>,
    #[serde(default)]
    pub page_number: Option<u64>,
    #[serde(default)]
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListEventsResponse {
    pub events: Vec<EventResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReviewEventRequest {
    #[serde(default)]
    pub remark: Option<String>,
}

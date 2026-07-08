//! Integration surface for the audit/approval event system.
//!
//! A business table opts into reviewed mutations by implementing
//! [`ReviewableResource`] on its serde payload struct and registering it in
//! `main.rs` with [`ApplierRegistry::register`]. Handlers then call
//! `EventService::submit_create/submit_update/submit_delete` instead of
//! writing to the table directly; once enough approve events accumulate,
//! the stored JSON payload is deserialized back into the payload struct and
//! applied inside the review transaction.

use std::collections::HashMap;
use std::marker::PhantomData;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseTransaction;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("database operation failed during apply")]
    Database(#[from] sea_orm::DbErr),
    #[error("stored payload is not valid for this resource: {0}")]
    InvalidPayload(#[from] serde_json::Error),
    #[error("target row was not found during apply")]
    ResourceMissing,
}

/// Implemented by the serde payload struct for one business table. The
/// payload is what gets stored in the event's `new_value`/`old_value` JSONB
/// columns, so it should contain every field needed to (re)build a row.
#[async_trait]
pub trait ReviewableResource: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Stored as the event's `resource_type`; conventionally the table name.
    const RESOURCE_TYPE: &'static str;
    /// The `object:action` permission a reviewer must hold to approve or
    /// reject events of this resource type, e.g. `products:approve`. This
    /// is the single authoritative declaration for the type: it is
    /// validated when the type is registered (startup fails on a malformed
    /// string, like an invalid route permission) and resolved through the
    /// registry at review time, so pending events automatically follow the
    /// current declaration.
    const APPROVAL_PERMISSION: &'static str;

    /// Inserts a new row from this payload and returns the new row id.
    async fn apply_insert(
        self,
        tx: &DatabaseTransaction,
        now: DateTime<Utc>,
    ) -> Result<Uuid, ApplyError>;

    /// Overwrites the row identified by `resource_id` with this payload.
    async fn apply_update(
        self,
        tx: &DatabaseTransaction,
        resource_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError>;

    /// Deletes the row identified by `resource_id`.
    async fn apply_delete(
        tx: &DatabaseTransaction,
        resource_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError>;
}

/// Object-safe dispatch layer stored in the registry: takes raw JSON and
/// bridges to the typed [`ReviewableResource`] implementation.
#[async_trait]
pub trait ResourceApplier: Send + Sync {
    async fn apply_create(
        &self,
        tx: &DatabaseTransaction,
        new_value: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<Uuid, ApplyError>;

    async fn apply_update(
        &self,
        tx: &DatabaseTransaction,
        resource_id: Uuid,
        new_value: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError>;

    async fn apply_delete(
        &self,
        tx: &DatabaseTransaction,
        resource_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError>;
}

struct Applier<R: ReviewableResource>(PhantomData<fn() -> R>);

#[async_trait]
impl<R: ReviewableResource> ResourceApplier for Applier<R> {
    async fn apply_create(
        &self,
        tx: &DatabaseTransaction,
        new_value: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<Uuid, ApplyError> {
        let payload: R = serde_json::from_value(new_value)?;
        payload.apply_insert(tx, now).await
    }

    async fn apply_update(
        &self,
        tx: &DatabaseTransaction,
        resource_id: Uuid,
        new_value: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError> {
        let payload: R = serde_json::from_value(new_value)?;
        payload.apply_update(tx, resource_id, now).await
    }

    async fn apply_delete(
        &self,
        tx: &DatabaseTransaction,
        resource_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), ApplyError> {
        R::apply_delete(tx, resource_id, now).await
    }
}

/// Everything the registry knows about one registered resource type.
pub struct Registration {
    applier: Box<dyn ResourceApplier>,
    approval_object: &'static str,
    approval_action: &'static str,
}

impl Registration {
    pub fn applier(&self) -> &dyn ResourceApplier {
        self.applier.as_ref()
    }

    /// The parsed `(object, action)` a reviewer must hold.
    pub fn approval_permission(&self) -> (&'static str, &'static str) {
        (self.approval_object, self.approval_action)
    }
}

/// Maps `resource_type` strings to their appliers and approval
/// permissions. Built once at startup and shared through `EventService`;
/// submissions for unregistered types are rejected up front so events can
/// never get stuck unappliable.
#[derive(Default)]
pub struct ApplierRegistry {
    registrations: HashMap<&'static str, Registration>,
}

impl ApplierRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a resource type. Panics on a malformed
    /// `APPROVAL_PERMISSION`: registration runs at startup, so a bad
    /// declaration fails the boot (same failure mode as an invalid route
    /// permission in `require_permission`).
    pub fn register<R: ReviewableResource>(&mut self) {
        let (object, action) = crate::middleware::authz::parse_permission(R::APPROVAL_PERMISSION)
            .unwrap_or_else(|reason| {
                panic!(
                    "invalid APPROVAL_PERMISSION `{}` for resource type `{}`: {reason}",
                    R::APPROVAL_PERMISSION,
                    R::RESOURCE_TYPE
                )
            });

        self.registrations.insert(
            R::RESOURCE_TYPE,
            Registration {
                applier: Box::new(Applier::<R>(PhantomData)),
                approval_object: object,
                approval_action: action,
            },
        );
    }

    pub fn get(&self, resource_type: &str) -> Option<&Registration> {
        self.registrations.get(resource_type)
    }

    /// Iterates `(resource_type, object, action)` for every registered
    /// approval permission, so `main.rs` can merge them into the
    /// permission catalog alongside the builtin route permissions.
    pub fn approval_permissions(
        &self,
    ) -> impl Iterator<Item = (&'static str, &'static str, &'static str)> + '_ {
        self.registrations
            .iter()
            .map(|(resource_type, registration)| {
                (
                    *resource_type,
                    registration.approval_object,
                    registration.approval_action,
                )
            })
    }

    pub fn contains(&self, resource_type: &str) -> bool {
        self.registrations.contains_key(resource_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct BadPermissionResource;

    #[async_trait]
    impl ReviewableResource for BadPermissionResource {
        const RESOURCE_TYPE: &'static str = "bad_permission_resource";
        // Missing the `object:` prefix — must fail registration.
        const APPROVAL_PERMISSION: &'static str = "approve";

        async fn apply_insert(
            self,
            _tx: &DatabaseTransaction,
            _now: DateTime<Utc>,
        ) -> Result<Uuid, ApplyError> {
            unreachable!()
        }

        async fn apply_update(
            self,
            _tx: &DatabaseTransaction,
            _resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            unreachable!()
        }

        async fn apply_delete(
            _tx: &DatabaseTransaction,
            _resource_id: Uuid,
            _now: DateTime<Utc>,
        ) -> Result<(), ApplyError> {
            unreachable!()
        }
    }

    #[test]
    #[should_panic(expected = "invalid APPROVAL_PERMISSION")]
    fn registering_a_malformed_approval_permission_panics_at_startup() {
        let mut registry = ApplierRegistry::new();
        registry.register::<BadPermissionResource>();
    }
}

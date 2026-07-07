use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    entities::{permission_policies, role_inheritances, roles, user_roles, users},
    repositories::RepositoryError,
};

pub const SUBJECT_KIND_USER: &str = "user";
pub const SUBJECT_KIND_ROLE: &str = "role";
pub const EFFECT_ALLOW: &str = "allow";
pub const EFFECT_DENY: &str = "deny";

/// A policy row prepared for enforcer loading. `layer` is 0 for user
/// policies and the owning role's priority for role policies, so sorting
/// by `(layer, allow-after-deny, created_at, id)` yields the enforcement
/// precedence order expected by the implicit-priority casbin model.
#[derive(Debug, Clone)]
pub struct PolicyRow {
    pub id: Uuid,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub object: String,
    pub action: String,
    pub effect: String,
    pub layer: i32,
    pub created_at: DateTime<Utc>,
}

/// Everything needed to build a casbin enforcer in one consistent snapshot.
#[derive(Debug, Default)]
pub struct AuthzSnapshot {
    /// (user_id, role_id) assignments.
    pub user_roles: Vec<(Uuid, Uuid)>,
    /// (child_role_id, parent_role_id) inheritance links.
    pub role_inheritances: Vec<(Uuid, Uuid)>,
    /// Policies sorted by precedence (highest first).
    pub policies: Vec<PolicyRow>,
}

#[derive(Clone)]
pub struct AuthzRepository {
    pub(crate) db: DatabaseConnection,
}

impl AuthzRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn load_snapshot(&self) -> Result<AuthzSnapshot, RepositoryError> {
        let roles = roles::Entity::find().all(&self.db).await?;
        let role_priorities: std::collections::HashMap<Uuid, i32> =
            roles.iter().map(|r| (r.id, r.priority)).collect();

        let user_roles: Vec<(Uuid, Uuid)> = user_roles::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|link| (link.user_id, link.role_id))
            .collect();

        let role_inheritances: Vec<(Uuid, Uuid)> = role_inheritances::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|link| (link.child_role_id, link.parent_role_id))
            .collect();

        let mut policies = Vec::new();
        for policy in permission_policies::Entity::find().all(&self.db).await? {
            let layer = match policy.subject_kind.as_str() {
                SUBJECT_KIND_USER => 0,
                SUBJECT_KIND_ROLE => match role_priorities.get(&policy.subject_id) {
                    Some(priority) => *priority,
                    None => {
                        warn!(
                            policy_id = %policy.id,
                            role_id = %policy.subject_id,
                            "skipping policy whose role no longer exists"
                        );
                        continue;
                    }
                },
                other => {
                    warn!(
                        policy_id = %policy.id,
                        subject_kind = %other,
                        "skipping policy with unknown subject kind"
                    );
                    continue;
                }
            };

            policies.push(PolicyRow {
                id: policy.id,
                subject_kind: policy.subject_kind,
                subject_id: policy.subject_id,
                object: policy.object,
                action: policy.action,
                effect: policy.effect,
                layer,
                created_at: policy.created_at,
            });
        }

        // Precedence: lower layer first (user=0 wins over every role layer);
        // within a layer deny wins over allow; then stable order by creation.
        policies.sort_by(|a, b| {
            a.layer
                .cmp(&b.layer)
                .then_with(|| effect_rank(&a.effect).cmp(&effect_rank(&b.effect)))
                .then_with(|| a.created_at.cmp(&b.created_at))
                .then_with(|| a.id.cmp(&b.id))
        });

        debug!(
            policy_count = policies.len(),
            user_role_count = user_roles.len(),
            inheritance_count = role_inheritances.len(),
            "loaded authorization snapshot"
        );

        Ok(AuthzSnapshot {
            user_roles,
            role_inheritances,
            policies,
        })
    }

    // --- roles ---

    #[tracing::instrument(level = "info", skip(self), fields(code = %code, kind = %kind))]
    pub async fn create_role(
        &self,
        code: &str,
        name: &str,
        kind: &str,
        priority: i32,
        now: DateTime<Utc>,
    ) -> Result<roles::Model, RepositoryError> {
        let role = roles::ActiveModel {
            id: Set(Uuid::new_v4()),
            code: Set(code.to_string()),
            name: Set(name.to_string()),
            kind: Set(kind.to_string()),
            priority: Set(priority),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(role_id = %role.id, "created role");
        Ok(role)
    }

    #[tracing::instrument(level = "info", skip(self), fields(role_id = %id))]
    pub async fn update_role(
        &self,
        id: Uuid,
        name: Option<String>,
        priority: Option<i32>,
        now: DateTime<Utc>,
    ) -> Result<Option<roles::Model>, RepositoryError> {
        let Some(role) = roles::Entity::find_by_id(id).one(&self.db).await? else {
            return Ok(None);
        };

        let mut active: roles::ActiveModel = role.into();
        if let Some(name) = name {
            active.name = Set(name);
        }
        if let Some(priority) = priority {
            active.priority = Set(priority);
        }
        active.updated_at = Set(now);
        let role = active.update(&self.db).await?;

        info!("updated role");
        Ok(Some(role))
    }

    /// Deletes a role together with its policies, inheritance links, and
    /// user assignments in one transaction. `permission_policies` has no
    /// foreign key on `subject_id`, so its rows must be removed explicitly.
    #[tracing::instrument(level = "info", skip(self), fields(role_id = %id))]
    pub async fn delete_role(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let tx = self.db.begin().await?;

        let Some(role) = roles::Entity::find_by_id(id).one(&tx).await? else {
            tx.rollback().await?;
            return Ok(false);
        };

        permission_policies::Entity::delete_many()
            .filter(permission_policies::Column::SubjectKind.eq(SUBJECT_KIND_ROLE))
            .filter(permission_policies::Column::SubjectId.eq(id))
            .exec(&tx)
            .await?;
        role_inheritances::Entity::delete_many()
            .filter(
                role_inheritances::Column::ChildRoleId
                    .eq(id)
                    .or(role_inheritances::Column::ParentRoleId.eq(id)),
            )
            .exec(&tx)
            .await?;
        user_roles::Entity::delete_many()
            .filter(user_roles::Column::RoleId.eq(id))
            .exec(&tx)
            .await?;
        roles::Entity::delete_by_id(role.id).exec(&tx).await?;

        tx.commit().await?;
        info!("deleted role and its dependent rows");
        Ok(true)
    }

    #[tracing::instrument(level = "debug", skip(self), fields(role_id = %id))]
    pub async fn find_role(&self, id: Uuid) -> Result<Option<roles::Model>, RepositoryError> {
        Ok(roles::Entity::find_by_id(id).one(&self.db).await?)
    }

    #[tracing::instrument(level = "debug", skip(self), fields(code = %code))]
    pub async fn find_role_by_code(
        &self,
        code: &str,
    ) -> Result<Option<roles::Model>, RepositoryError> {
        Ok(roles::Entity::find()
            .filter(roles::Column::Code.eq(code))
            .one(&self.db)
            .await?)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_roles(&self) -> Result<Vec<roles::Model>, RepositoryError> {
        Ok(roles::Entity::find()
            .order_by_asc(roles::Column::Priority)
            .order_by_asc(roles::Column::Code)
            .all(&self.db)
            .await?)
    }

    #[tracing::instrument(level = "debug", skip(self, ids))]
    pub async fn count_roles_by_ids(&self, ids: &[Uuid]) -> Result<u64, RepositoryError> {
        if ids.is_empty() {
            return Ok(0);
        }
        use sea_orm::PaginatorTrait;
        Ok(roles::Entity::find()
            .filter(roles::Column::Id.is_in(ids.iter().copied()))
            .count(&self.db)
            .await?)
    }

    // --- role inheritance ---

    #[tracing::instrument(level = "debug", skip(self), fields(child_role_id = %child))]
    pub async fn list_role_parents(&self, child: Uuid) -> Result<Vec<Uuid>, RepositoryError> {
        Ok(role_inheritances::Entity::find()
            .filter(role_inheritances::Column::ChildRoleId.eq(child))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|link| link.parent_role_id)
            .collect())
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_all_inheritances(&self) -> Result<Vec<(Uuid, Uuid)>, RepositoryError> {
        Ok(role_inheritances::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|link| (link.child_role_id, link.parent_role_id))
            .collect())
    }

    /// Replaces the full parent set of a role in one transaction.
    #[tracing::instrument(level = "info", skip(self, parents), fields(child_role_id = %child, parent_count = parents.len()))]
    pub async fn set_role_parents(
        &self,
        child: Uuid,
        parents: &[Uuid],
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let tx = self.db.begin().await?;

        role_inheritances::Entity::delete_many()
            .filter(role_inheritances::Column::ChildRoleId.eq(child))
            .exec(&tx)
            .await?;

        for parent in parents {
            role_inheritances::ActiveModel {
                child_role_id: Set(child),
                parent_role_id: Set(*parent),
                created_at: Set(now),
            }
            .insert(&tx)
            .await?;
        }

        tx.commit().await?;
        info!("replaced role parents");
        Ok(())
    }

    // --- user role assignment ---

    #[tracing::instrument(level = "debug", skip(self), fields(user_id = %user_id))]
    pub async fn list_user_roles(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<roles::Model>, RepositoryError> {
        let role_ids: Vec<Uuid> = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|link| link.role_id)
            .collect();

        if role_ids.is_empty() {
            return Ok(Vec::new());
        }

        Ok(roles::Entity::find()
            .filter(roles::Column::Id.is_in(role_ids))
            .order_by_asc(roles::Column::Priority)
            .order_by_asc(roles::Column::Code)
            .all(&self.db)
            .await?)
    }

    /// Replaces the full role set of a user in one transaction.
    #[tracing::instrument(level = "info", skip(self, role_ids), fields(user_id = %user_id, role_count = role_ids.len()))]
    pub async fn set_user_roles(
        &self,
        user_id: Uuid,
        role_ids: &[Uuid],
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let tx = self.db.begin().await?;

        user_roles::Entity::delete_many()
            .filter(user_roles::Column::UserId.eq(user_id))
            .exec(&tx)
            .await?;

        for role_id in role_ids {
            user_roles::ActiveModel {
                user_id: Set(user_id),
                role_id: Set(*role_id),
                created_at: Set(now),
            }
            .insert(&tx)
            .await?;
        }

        tx.commit().await?;
        info!("replaced user roles");
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip(self), fields(user_id = %user_id))]
    pub async fn user_exists(&self, user_id: Uuid) -> Result<bool, RepositoryError> {
        Ok(users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .is_some())
    }

    // --- permission policies ---

    #[tracing::instrument(
        level = "info",
        skip(self),
        fields(subject_kind = %subject_kind, subject_id = %subject_id, object = %object, action = %action, effect = %effect)
    )]
    pub async fn create_policy(
        &self,
        subject_kind: &str,
        subject_id: Uuid,
        object: &str,
        action: &str,
        effect: &str,
        now: DateTime<Utc>,
    ) -> Result<permission_policies::Model, RepositoryError> {
        let policy = permission_policies::ActiveModel {
            id: Set(Uuid::new_v4()),
            subject_kind: Set(subject_kind.to_string()),
            subject_id: Set(subject_id),
            object: Set(object.to_string()),
            action: Set(action.to_string()),
            effect: Set(effect.to_string()),
            created_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        info!(policy_id = %policy.id, "created permission policy");
        Ok(policy)
    }

    #[tracing::instrument(level = "info", skip(self), fields(policy_id = %id))]
    pub async fn delete_policy(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let result = permission_policies::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        let deleted = result.rows_affected > 0;
        info!(deleted, "deleted permission policy");
        Ok(deleted)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn find_policy(
        &self,
        subject_kind: &str,
        subject_id: Uuid,
        object: &str,
        action: &str,
    ) -> Result<Option<permission_policies::Model>, RepositoryError> {
        Ok(permission_policies::Entity::find()
            .filter(permission_policies::Column::SubjectKind.eq(subject_kind))
            .filter(permission_policies::Column::SubjectId.eq(subject_id))
            .filter(permission_policies::Column::Object.eq(object))
            .filter(permission_policies::Column::Action.eq(action))
            .one(&self.db)
            .await?)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn list_policies(
        &self,
        subject_kind: Option<&str>,
        subject_id: Option<Uuid>,
    ) -> Result<Vec<permission_policies::Model>, RepositoryError> {
        let mut query = permission_policies::Entity::find();
        if let Some(kind) = subject_kind {
            query = query.filter(permission_policies::Column::SubjectKind.eq(kind));
        }
        if let Some(id) = subject_id {
            query = query.filter(permission_policies::Column::SubjectId.eq(id));
        }
        Ok(query
            .order_by_asc(permission_policies::Column::CreatedAt)
            .all(&self.db)
            .await?)
    }
}

fn effect_rank(effect: &str) -> u8 {
    if effect == EFFECT_DENY { 0 } else { 1 }
}

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use casbin::rhai::Dynamic;
use casbin::{
    CoreApi, DefaultModel, Enforcer, MemoryAdapter, MgmtApi,
    function_map::{OperatorFunction, dynamic_to_str},
};
use chrono::Utc;
use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    entities::{permission_policies, roles, users},
    repositories::{
        RepositoryError,
        authz::{
            AuthzRepository, EFFECT_ALLOW, EFFECT_DENY, NewPolicy, SUBJECT_KIND_ROLE,
            SUBJECT_KIND_USER,
        },
    },
    services::authz_catalog::PermissionCatalog,
};

/// Casbin model: implicit priority — the first policy row whose matcher
/// evaluates true decides the outcome, so the load order produced by
/// `AuthzRepository::load_snapshot` (user layer first, then roles by
/// ascending priority, deny before allow within a layer) implements the
/// layered-override semantics.
pub const CASBIN_MODEL: &str = r#"
[request_definition]
r = sub, obj, act

[policy_definition]
p = sub, obj, act, eft

[role_definition]
g = _, _

[policy_effect]
e = priority(p.eft) || deny

[matchers]
m = g(r.sub, p.sub) && scopeMatch(r.obj, p.obj) && (p.act == "*" || r.act == p.act)
"#;

pub const KIND_POSITION: &str = "position";
pub const KIND_DEPARTMENT: &str = "department";
pub const KIND_CUSTOM: &str = "custom";

pub const DEFAULT_PRIORITY_POSITION: i32 = 20;
pub const DEFAULT_PRIORITY_DEPARTMENT: i32 = 30;
pub const DEFAULT_PRIORITY_CUSTOM: i32 = 40;

pub const ACTIONS: [&str; 4] = ["read", "write", "post", "approve"];

/// casbin's DefaultRoleManager stops BFS at 10 hierarchy levels; keep
/// inheritance chains safely below that.
const MAX_INHERITANCE_DEPTH: usize = 8;

#[derive(Clone)]
pub struct AuthzService {
    repo: AuthzRepository,
    catalog: Arc<PermissionCatalog>,
    enforcer: Arc<tokio::sync::RwLock<Enforcer>>,
    super_admin_user_ids: Arc<tokio::sync::RwLock<HashSet<Uuid>>>,
    reload_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AuthzService {
    pub async fn new(repo: AuthzRepository) -> Result<Self, AuthzError> {
        Self::with_catalog(repo, PermissionCatalog::builtin()).await
    }

    /// Builds the service with an explicit catalog — used by `main.rs` to
    /// merge approval permissions from the review registry into the
    /// builtin route permissions.
    pub async fn with_catalog(
        repo: AuthzRepository,
        catalog: PermissionCatalog,
    ) -> Result<Self, AuthzError> {
        let authorization = build_authorization(&repo).await?;
        Ok(Self {
            repo,
            catalog: Arc::new(catalog),
            enforcer: Arc::new(tokio::sync::RwLock::new(authorization.enforcer)),
            super_admin_user_ids: Arc::new(tokio::sync::RwLock::new(
                authorization.super_admin_user_ids,
            )),
            reload_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn catalog(&self) -> &PermissionCatalog {
        &self.catalog
    }

    /// Checks whether the user may perform `action` on `object`.
    #[tracing::instrument(level = "debug", skip(self), fields(user_id = %user_id, object = %object, action = %action))]
    pub async fn check(
        &self,
        user_id: Uuid,
        object: &str,
        action: &str,
    ) -> Result<bool, AuthzError> {
        if self.is_super_admin(user_id).await {
            debug!("super admin permission granted");
            return Ok(true);
        }

        let subject = user_subject(user_id);
        let enforcer = self.enforcer.read().await;
        let allowed = enforcer.enforce((subject, object, action))?;
        debug!(allowed, "evaluated permission");
        Ok(allowed)
    }

    /// Evaluates every catalog permission for the user and returns the
    /// granted ones as concrete `object:action` strings, with wildcards,
    /// role inheritance, and layer precedence already resolved. This is
    /// what the frontend consumes for menu and button gating.
    #[tracing::instrument(level = "debug", skip(self), fields(user_id = %user_id))]
    pub async fn effective_permissions(&self, user_id: Uuid) -> Result<Vec<String>, AuthzError> {
        if self.is_super_admin(user_id).await {
            let permissions = self.all_catalog_permissions();
            debug!(
                granted = permissions.len(),
                "computed super admin effective permissions"
            );
            return Ok(permissions);
        }

        let subject = user_subject(user_id);
        let enforcer = self.enforcer.read().await;

        let mut permissions = Vec::new();
        for entry in self.catalog.entries() {
            for action in entry.actions() {
                if enforcer.enforce((subject.as_str(), entry.object(), action.as_str()))? {
                    permissions.push(format!("{}:{}", entry.object(), action));
                }
            }
        }
        debug!(
            granted = permissions.len(),
            "computed effective permissions"
        );
        Ok(permissions)
    }

    /// Rebuilds the enforcer from the database and swaps it in. Rebuilds
    /// are serialized so that racing mutations converge on a snapshot
    /// that includes every committed change.
    #[tracing::instrument(level = "info", skip(self))]
    pub async fn reload(&self) -> Result<(), AuthzError> {
        let _guard = self.reload_lock.lock().await;
        let authorization = build_authorization(&self.repo).await?;
        *self.enforcer.write().await = authorization.enforcer;
        *self.super_admin_user_ids.write().await = authorization.super_admin_user_ids;
        info!("reloaded authorization policies");
        Ok(())
    }

    async fn is_super_admin(&self, user_id: Uuid) -> bool {
        self.super_admin_user_ids.read().await.contains(&user_id)
    }

    fn all_catalog_permissions(&self) -> Vec<String> {
        self.catalog
            .entries()
            .iter()
            .flat_map(|entry| {
                entry
                    .actions()
                    .iter()
                    .map(|action| format!("{}:{}", entry.object(), action))
            })
            .collect()
    }

    // --- roles ---

    pub async fn create_role(
        &self,
        code: String,
        name: String,
        kind: String,
        priority: Option<i32>,
    ) -> Result<roles::Model, AuthzError> {
        let code = code.trim().to_string();
        let name = name.trim().to_string();
        if code.is_empty() {
            return Err(AuthzError::InvalidInput("code must not be empty"));
        }
        if name.is_empty() {
            return Err(AuthzError::InvalidInput("name must not be empty"));
        }
        let default_priority = match kind.as_str() {
            KIND_POSITION => DEFAULT_PRIORITY_POSITION,
            KIND_DEPARTMENT => DEFAULT_PRIORITY_DEPARTMENT,
            KIND_CUSTOM => DEFAULT_PRIORITY_CUSTOM,
            _ => return Err(AuthzError::InvalidRoleKind),
        };
        let priority = priority.unwrap_or(default_priority);
        if priority < 1 {
            return Err(AuthzError::InvalidInput(
                "priority must be >= 1 (0 is reserved for user-level policies)",
            ));
        }
        if self.repo.find_role_by_code(&code).await?.is_some() {
            return Err(AuthzError::DuplicateRoleCode);
        }

        let role = self
            .repo
            .create_role(&code, &name, &kind, priority, Utc::now())
            .await?;
        // A new role has no policies or members yet, so no reload is needed.
        Ok(role)
    }

    pub async fn update_role(
        &self,
        id: Uuid,
        name: Option<String>,
        priority: Option<i32>,
    ) -> Result<roles::Model, AuthzError> {
        self.ensure_role_is_not_protected(id).await?;
        if let Some(name) = &name
            && name.trim().is_empty()
        {
            return Err(AuthzError::InvalidInput("name must not be empty"));
        }
        if let Some(priority) = priority
            && priority < 1
        {
            return Err(AuthzError::InvalidInput(
                "priority must be >= 1 (0 is reserved for user-level policies)",
            ));
        }

        let role = self
            .repo
            .update_role(id, name.map(|n| n.trim().to_string()), priority, Utc::now())
            .await?
            .ok_or(AuthzError::RoleNotFound)?;
        self.reload().await?;
        Ok(role)
    }

    pub async fn delete_role(&self, id: Uuid) -> Result<(), AuthzError> {
        self.ensure_role_is_not_protected(id).await?;
        if !self.repo.delete_role(id).await? {
            return Err(AuthzError::RoleNotFound);
        }
        self.reload().await?;
        Ok(())
    }

    pub async fn get_role(&self, id: Uuid) -> Result<(roles::Model, Vec<Uuid>), AuthzError> {
        let role = self
            .repo
            .find_role(id)
            .await?
            .ok_or(AuthzError::RoleNotFound)?;
        let parents = self.repo.list_role_parents(id).await?;
        Ok((role, parents))
    }

    pub async fn list_roles(&self) -> Result<Vec<roles::Model>, AuthzError> {
        Ok(self.repo.list_roles().await?)
    }

    // --- role inheritance ---

    /// Replaces the parent set of `child`. Rejects unknown roles,
    /// self-inheritance, cycles, and chains deeper than
    /// `MAX_INHERITANCE_DEPTH`.
    pub async fn set_role_parents(
        &self,
        child: Uuid,
        parents: Vec<Uuid>,
    ) -> Result<(), AuthzError> {
        self.ensure_role_is_not_protected(child).await?;
        self.repo
            .find_role(child)
            .await?
            .ok_or(AuthzError::RoleNotFound)?;

        let unique: HashSet<Uuid> = parents.iter().copied().collect();
        if unique.len() != parents.len() {
            return Err(AuthzError::InvalidInput("duplicate parent role ids"));
        }
        if unique.contains(&child) {
            return Err(AuthzError::InheritanceCycle);
        }
        let found = self.repo.count_roles_by_ids(&parents).await?;
        if found != parents.len() as u64 {
            return Err(AuthzError::RoleNotFound);
        }

        // Validate the prospective graph (current links with child's
        // parent set replaced) before writing anything.
        let mut edges: Vec<(Uuid, Uuid)> = self
            .repo
            .list_all_inheritances()
            .await?
            .into_iter()
            .filter(|(c, _)| *c != child)
            .collect();
        edges.extend(parents.iter().map(|p| (child, *p)));

        let mut adjacency: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for (c, p) in &edges {
            adjacency.entry(*c).or_default().push(*p);
        }
        if has_cycle_from(child, &adjacency) {
            return Err(AuthzError::InheritanceCycle);
        }
        // The new edges may extend chains that pass *through* `child`
        // (descendants of `child` inherit through it), so check the
        // longest chain of the whole prospective graph, not just the
        // ancestors of `child`.
        if global_max_depth(&adjacency) > MAX_INHERITANCE_DEPTH {
            return Err(AuthzError::InheritanceTooDeep);
        }

        self.repo
            .set_role_parents(child, &parents, Utc::now())
            .await?;
        self.reload().await?;
        Ok(())
    }

    // --- user role assignment ---

    pub async fn list_user_roles(&self, user_id: Uuid) -> Result<Vec<roles::Model>, AuthzError> {
        if !self.repo.user_exists(user_id).await? {
            return Err(AuthzError::UserNotFound);
        }
        Ok(self.repo.list_user_roles(user_id).await?)
    }

    pub async fn list_role_users(&self, role_id: Uuid) -> Result<Vec<users::Model>, AuthzError> {
        if self.repo.find_role(role_id).await?.is_none() {
            return Err(AuthzError::RoleNotFound);
        }
        Ok(self.repo.list_role_users(role_id).await?)
    }

    /// Adds one user to a role. Idempotent: adding an existing member is a
    /// no-op, so concurrent membership edits can never clobber each other
    /// the way a whole-set replace can.
    pub async fn add_role_member(&self, role_id: Uuid, user_id: Uuid) -> Result<(), AuthzError> {
        if self.repo.find_role(role_id).await?.is_none() {
            return Err(AuthzError::RoleNotFound);
        }
        if !self.repo.user_exists(user_id).await? {
            return Err(AuthzError::UserNotFound);
        }
        if self.repo.user_has_role(user_id, role_id).await? {
            return Ok(());
        }
        self.repo
            .add_user_role(user_id, role_id, Utc::now())
            .await?;
        self.reload().await?;
        Ok(())
    }

    /// Removes one user from a role. Idempotent for non-members; removing a
    /// holder of the protected super_admin role is rejected.
    pub async fn remove_role_member(&self, role_id: Uuid, user_id: Uuid) -> Result<(), AuthzError> {
        if self.repo.find_role(role_id).await?.is_none() {
            return Err(AuthzError::RoleNotFound);
        }
        if !self.repo.user_exists(user_id).await? {
            return Err(AuthzError::UserNotFound);
        }
        if self
            .repo
            .super_admin_role()
            .await?
            .is_some_and(|role| role.id == role_id)
        {
            return Err(AuthzError::ProtectedSystemRole);
        }
        if !self.repo.user_has_role(user_id, role_id).await? {
            return Ok(());
        }
        self.repo.remove_user_role(user_id, role_id).await?;
        self.reload().await?;
        Ok(())
    }

    pub async fn set_user_roles(
        &self,
        user_id: Uuid,
        role_ids: Vec<Uuid>,
    ) -> Result<Vec<roles::Model>, AuthzError> {
        if !self.repo.user_exists(user_id).await? {
            return Err(AuthzError::UserNotFound);
        }
        let unique: HashSet<Uuid> = role_ids.iter().copied().collect();
        if unique.len() != role_ids.len() {
            return Err(AuthzError::InvalidInput("duplicate role ids"));
        }
        let found = self.repo.count_roles_by_ids(&role_ids).await?;
        if found != role_ids.len() as u64 {
            return Err(AuthzError::RoleNotFound);
        }
        self.ensure_super_admin_assignment_is_preserved(user_id, &role_ids)
            .await?;

        self.repo
            .set_user_roles(user_id, &role_ids, Utc::now())
            .await?;
        self.reload().await?;
        self.repo.list_user_roles(user_id).await.map_err(Into::into)
    }

    // --- permission policies ---

    pub async fn create_policy(
        &self,
        subject_kind: String,
        subject_id: Uuid,
        object: String,
        action: String,
        effect: String,
    ) -> Result<permission_policies::Model, AuthzError> {
        self.ensure_subject_exists(&subject_kind, subject_id)
            .await?;
        self.ensure_policy_subject_is_not_protected(&subject_kind, subject_id)
            .await?;
        self.validate_policy_content(&object, &action, &effect)?;
        if self
            .repo
            .find_policy(&subject_kind, subject_id, &object, &action)
            .await?
            .is_some()
        {
            return Err(AuthzError::DuplicatePolicy);
        }

        let policy = self
            .repo
            .create_policy(
                &subject_kind,
                subject_id,
                &object,
                &action,
                &effect,
                Utc::now(),
            )
            .await?;
        self.reload().await?;
        Ok(policy)
    }

    pub async fn delete_policy(&self, id: Uuid) -> Result<(), AuthzError> {
        let policy = self
            .repo
            .find_policy_by_id(id)
            .await?
            .ok_or(AuthzError::PolicyNotFound)?;
        self.ensure_policy_subject_is_not_protected(&policy.subject_kind, policy.subject_id)
            .await?;
        if !self.repo.delete_policy(id).await? {
            return Err(AuthzError::PolicyNotFound);
        }
        self.reload().await?;
        Ok(())
    }

    /// Replaces the full policy set of one subject in a single transaction
    /// followed by exactly one enforcer reload — the bulk operation behind
    /// the permission panel's "edit matrix, then save" flow.
    pub async fn replace_subject_policies(
        &self,
        subject_kind: String,
        subject_id: Uuid,
        policies: Vec<NewPolicy>,
    ) -> Result<Vec<permission_policies::Model>, AuthzError> {
        self.ensure_subject_exists(&subject_kind, subject_id)
            .await?;
        self.ensure_policy_subject_is_not_protected(&subject_kind, subject_id)
            .await?;
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        for policy in &policies {
            self.validate_policy_content(&policy.object, &policy.action, &policy.effect)?;
            if !seen.insert((policy.object.as_str(), policy.action.as_str())) {
                return Err(AuthzError::InvalidInput(
                    "policies must not repeat the same object and action",
                ));
            }
        }

        let created = self
            .repo
            .replace_subject_policies(&subject_kind, subject_id, &policies, Utc::now())
            .await?;
        self.reload().await?;
        Ok(created)
    }

    async fn ensure_subject_exists(
        &self,
        subject_kind: &str,
        subject_id: Uuid,
    ) -> Result<(), AuthzError> {
        match subject_kind {
            SUBJECT_KIND_USER => {
                if !self.repo.user_exists(subject_id).await? {
                    return Err(AuthzError::UserNotFound);
                }
            }
            SUBJECT_KIND_ROLE => {
                if self.repo.find_role(subject_id).await?.is_none() {
                    return Err(AuthzError::RoleNotFound);
                }
            }
            _ => return Err(AuthzError::InvalidSubjectKind),
        }
        Ok(())
    }

    async fn ensure_role_is_not_protected(&self, role_id: Uuid) -> Result<(), AuthzError> {
        if self
            .repo
            .super_admin_role()
            .await?
            .is_some_and(|role| role.id == role_id)
        {
            return Err(AuthzError::ProtectedSystemRole);
        }
        Ok(())
    }

    async fn ensure_policy_subject_is_not_protected(
        &self,
        subject_kind: &str,
        subject_id: Uuid,
    ) -> Result<(), AuthzError> {
        if subject_kind == SUBJECT_KIND_ROLE
            && self
                .repo
                .super_admin_role()
                .await?
                .is_some_and(|role| role.id == subject_id)
        {
            return Err(AuthzError::ProtectedSystemRole);
        }
        Ok(())
    }

    async fn ensure_super_admin_assignment_is_preserved(
        &self,
        user_id: Uuid,
        requested_role_ids: &[Uuid],
    ) -> Result<(), AuthzError> {
        let Some(role) = self.repo.super_admin_role().await? else {
            return Ok(());
        };
        if self.repo.user_has_role(user_id, role.id).await?
            && !requested_role_ids.contains(&role.id)
        {
            return Err(AuthzError::ProtectedSystemRole);
        }
        Ok(())
    }

    /// Shared validation for single and bulk policy writes. The object
    /// must be covered by the catalog (exactly, or as a wildcard pattern
    /// matching at least one catalog object) so a typo can never create a
    /// policy that silently grants nothing.
    fn validate_policy_content(
        &self,
        object: &str,
        action: &str,
        effect: &str,
    ) -> Result<(), AuthzError> {
        if effect != EFFECT_ALLOW && effect != EFFECT_DENY {
            return Err(AuthzError::InvalidEffect);
        }
        validate_policy_object(object)?;
        if action != "*" && !ACTIONS.contains(&action) {
            return Err(AuthzError::InvalidInput(
                "action must be one of read/write/post/approve or *",
            ));
        }
        let covers_catalog_permission = self.catalog.entries().iter().any(|entry| {
            scope_match(entry.object(), object)
                && (action == "*" || entry.actions().iter().any(|a| a == action))
        });
        if !covers_catalog_permission {
            return Err(AuthzError::PermissionNotInCatalog);
        }
        Ok(())
    }

    pub async fn list_policies(
        &self,
        subject_kind: Option<String>,
        subject_id: Option<Uuid>,
    ) -> Result<Vec<permission_policies::Model>, AuthzError> {
        if let Some(kind) = &subject_kind
            && kind != SUBJECT_KIND_USER
            && kind != SUBJECT_KIND_ROLE
        {
            return Err(AuthzError::InvalidSubjectKind);
        }
        Ok(self
            .repo
            .list_policies(subject_kind.as_deref(), subject_id)
            .await?)
    }
}

struct AuthorizationBuild {
    enforcer: Enforcer,
    super_admin_user_ids: HashSet<Uuid>,
}

async fn build_authorization(repo: &AuthzRepository) -> Result<AuthorizationBuild, AuthzError> {
    let model = DefaultModel::from_str(CASBIN_MODEL).await?;
    let mut enforcer = Enforcer::new(model, MemoryAdapter::default()).await?;
    enforcer.add_function("scopeMatch", OperatorFunction::Arg2(scope_match_dynamic));

    let snapshot = repo.load_snapshot().await?;

    let grouping: Vec<Vec<String>> = snapshot
        .user_roles
        .iter()
        .map(|(user_id, role_id)| vec![user_subject(*user_id), role_subject(*role_id)])
        .chain(
            snapshot
                .role_inheritances
                .iter()
                .map(|(child, parent)| vec![role_subject(*child), role_subject(*parent)]),
        )
        .collect();

    let policies: Vec<Vec<String>> = snapshot
        .policies
        .iter()
        .map(|row| {
            let subject = match row.subject_kind.as_str() {
                SUBJECT_KIND_USER => user_subject(row.subject_id),
                _ => role_subject(row.subject_id),
            };
            vec![
                subject,
                row.object.clone(),
                row.action.clone(),
                row.effect.clone(),
            ]
        })
        .collect();

    // add_policies is all-or-nothing: a `false` return means the whole
    // batch was rejected (duplicate rule) and the enforcer would run
    // with an empty policy set — treat that as an internal error.
    if !grouping.is_empty() && !enforcer.add_grouping_policies(grouping).await? {
        return Err(AuthzError::PolicyLoadConflict);
    }
    if !policies.is_empty() && !enforcer.add_policies(policies).await? {
        return Err(AuthzError::PolicyLoadConflict);
    }

    Ok(AuthorizationBuild {
        enforcer,
        super_admin_user_ids: snapshot.super_admin_user_ids,
    })
}

fn user_subject(user_id: Uuid) -> String {
    format!("user:{user_id}")
}

fn role_subject(role_id: Uuid) -> String {
    format!("role:{role_id}")
}

/// rhai bridge for `scope_match`; OperatorFunction only accepts plain fn
/// pointers, so this cannot capture anything.
fn scope_match_dynamic(request: Dynamic, policy: Dynamic) -> Dynamic {
    scope_match(&dynamic_to_str(&request), &dynamic_to_str(&policy)).into()
}

/// Segment-wise match of a `scope1:scope2:...` object path. In the policy
/// pattern, `*` matches exactly one segment, and a trailing `*` matches
/// one or more remaining segments; otherwise segment counts must match.
fn scope_match(request: &str, policy: &str) -> bool {
    let request_segments: Vec<&str> = request.split(':').collect();
    let policy_segments: Vec<&str> = policy.split(':').collect();

    for (index, policy_segment) in policy_segments.iter().enumerate() {
        let is_last = index == policy_segments.len() - 1;
        if *policy_segment == "*" && is_last {
            // Trailing wildcard: matches all remaining segments (>= 1).
            return request_segments.len() > index;
        }
        let Some(request_segment) = request_segments.get(index) else {
            return false;
        };
        if *policy_segment != "*" && policy_segment != request_segment {
            return false;
        }
    }

    request_segments.len() == policy_segments.len()
}

fn validate_policy_object(object: &str) -> Result<(), AuthzError> {
    if object.is_empty() {
        return Err(AuthzError::InvalidInput("object must not be empty"));
    }
    if object.split(':').any(|segment| segment.is_empty()) {
        return Err(AuthzError::InvalidInput(
            "object must not contain empty segments",
        ));
    }
    Ok(())
}

/// DFS from `start` looking for a path back to `start`.
fn has_cycle_from(start: Uuid, adjacency: &HashMap<Uuid, Vec<Uuid>>) -> bool {
    let mut stack: Vec<Uuid> = adjacency.get(&start).cloned().unwrap_or_default();
    let mut visited: HashSet<Uuid> = HashSet::new();
    while let Some(node) = stack.pop() {
        if node == start {
            return true;
        }
        if visited.insert(node)
            && let Some(parents) = adjacency.get(&node)
        {
            stack.extend(parents.iter().copied());
        }
    }
    false
}

/// Longest path length (in edges) reachable from `start`. Only valid on
/// acyclic graphs (cycle check runs first).
fn max_depth_from(
    start: Uuid,
    adjacency: &HashMap<Uuid, Vec<Uuid>>,
    memo: &mut HashMap<Uuid, usize>,
) -> usize {
    if let Some(cached) = memo.get(&start) {
        return *cached;
    }
    let result = adjacency
        .get(&start)
        .map(|parents| {
            parents
                .iter()
                .map(|parent| 1 + max_depth_from(*parent, adjacency, memo))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    memo.insert(start, result);
    result
}

/// Longest inheritance chain (in edges) anywhere in the graph.
fn global_max_depth(adjacency: &HashMap<Uuid, Vec<Uuid>>) -> usize {
    let mut memo = HashMap::new();
    adjacency
        .keys()
        .map(|node| max_depth_from(*node, adjacency, &mut memo))
        .max()
        .unwrap_or(0)
}

#[derive(Debug, Error)]
pub enum AuthzError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("authorization engine error")]
    Casbin(#[from] casbin::Error),
    #[error("role not found")]
    RoleNotFound,
    #[error("user not found")]
    UserNotFound,
    #[error("policy not found")]
    PolicyNotFound,
    #[error("role code already exists")]
    DuplicateRoleCode,
    #[error("an identical policy already exists")]
    DuplicatePolicy,
    #[error("object/action does not cover any known permission")]
    PermissionNotInCatalog,
    #[error("kind must be one of position/department/custom")]
    InvalidRoleKind,
    #[error("subject_kind must be user or role")]
    InvalidSubjectKind,
    #[error("effect must be allow or deny")]
    InvalidEffect,
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error("role inheritance must not contain cycles")]
    InheritanceCycle,
    #[error("role inheritance chain is too deep (max {MAX_INHERITANCE_DEPTH})")]
    InheritanceTooDeep,
    #[error("failed to load policies into the authorization engine")]
    PolicyLoadConflict,
    #[error("request is missing an authenticated session")]
    MissingSession,
    #[error("protected system role cannot be modified")]
    ProtectedSystemRole,
}

impl AuthzError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(error) => match error {
                RepositoryError::DisabledUser => "user_disabled",
                RepositoryError::MissingRequiredField { .. } => "validation_error",
                RepositoryError::Database(_) => "database_error",
            },
            Self::Casbin(_) => "authorization_engine_error",
            Self::RoleNotFound => "role_not_found",
            Self::UserNotFound => "user_not_found",
            Self::PolicyNotFound => "policy_not_found",
            Self::DuplicateRoleCode => "duplicate_role_code",
            Self::DuplicatePolicy => "duplicate_policy",
            Self::PermissionNotInCatalog => "permission_not_in_catalog",
            Self::InvalidRoleKind => "invalid_role_kind",
            Self::InvalidSubjectKind => "invalid_subject_kind",
            Self::InvalidEffect => "invalid_effect",
            Self::InvalidInput(_) => "validation_error",
            Self::InheritanceCycle => "inheritance_cycle",
            Self::InheritanceTooDeep => "inheritance_too_deep",
            Self::PolicyLoadConflict => "policy_load_conflict",
            Self::MissingSession => "missing_session",
            Self::ProtectedSystemRole => "protected_system_role",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- scope_match pure function ---

    #[test]
    fn scope_match_exact() {
        assert!(scope_match("finance:invoices", "finance:invoices"));
        assert!(!scope_match("finance:invoices", "finance:reports"));
    }

    #[test]
    fn scope_match_single_segment_wildcard() {
        assert!(scope_match("finance:invoices", "finance:*"));
        assert!(scope_match("finance:invoices:q3", "finance:*:q3"));
        assert!(!scope_match("hr:invoices:q3", "finance:*:q3"));
    }

    #[test]
    fn scope_match_trailing_wildcard_matches_remaining() {
        assert!(scope_match("finance:invoices:q3", "finance:*"));
        assert!(scope_match("finance:invoices:q3:extra", "finance:*"));
        assert!(scope_match("a:b", "*"));
        assert!(scope_match("a", "*"));
    }

    #[test]
    fn scope_match_length_mismatch() {
        assert!(!scope_match("finance", "finance:invoices"));
        assert!(!scope_match("finance:invoices:q3", "finance:invoices"));
        // Trailing wildcard requires at least one remaining segment.
        assert!(!scope_match("finance", "finance:*"));
    }

    #[test]
    fn scope_match_wildcard_in_middle_is_single_segment() {
        assert!(!scope_match("finance:a:b:q3", "finance:*:q3"));
    }

    // --- validate_policy_object ---

    #[test]
    fn policy_object_rejects_empty_segments() {
        assert!(validate_policy_object("finance:invoices").is_ok());
        assert!(validate_policy_object("*").is_ok());
        assert!(validate_policy_object("").is_err());
        assert!(validate_policy_object("finance::x").is_err());
        assert!(validate_policy_object(":finance").is_err());
    }

    // --- cycle / depth helpers ---

    #[test]
    fn detects_direct_and_indirect_cycles() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let mut adjacency = HashMap::new();
        adjacency.insert(a, vec![b]);
        adjacency.insert(b, vec![c]);
        adjacency.insert(c, vec![a]);
        assert!(has_cycle_from(a, &adjacency));

        let mut acyclic = HashMap::new();
        acyclic.insert(a, vec![b]);
        acyclic.insert(b, vec![c]);
        assert!(!has_cycle_from(a, &acyclic));
    }

    #[test]
    fn computes_max_depth() {
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let mut adjacency = HashMap::new();
        adjacency.insert(ids[0], vec![ids[1]]);
        adjacency.insert(ids[1], vec![ids[2]]);
        adjacency.insert(ids[2], vec![ids[3]]);
        assert_eq!(max_depth_from(ids[0], &adjacency, &mut HashMap::new()), 3);
        assert_eq!(max_depth_from(ids[3], &adjacency, &mut HashMap::new()), 0);
        assert_eq!(global_max_depth(&adjacency), 3);
    }

    // --- DB-backed enforcement tests ---

    mod enforcement {
        use super::*;
        use crate::{
            config::{DatabaseConfig, DatabaseKind},
            db,
            repositories::users::UserRepository,
        };
        use std::path::PathBuf;

        fn sqlite_memory_config() -> DatabaseConfig {
            DatabaseConfig {
                kind: DatabaseKind::SqliteMemory,
                url: "postgres://unused".to_string(),
                sqlite_file: PathBuf::from("unused.sqlite"),
            }
        }

        struct Harness {
            authz: AuthzService,
            users: UserRepository,
        }

        impl Harness {
            async fn new() -> Self {
                let db = db::connect_and_migrate(&sqlite_memory_config())
                    .await
                    .expect("sqlite memory database should initialize");
                let users = UserRepository::new(db.clone());
                // Policy writes validate objects against the catalog, so
                // the fixture objects used below must be registered — the
                // same way main.rs merges approval permissions in.
                let mut catalog = PermissionCatalog::builtin();
                for (object, action) in [
                    ("finance:invoices", "read"),
                    ("finance:invoices", "write"),
                    ("finance:reports", "approve"),
                    ("finance:ledger", "read"),
                    ("finance:x", "read"),
                    ("hr:staff", "read"),
                ] {
                    catalog.add_permission(object, action, "test", object);
                }
                let authz = AuthzService::with_catalog(AuthzRepository::new(db), catalog)
                    .await
                    .expect("authz service should initialize");
                Self { authz, users }
            }

            async fn user(&self, dingtalk_id: &str) -> Uuid {
                self.users
                    .find_or_create_for_login(dingtalk_id, Utc::now())
                    .await
                    .expect("user should be created")
                    .id
            }

            async fn role(&self, code: &str, kind: &str, priority: Option<i32>) -> Uuid {
                self.authz
                    .create_role(
                        code.to_string(),
                        code.to_string(),
                        kind.to_string(),
                        priority,
                    )
                    .await
                    .expect("role should be created")
                    .id
            }

            async fn assign(&self, user_id: Uuid, role_ids: Vec<Uuid>) {
                self.authz
                    .set_user_roles(user_id, role_ids)
                    .await
                    .expect("roles should be assigned");
            }

            async fn policy(
                &self,
                kind: &str,
                subject: Uuid,
                object: &str,
                action: &str,
                effect: &str,
            ) {
                self.authz
                    .create_policy(
                        kind.to_string(),
                        subject,
                        object.to_string(),
                        action.to_string(),
                        effect.to_string(),
                    )
                    .await
                    .expect("policy should be created");
            }

            async fn check(&self, user_id: Uuid, object: &str, action: &str) -> bool {
                self.authz
                    .check(user_id, object, action)
                    .await
                    .expect("check should not fail")
            }
        }

        #[tokio::test]
        async fn default_deny_without_policies() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            assert!(!h.check(user, "finance:invoices", "read").await);
        }

        #[tokio::test]
        async fn user_allow_overrides_position_deny() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let position = h.role("accountant", KIND_POSITION, None).await;
            h.assign(user, vec![position]).await;

            h.policy("role", position, "finance:invoices", "write", "deny")
                .await;
            h.policy("user", user, "finance:invoices", "write", "allow")
                .await;

            assert!(h.check(user, "finance:invoices", "write").await);
        }

        #[tokio::test]
        async fn user_deny_overrides_role_allow() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let position = h.role("accountant", KIND_POSITION, None).await;
            h.assign(user, vec![position]).await;

            h.policy("role", position, "finance:invoices", "write", "allow")
                .await;
            h.policy("user", user, "finance:invoices", "write", "deny")
                .await;

            assert!(!h.check(user, "finance:invoices", "write").await);
        }

        #[tokio::test]
        async fn position_allow_overrides_department_deny() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let position = h.role("cfo", KIND_POSITION, None).await;
            let department = h.role("finance-dept", KIND_DEPARTMENT, None).await;
            h.assign(user, vec![position, department]).await;

            h.policy("role", department, "finance:reports", "approve", "deny")
                .await;
            h.policy("role", position, "finance:reports", "approve", "allow")
                .await;

            assert!(h.check(user, "finance:reports", "approve").await);
        }

        #[tokio::test]
        async fn deny_wins_within_same_layer() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let dept_a = h.role("dept-a", KIND_DEPARTMENT, None).await;
            let dept_b = h.role("dept-b", KIND_DEPARTMENT, None).await;
            h.assign(user, vec![dept_a, dept_b]).await;

            // allow created first, deny second — deny must still win.
            h.policy("role", dept_a, "finance:invoices", "read", "allow")
                .await;
            h.policy("role", dept_b, "finance:invoices", "read", "deny")
                .await;

            assert!(!h.check(user, "finance:invoices", "read").await);
        }

        #[tokio::test]
        async fn parent_department_policy_reaches_child_department_members() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let parent = h.role("dept-root", KIND_DEPARTMENT, None).await;
            let child = h.role("dept-sub", KIND_DEPARTMENT, None).await;
            h.authz
                .set_role_parents(child, vec![parent])
                .await
                .expect("inheritance should be set");
            h.assign(user, vec![child]).await;

            h.policy("role", parent, "hr:staff", "read", "allow").await;

            assert!(h.check(user, "hr:staff", "read").await);
        }

        #[tokio::test]
        async fn wildcard_object_and_action_match() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;

            h.policy("user", user, "system:*", "*", "allow").await;

            assert!(h.check(user, "system:permissions", "read").await);
            assert!(h.check(user, "system:permissions:policies", "write").await);
            assert!(!h.check(user, "finance:invoices", "read").await);
        }

        #[tokio::test]
        async fn custom_role_loses_to_department() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let custom = h.role("auditors", KIND_CUSTOM, None).await;
            let department = h.role("finance-dept", KIND_DEPARTMENT, None).await;
            h.assign(user, vec![custom, department]).await;

            h.policy("role", custom, "finance:ledger", "read", "allow")
                .await;
            h.policy("role", department, "finance:ledger", "read", "deny")
                .await;

            assert!(!h.check(user, "finance:ledger", "read").await);
        }

        #[tokio::test]
        async fn policy_mutations_take_effect_immediately() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;

            assert!(!h.check(user, "finance:invoices", "read").await);

            h.policy("user", user, "finance:invoices", "read", "allow")
                .await;
            assert!(h.check(user, "finance:invoices", "read").await);

            let policies = h
                .authz
                .list_policies(Some("user".to_string()), Some(user))
                .await
                .expect("policies should list");
            h.authz
                .delete_policy(policies[0].id)
                .await
                .expect("policy should delete");
            assert!(!h.check(user, "finance:invoices", "read").await);
        }

        #[tokio::test]
        async fn deleting_role_revokes_access() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let role = h.role("temp-role", KIND_CUSTOM, None).await;
            h.assign(user, vec![role]).await;
            h.policy("role", role, "finance:invoices", "read", "allow")
                .await;
            assert!(h.check(user, "finance:invoices", "read").await);

            h.authz.delete_role(role).await.expect("role should delete");
            assert!(!h.check(user, "finance:invoices", "read").await);
        }

        #[tokio::test]
        async fn list_role_users_returns_members_and_rejects_unknown_role() {
            let h = Harness::new().await;
            let member = h.user("ding-1").await;
            let outsider = h.user("ding-2").await;
            let role = h.role("with-members", KIND_CUSTOM, None).await;
            let empty_role = h.role("no-members", KIND_CUSTOM, None).await;
            h.assign(member, vec![role]).await;

            let members = h
                .authz
                .list_role_users(role)
                .await
                .expect("members should list");
            assert_eq!(
                members.iter().map(|u| u.id).collect::<Vec<_>>(),
                vec![member]
            );
            assert!(!members.iter().any(|u| u.id == outsider));

            let none = h
                .authz
                .list_role_users(empty_role)
                .await
                .expect("member-less role should list");
            assert!(none.is_empty());

            let missing = h.authz.list_role_users(Uuid::new_v4()).await;
            assert!(matches!(missing, Err(AuthzError::RoleNotFound)));
        }

        #[tokio::test]
        async fn add_and_remove_role_member_are_atomic_and_idempotent() {
            let h = Harness::new().await;
            let member = h.user("ding-1").await;
            let role_a = h.role("role-a", KIND_CUSTOM, None).await;
            let role_b = h.role("role-b", KIND_CUSTOM, None).await;
            h.assign(member, vec![role_b]).await;

            h.authz
                .add_role_member(role_a, member)
                .await
                .expect("member should be added");
            // Adding one role must not disturb the user's other roles.
            let roles: Vec<Uuid> = h
                .authz
                .list_user_roles(member)
                .await
                .expect("roles should list")
                .into_iter()
                .map(|r| r.id)
                .collect();
            assert!(roles.contains(&role_a) && roles.contains(&role_b));

            // Idempotent: re-adding is a no-op, not a unique-constraint error.
            h.authz
                .add_role_member(role_a, member)
                .await
                .expect("re-adding an existing member should be a no-op");

            h.authz
                .remove_role_member(role_a, member)
                .await
                .expect("member should be removed");
            h.authz
                .remove_role_member(role_a, member)
                .await
                .expect("removing a non-member should be a no-op");
            let remaining: Vec<Uuid> = h
                .authz
                .list_user_roles(member)
                .await
                .expect("roles should list")
                .into_iter()
                .map(|r| r.id)
                .collect();
            assert_eq!(remaining, vec![role_b]);

            // The enforcer reloads on membership changes: role-a grants a
            // permission that must appear/disappear with membership.
            h.policy("role", role_a, "finance:invoices", "read", "allow")
                .await;
            assert!(!h.check(member, "finance:invoices", "read").await);
            h.authz
                .add_role_member(role_a, member)
                .await
                .expect("member should be added");
            assert!(h.check(member, "finance:invoices", "read").await);
            h.authz
                .remove_role_member(role_a, member)
                .await
                .expect("member should be removed");
            assert!(!h.check(member, "finance:invoices", "read").await);

            // Validation and protection paths.
            assert!(matches!(
                h.authz.add_role_member(Uuid::new_v4(), member).await,
                Err(AuthzError::RoleNotFound)
            ));
            assert!(matches!(
                h.authz.add_role_member(role_a, Uuid::new_v4()).await,
                Err(AuthzError::UserNotFound)
            ));
            let super_admin = h
                .authz
                .repo
                .super_admin_role()
                .await
                .expect("role lookup should succeed")
                .expect("super admin role should be seeded");
            h.assign(member, vec![role_b, super_admin.id]).await;
            assert!(matches!(
                h.authz.remove_role_member(super_admin.id, member).await,
                Err(AuthzError::ProtectedSystemRole)
            ));
        }

        #[tokio::test]
        async fn rejects_inheritance_cycles_and_depth() {
            let h = Harness::new().await;
            let a = h.role("dept-a", KIND_DEPARTMENT, None).await;
            let b = h.role("dept-b", KIND_DEPARTMENT, None).await;
            h.authz
                .set_role_parents(a, vec![b])
                .await
                .expect("a -> b should be allowed");

            let result = h.authz.set_role_parents(b, vec![a]).await;
            assert!(matches!(result, Err(AuthzError::InheritanceCycle)));

            let self_cycle = h.authz.set_role_parents(a, vec![a]).await;
            assert!(matches!(self_cycle, Err(AuthzError::InheritanceCycle)));

            // Build a chain of 9 links: too deep once it exceeds 8.
            let mut chain = vec![a, b];
            for index in 0..8 {
                let role = h
                    .role(&format!("dept-{index}"), KIND_DEPARTMENT, None)
                    .await;
                chain.push(role);
            }
            for pair in chain.windows(2).skip(1) {
                let result = h.authz.set_role_parents(pair[0], vec![pair[1]]).await;
                if pair[1] == *chain.last().unwrap() {
                    assert!(matches!(result, Err(AuthzError::InheritanceTooDeep)));
                } else {
                    result.expect("chain within depth limit should be allowed");
                }
            }
        }

        #[tokio::test]
        async fn effective_permissions_resolve_wildcards_and_layers() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let role = h.role("accountant", KIND_POSITION, None).await;
            h.assign(user, vec![role]).await;

            // Wildcard object and action on the role layer...
            h.policy("role", role, "finance:*", "*", "allow").await;
            // ...with one user-layer deny carved out.
            h.policy("user", user, "finance:invoices", "write", "deny")
                .await;

            let permissions = h
                .authz
                .effective_permissions(user)
                .await
                .expect("effective permissions should compute");

            assert!(permissions.contains(&"finance:invoices:read".to_string()));
            assert!(permissions.contains(&"finance:reports:approve".to_string()));
            // The user-layer deny wins over the role wildcard allow.
            assert!(!permissions.contains(&"finance:invoices:write".to_string()));
            // Nothing outside the granted subtree leaks in.
            assert!(!permissions.contains(&"products:read".to_string()));

            // Every returned permission is enumerable from the catalog.
            for permission in &permissions {
                let (object, action) = permission
                    .rsplit_once(':')
                    .expect("permission should be object:action");
                assert!(h.authz.catalog().contains(object, action));
            }
        }

        #[tokio::test]
        async fn effective_permissions_are_empty_without_policies() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let permissions = h
                .authz
                .effective_permissions(user)
                .await
                .expect("effective permissions should compute");
            assert!(permissions.is_empty());
        }

        #[tokio::test]
        async fn super_admin_bypasses_denies_and_gets_full_catalog() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let super_admin = h
                .authz
                .repo
                .super_admin_role()
                .await
                .expect("role lookup should succeed")
                .expect("super admin role should be seeded");
            h.assign(user, vec![super_admin.id]).await;
            h.policy("user", user, "products", "read", "deny").await;

            assert!(h.check(user, "products", "read").await);
            assert!(h.check(user, "finance:ledger", "read").await);

            let permissions = h
                .authz
                .effective_permissions(user)
                .await
                .expect("effective permissions should compute");
            let catalog_permission_count: usize = h
                .authz
                .catalog()
                .entries()
                .iter()
                .map(|entry| entry.actions().len())
                .sum();
            assert_eq!(permissions.len(), catalog_permission_count);
            assert!(permissions.contains(&"products:read".to_string()));
            assert!(permissions.contains(&"system:permissions:write".to_string()));
            assert!(permissions.contains(&"finance:ledger:read".to_string()));
        }

        #[tokio::test]
        async fn protected_super_admin_role_cannot_be_mutated() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let super_admin = h
                .authz
                .repo
                .super_admin_role()
                .await
                .expect("role lookup should succeed")
                .expect("super admin role should be seeded");
            h.assign(user, vec![super_admin.id]).await;
            let policy = h
                .authz
                .list_policies(Some("role".to_string()), Some(super_admin.id))
                .await
                .expect("policies should list")
                .into_iter()
                .find(|policy| policy.object == "*" && policy.action == "*")
                .expect("super admin wildcard policy should exist");

            assert!(matches!(
                h.authz
                    .update_role(super_admin.id, Some("x".into()), None)
                    .await,
                Err(AuthzError::ProtectedSystemRole)
            ));
            assert!(matches!(
                h.authz.delete_role(super_admin.id).await,
                Err(AuthzError::ProtectedSystemRole)
            ));
            assert!(matches!(
                h.authz.set_role_parents(super_admin.id, vec![]).await,
                Err(AuthzError::ProtectedSystemRole)
            ));
            assert!(matches!(
                h.authz
                    .create_policy(
                        "role".into(),
                        super_admin.id,
                        "products".into(),
                        "read".into(),
                        "deny".into(),
                    )
                    .await,
                Err(AuthzError::ProtectedSystemRole)
            ));
            assert!(matches!(
                h.authz
                    .replace_subject_policies("role".into(), super_admin.id, vec![])
                    .await,
                Err(AuthzError::ProtectedSystemRole)
            ));
            assert!(matches!(
                h.authz.delete_policy(policy.id).await,
                Err(AuthzError::ProtectedSystemRole)
            ));
            assert!(matches!(
                h.authz.set_user_roles(user, vec![]).await,
                Err(AuthzError::ProtectedSystemRole)
            ));
        }

        #[tokio::test]
        async fn replace_subject_policies_swaps_the_full_set_atomically() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let role = h.role("accountant", KIND_POSITION, None).await;
            h.assign(user, vec![role]).await;

            h.policy("role", role, "finance:invoices", "read", "allow")
                .await;
            assert!(h.check(user, "finance:invoices", "read").await);

            let created = h
                .authz
                .replace_subject_policies(
                    "role".to_string(),
                    role,
                    vec![
                        NewPolicy {
                            object: "finance:reports".to_string(),
                            action: "approve".to_string(),
                            effect: "allow".to_string(),
                        },
                        NewPolicy {
                            object: "finance:ledger".to_string(),
                            action: "read".to_string(),
                            effect: "deny".to_string(),
                        },
                    ],
                )
                .await
                .expect("replace should succeed");
            assert_eq!(created.len(), 2);

            // The old policy is gone, the new set is live.
            assert!(!h.check(user, "finance:invoices", "read").await);
            assert!(h.check(user, "finance:reports", "approve").await);
            assert!(!h.check(user, "finance:ledger", "read").await);

            let listed = h
                .authz
                .list_policies(Some("role".to_string()), Some(role))
                .await
                .expect("policies should list");
            assert_eq!(listed.len(), 2);

            // Replacing with an empty set clears every policy.
            h.authz
                .replace_subject_policies("role".to_string(), role, vec![])
                .await
                .expect("empty replace should succeed");
            assert!(!h.check(user, "finance:reports", "approve").await);
        }

        #[tokio::test]
        async fn replace_subject_policies_validates_input_and_leaves_state_untouched() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;
            let role = h.role("accountant", KIND_POSITION, None).await;
            h.assign(user, vec![role]).await;
            h.policy("role", role, "finance:invoices", "read", "allow")
                .await;

            let valid = NewPolicy {
                object: "finance:reports".to_string(),
                action: "approve".to_string(),
                effect: "allow".to_string(),
            };

            let duplicate_pair = h
                .authz
                .replace_subject_policies(
                    "role".to_string(),
                    role,
                    vec![
                        valid.clone(),
                        NewPolicy {
                            effect: "deny".to_string(),
                            ..valid.clone()
                        },
                    ],
                )
                .await;
            assert!(matches!(duplicate_pair, Err(AuthzError::InvalidInput(_))));

            let unknown_object = h
                .authz
                .replace_subject_policies(
                    "role".to_string(),
                    role,
                    vec![NewPolicy {
                        object: "nonexistent:domain".to_string(),
                        ..valid.clone()
                    }],
                )
                .await;
            assert!(matches!(
                unknown_object,
                Err(AuthzError::PermissionNotInCatalog)
            ));

            let bad_effect = h
                .authz
                .replace_subject_policies(
                    "role".to_string(),
                    role,
                    vec![NewPolicy {
                        effect: "maybe".to_string(),
                        ..valid.clone()
                    }],
                )
                .await;
            assert!(matches!(bad_effect, Err(AuthzError::InvalidEffect)));

            let bad_kind = h
                .authz
                .replace_subject_policies("group".to_string(), role, vec![valid.clone()])
                .await;
            assert!(matches!(bad_kind, Err(AuthzError::InvalidSubjectKind)));

            let missing_role = h
                .authz
                .replace_subject_policies("role".to_string(), Uuid::new_v4(), vec![valid.clone()])
                .await;
            assert!(matches!(missing_role, Err(AuthzError::RoleNotFound)));

            let missing_user = h
                .authz
                .replace_subject_policies("user".to_string(), Uuid::new_v4(), vec![valid])
                .await;
            assert!(matches!(missing_user, Err(AuthzError::UserNotFound)));

            // Every failure left the original policy set untouched.
            assert!(h.check(user, "finance:invoices", "read").await);
        }

        #[tokio::test]
        async fn create_policy_rejects_objects_outside_the_catalog() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;

            // A typo'd object must not create a dead policy.
            let typo = h
                .authz
                .create_policy(
                    "user".to_string(),
                    user,
                    "finanse:invoices".to_string(),
                    "read".to_string(),
                    "allow".to_string(),
                )
                .await;
            assert!(matches!(typo, Err(AuthzError::PermissionNotInCatalog)));

            // A wildcard covering catalog objects is accepted.
            h.policy("user", user, "finance:*", "read", "allow").await;
            // An action no covered entry supports is rejected.
            let wrong_action = h
                .authz
                .create_policy(
                    "user".to_string(),
                    user,
                    "finance:ledger".to_string(),
                    "approve".to_string(),
                    "allow".to_string(),
                )
                .await;
            assert!(matches!(
                wrong_action,
                Err(AuthzError::PermissionNotInCatalog)
            ));
        }

        #[tokio::test]
        async fn validates_role_and_policy_inputs() {
            let h = Harness::new().await;
            let user = h.user("ding-1").await;

            let bad_kind = h
                .authz
                .create_role("x".into(), "x".into(), "team".into(), None)
                .await;
            assert!(matches!(bad_kind, Err(AuthzError::InvalidRoleKind)));

            h.role("dup", KIND_CUSTOM, None).await;
            let dup = h
                .authz
                .create_role("dup".into(), "dup".into(), KIND_CUSTOM.into(), None)
                .await;
            assert!(matches!(dup, Err(AuthzError::DuplicateRoleCode)));

            let zero_priority = h
                .authz
                .create_role("z".into(), "z".into(), KIND_CUSTOM.into(), Some(0))
                .await;
            assert!(matches!(zero_priority, Err(AuthzError::InvalidInput(_))));

            let bad_effect = h
                .authz
                .create_policy(
                    "user".into(),
                    user,
                    "finance:x".into(),
                    "read".into(),
                    "maybe".into(),
                )
                .await;
            assert!(matches!(bad_effect, Err(AuthzError::InvalidEffect)));

            let bad_action = h
                .authz
                .create_policy(
                    "user".into(),
                    user,
                    "finance:x".into(),
                    "delete".into(),
                    "allow".into(),
                )
                .await;
            assert!(matches!(bad_action, Err(AuthzError::InvalidInput(_))));

            let unknown_role_subject = h
                .authz
                .create_policy(
                    "role".into(),
                    Uuid::new_v4(),
                    "finance:x".into(),
                    "read".into(),
                    "allow".into(),
                )
                .await;
            assert!(matches!(
                unknown_role_subject,
                Err(AuthzError::RoleNotFound)
            ));

            h.policy("user", user, "finance:x", "read", "allow").await;
            let dup_policy = h
                .authz
                .create_policy(
                    "user".into(),
                    user,
                    "finance:x".into(),
                    "read".into(),
                    "deny".into(),
                )
                .await;
            assert!(matches!(dup_policy, Err(AuthzError::DuplicatePolicy)));
        }
    }
}

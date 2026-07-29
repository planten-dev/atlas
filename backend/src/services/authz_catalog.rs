//! Authoritative catalog of every permission the backend enforces.
//!
//! Route declarations in `app.rs` are validated against this catalog at
//! router build time (a permission missing from the catalog fails startup,
//! same as a malformed permission string), and policy writes are validated
//! against it so a typo'd object can never create a dead policy. Approval
//! permissions declared via `ReviewableResource::APPROVAL_PERMISSION` are
//! merged in at startup (see `main.rs`), so the catalog stays the single
//! list that the permission panel and
//! `AuthzService::effective_permissions` enumerate.

/// One permission object together with the actions it supports and the
/// display metadata the permission panel groups it by.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    object: String,
    actions: Vec<String>,
    group: String,
    label: String,
}

impl CatalogEntry {
    pub fn object(&self) -> &str {
        &self.object
    }

    pub fn actions(&self) -> &[String] {
        &self.actions
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone)]
pub struct PermissionCatalog {
    entries: Vec<CatalogEntry>,
}

impl PermissionCatalog {
    /// The permissions enforced by static route declarations. Every
    /// `require_permission` string in `app.rs` must appear here; the
    /// reverse is checked at router build time.
    pub fn builtin() -> Self {
        let mut catalog = Self {
            entries: Vec::new(),
        };
        let builtin: [(&str, &[&str], &str, &str); 12] = [
            ("users", &["read", "write"], "系统", "用户管理"),
            ("departments", &["read", "write"], "系统", "部门"),
            (
                "products:categories",
                &["read", "write"],
                "商品",
                "商品分类",
            ),
            ("products", &["read", "write"], "商品", "商品"),
            ("events", &["read"], "审核", "审核事件"),
            ("systems", &["read", "write"], "门店", "门店体系"),
            ("stores", &["read", "write"], "门店", "门店"),
            ("customers", &["read", "write"], "销售", "客户"),
            (
                "sales:records",
                &["read", "write"],
                "销售",
                "销售记录及可操作次数",
            ),
            (
                "sales:operation-usages",
                &["read", "write"],
                "销售",
                "耗用记录",
            ),
            ("sales:performance", &["read", "post"], "销售", "人员业绩"),
            ("system:permissions", &["read", "write"], "系统", "权限管理"),
        ];
        for (object, actions, group, label) in builtin {
            for action in actions {
                catalog.add_permission(object, action, group, label);
            }
        }
        catalog
    }

    /// Adds one `(object, action)` permission. When the object already has
    /// an entry the action is merged into it (existing group/label win);
    /// otherwise a new entry is appended.
    pub fn add_permission(&mut self, object: &str, action: &str, group: &str, label: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.object == object) {
            if !entry.actions.iter().any(|existing| existing == action) {
                entry.actions.push(action.to_string());
            }
            return;
        }
        self.entries.push(CatalogEntry {
            object: object.to_string(),
            actions: vec![action.to_string()],
            group: group.to_string(),
            label: label.to_string(),
        });
    }

    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    /// Exact `(object, action)` membership — used to validate route
    /// declarations, which never carry wildcards.
    pub fn contains(&self, object: &str, action: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.object == object && entry.actions.iter().any(|a| a == action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_contains_route_permissions() {
        let catalog = PermissionCatalog::builtin();
        assert!(catalog.contains("products", "read"));
        assert!(catalog.contains("products:categories", "write"));
        assert!(catalog.contains("system:permissions", "write"));
        assert!(catalog.contains("events", "read"));
        assert!(catalog.contains("sales:records", "read"));
        assert!(catalog.contains("sales:records", "write"));
        assert!(catalog.contains("sales:performance", "read"));
        assert!(catalog.contains("sales:performance", "post"));
        assert!(!catalog.contains("sales:operation-counts", "read"));
        assert!(!catalog.contains("events", "write"));
        assert!(!catalog.contains("nonexistent", "read"));
    }

    #[test]
    fn add_permission_merges_actions_into_existing_entry() {
        let mut catalog = PermissionCatalog::builtin();
        let entry_count = catalog.entries().len();

        catalog.add_permission("products", "approve", "审核", "商品审核");
        assert_eq!(catalog.entries().len(), entry_count);
        assert!(catalog.contains("products", "approve"));

        // Duplicate add is a no-op.
        catalog.add_permission("products", "approve", "审核", "商品审核");
        let products = catalog
            .entries()
            .iter()
            .find(|entry| entry.object() == "products")
            .expect("products entry should exist");
        assert_eq!(
            products
                .actions()
                .iter()
                .filter(|action| *action == "approve")
                .count(),
            1
        );
        // The original display metadata is kept on merge.
        assert_eq!(products.group(), "商品");
    }

    #[test]
    fn add_permission_appends_new_objects() {
        let mut catalog = PermissionCatalog::builtin();
        catalog.add_permission("finance:docs", "approve", "审核", "财务文档");
        assert!(catalog.contains("finance:docs", "approve"));
        let entry = catalog
            .entries()
            .iter()
            .find(|entry| entry.object() == "finance:docs")
            .expect("new entry should exist");
        assert_eq!(entry.actions(), ["approve".to_string()]);
        assert_eq!(entry.group(), "审核");
        assert_eq!(entry.label(), "财务文档");
    }

    #[test]
    fn sales_approval_permissions_are_addable_with_chinese_labels() {
        let mut catalog = PermissionCatalog::builtin();
        catalog.add_permission("sales:records", "approve", "审核", "销售记录及可操作次数");
        catalog.add_permission("sales:operation-usages", "approve", "审核", "耗用记录");

        assert!(catalog.contains("sales:records", "approve"));
        assert!(catalog.contains("sales:operation-usages", "approve"));
        let records = catalog
            .entries()
            .iter()
            .find(|entry| entry.object() == "sales:records")
            .expect("sales records approval entry should exist");
        assert_eq!(records.label(), "销售记录及可操作次数");
        let usages = catalog
            .entries()
            .iter()
            .find(|entry| entry.object() == "sales:operation-usages")
            .expect("sales operation usages entry should exist");
        assert_eq!(usages.label(), "耗用记录");
    }
}

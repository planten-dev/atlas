use std::collections::HashMap;

use chrono::Utc;
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    config::DingTalkConfig,
    entities::departments,
    integrations::dingtalk::{
        DingTalkClient, DingTalkDepartment, DingTalkError, ROOT_DEPARTMENT_ID,
    },
    repositories::{RepositoryError, departments::DepartmentRepository},
};

pub const SOURCE_DINGTALK: &str = "dingtalk";
pub const SOURCE_MANUAL: &str = "manual";

#[derive(Clone)]
pub struct DepartmentService {
    dingtalk_config: DingTalkConfig,
    departments: DepartmentRepository,
}

impl DepartmentService {
    pub fn new(dingtalk_config: DingTalkConfig, departments: DepartmentRepository) -> Self {
        Self {
            dingtalk_config,
            departments,
        }
    }

    #[tracing::instrument(level = "info", skip(self), fields(provider = SOURCE_DINGTALK))]
    pub async fn sync_from_dingtalk(&self) -> Result<DepartmentSyncSummary, DepartmentError> {
        let client = DingTalkClient::new(self.dingtalk_config.clone())?;
        let fetched = client.fetch_all_departments().await?;
        let existing = self.departments.list_by_source(SOURCE_DINGTALK).await?;
        let plan = plan_department_sync(&fetched, &existing);
        let now = Utc::now();

        // Inserts follow the BFS order of `fetched`, so every parent row is
        // written before its children and the self-referential foreign key
        // holds. Updates run after all inserts so re-parenting onto a newly
        // created department is also safe.
        for insert in &plan.inserts {
            self.departments
                .insert_department(
                    insert.id,
                    SOURCE_DINGTALK,
                    &insert.external_department_id,
                    &insert.name,
                    insert.parent_id,
                    now,
                )
                .await?;
        }

        for update in &plan.updates {
            self.departments
                .update_name_and_parent(update.id, &update.name, update.parent_id, now)
                .await?;
        }

        let summary = DepartmentSyncSummary {
            total: fetched.len(),
            created: plan.inserts.len(),
            updated: plan.updates.len(),
            unchanged: plan.unchanged,
        };
        info!(
            total = summary.total,
            created = summary.created,
            updated = summary.updated,
            unchanged = summary.unchanged,
            "completed DingTalk department sync"
        );
        Ok(summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepartmentSyncSummary {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
}

#[derive(Debug, Error)]
pub enum DepartmentError {
    #[error(transparent)]
    DingTalk(#[from] DingTalkError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl DepartmentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DingTalk(error) => match error {
                DingTalkError::MissingConfig(_) => "dingtalk_configuration_error",
                _ => "dingtalk_error",
            },
            Self::Repository(error) => match error {
                RepositoryError::Database(_) => "database_error",
                _ => "validation_error",
            },
        }
    }
}

pub(crate) struct DepartmentSyncPlan {
    pub inserts: Vec<DepartmentInsert>,
    pub updates: Vec<DepartmentUpdate>,
    pub unchanged: usize,
}

pub(crate) struct DepartmentInsert {
    pub id: Uuid,
    pub external_department_id: String,
    pub name: String,
    pub parent_id: Option<Uuid>,
}

pub(crate) struct DepartmentUpdate {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
}

/// Diffs the DingTalk department tree against the rows already stored for the
/// `dingtalk` source. UUIDs for new departments are generated here so parent
/// links are fully resolved before any database write. Rows missing from
/// `fetched` are left untouched (deletion is not synced).
pub(crate) fn plan_department_sync(
    fetched: &[DingTalkDepartment],
    existing: &[departments::Model],
) -> DepartmentSyncPlan {
    let existing_by_external_id: HashMap<&str, &departments::Model> = existing
        .iter()
        .map(|model| (model.external_department_id.as_str(), model))
        .collect();

    let uuid_by_dept_id: HashMap<i64, Uuid> = fetched
        .iter()
        .map(|department| {
            let id = existing_by_external_id
                .get(department.dept_id.to_string().as_str())
                .map(|model| model.id)
                .unwrap_or_else(Uuid::new_v4);
            (department.dept_id, id)
        })
        .collect();

    let mut plan = DepartmentSyncPlan {
        inserts: Vec::new(),
        updates: Vec::new(),
        unchanged: 0,
    };

    for department in fetched {
        let parent_id = match department.parent_id {
            None | Some(ROOT_DEPARTMENT_ID) => None,
            Some(parent_dept_id) => {
                let parent_uuid = uuid_by_dept_id.get(&parent_dept_id).copied();
                if parent_uuid.is_none() {
                    warn!(
                        dept_id = department.dept_id,
                        parent_dept_id, "DingTalk department references an unknown parent"
                    );
                }
                parent_uuid
            }
        };

        let external_department_id = department.dept_id.to_string();
        match existing_by_external_id.get(external_department_id.as_str()) {
            None => plan.inserts.push(DepartmentInsert {
                id: uuid_by_dept_id[&department.dept_id],
                external_department_id,
                name: department.name.clone(),
                parent_id,
            }),
            Some(model) => {
                if model.name != department.name || model.parent_id != parent_id {
                    plan.updates.push(DepartmentUpdate {
                        id: model.id,
                        name: department.name.clone(),
                        parent_id,
                    });
                } else {
                    plan.unchanged += 1;
                }
            }
        }
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DatabaseConfig, DatabaseKind},
        db,
    };
    use axum::{
        Form, Json, Router,
        routing::{get, post},
    };
    use chrono::{TimeZone, Utc};
    use sea_orm::{ActiveModelTrait, Set};
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use tokio::net::TcpListener;

    fn dingtalk_department(dept_id: i64, name: &str, parent_id: Option<i64>) -> DingTalkDepartment {
        DingTalkDepartment {
            dept_id,
            name: name.to_string(),
            parent_id,
        }
    }

    fn department_model(
        external_department_id: &str,
        name: &str,
        parent_id: Option<Uuid>,
    ) -> departments::Model {
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        departments::Model {
            id: Uuid::new_v4(),
            source: SOURCE_DINGTALK.to_string(),
            external_department_id: external_department_id.to_string(),
            parent_id,
            name: name.to_string(),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn plans_inserts_for_new_tree_with_resolved_parent_links() {
        let fetched = vec![
            dingtalk_department(10, "总裁办", Some(ROOT_DEPARTMENT_ID)),
            dingtalk_department(20, "研发", Some(ROOT_DEPARTMENT_ID)),
            dingtalk_department(21, "后端", Some(20)),
        ];

        let plan = plan_department_sync(&fetched, &[]);

        assert_eq!(plan.inserts.len(), 3);
        assert_eq!(plan.updates.len(), 0);
        assert_eq!(plan.unchanged, 0);
        assert_eq!(plan.inserts[0].external_department_id, "10");
        assert_eq!(plan.inserts[0].parent_id, None);
        assert_eq!(plan.inserts[1].parent_id, None);
        assert_eq!(plan.inserts[2].external_department_id, "21");
        assert_eq!(plan.inserts[2].parent_id, Some(plan.inserts[1].id));
    }

    #[test]
    fn plans_unchanged_when_everything_matches() {
        let parent = department_model("10", "总裁办", None);
        let child = department_model("21", "后端", Some(parent.id));
        let fetched = vec![
            dingtalk_department(10, "总裁办", Some(ROOT_DEPARTMENT_ID)),
            dingtalk_department(21, "后端", Some(10)),
        ];

        let plan = plan_department_sync(&fetched, &[parent, child]);

        assert_eq!(plan.inserts.len(), 0);
        assert_eq!(plan.updates.len(), 0);
        assert_eq!(plan.unchanged, 2);
    }

    #[test]
    fn plans_update_for_renamed_and_reparented_departments() {
        let parent_a = department_model("10", "总裁办", None);
        let parent_b = department_model("20", "研发", None);
        let child = department_model("21", "后端", Some(parent_b.id));
        let fetched = vec![
            dingtalk_department(10, "总裁办公室", Some(ROOT_DEPARTMENT_ID)),
            dingtalk_department(20, "研发", Some(ROOT_DEPARTMENT_ID)),
            dingtalk_department(21, "后端", Some(10)),
        ];
        let expected_parent = parent_a.id;
        let expected_renamed = parent_a.id;
        let expected_child = child.id;

        let plan = plan_department_sync(&fetched, &[parent_a, parent_b, child]);

        assert_eq!(plan.inserts.len(), 0);
        assert_eq!(plan.updates.len(), 2);
        assert_eq!(plan.unchanged, 1);
        assert_eq!(plan.updates[0].id, expected_renamed);
        assert_eq!(plan.updates[0].name, "总裁办公室");
        assert_eq!(plan.updates[1].id, expected_child);
        assert_eq!(plan.updates[1].parent_id, Some(expected_parent));
    }

    #[test]
    fn plans_none_parent_for_unknown_parent_reference() {
        let fetched = vec![dingtalk_department(21, "后端", Some(999))];

        let plan = plan_department_sync(&fetched, &[]);

        assert_eq!(plan.inserts.len(), 1);
        assert_eq!(plan.inserts[0].parent_id, None);
    }

    #[test]
    fn leaves_departments_missing_from_dingtalk_untouched() {
        let removed = department_model("30", "已裁撤", None);
        let kept = department_model("10", "总裁办", None);
        let removed_id = removed.id;
        let fetched = vec![dingtalk_department(10, "总裁办", Some(ROOT_DEPARTMENT_ID))];

        let plan = plan_department_sync(&fetched, &[removed, kept]);

        assert_eq!(plan.inserts.len(), 0);
        assert_eq!(plan.updates.len(), 0);
        assert_eq!(plan.unchanged, 1);
        assert!(plan.updates.iter().all(|update| update.id != removed_id));
    }

    fn sqlite_memory_config() -> DatabaseConfig {
        DatabaseConfig {
            kind: DatabaseKind::SqliteMemory,
            url: "postgres://unused".to_string(),
            sqlite_file: PathBuf::from("unused.sqlite"),
        }
    }

    fn test_dingtalk_config(mock_base_url: &str) -> DingTalkConfig {
        DingTalkConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://127.0.0.1:3000/api/v1/auth/callback/dingtalk".to_string(),
            auth_url: "https://login.dingtalk.com/oauth2/auth".to_string(),
            token_url: format!("{mock_base_url}/token"),
            user_info_url: format!("{mock_base_url}/me"),
            corp_token_url: format!("{mock_base_url}/gettoken"),
            department_listsub_url: format!("{mock_base_url}/listsub"),
            user_detail_url: format!("{mock_base_url}/user_detail"),
            scope: "openid".to_string(),
            corp_id: "".to_string(),
            external_id_fields: vec!["userId".to_string()],
        }
    }

    #[derive(serde::Deserialize)]
    struct ListSubForm {
        dept_id: i64,
        language: String,
    }

    async fn start_mock_dingtalk_org(tree: HashMap<i64, Vec<Value>>) -> String {
        async fn gettoken() -> Json<Value> {
            Json(json!({
                "errcode": 0,
                "errmsg": "ok",
                "access_token": "corp-token",
                "expires_in": 7200
            }))
        }

        let listsub = move |Form(form): Form<ListSubForm>| {
            let tree = tree.clone();
            async move {
                assert_eq!(form.language, "zh_CN");
                let result = tree.get(&form.dept_id).cloned().unwrap_or_default();
                Json(json!({
                    "errcode": 0,
                    "errmsg": "ok",
                    "result": result
                }))
            }
        };

        let app = Router::new()
            .route("/gettoken", get(gettoken))
            .route("/listsub", post(listsub));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock DingTalk listener should bind");
        let addr = listener.local_addr().expect("mock address should be known");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock DingTalk server should run");
        });
        format!("http://{addr}")
    }

    fn default_tree() -> HashMap<i64, Vec<Value>> {
        HashMap::from([
            (
                ROOT_DEPARTMENT_ID,
                vec![
                    json!({"dept_id": 10, "name": "总裁办", "parent_id": 1}),
                    json!({"dept_id": 20, "name": "研发", "parent_id": 1}),
                ],
            ),
            (
                20,
                vec![json!({"dept_id": 21, "name": "后端", "parent_id": 20})],
            ),
        ])
    }

    #[tokio::test]
    async fn syncs_departments_from_dingtalk_idempotently() {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let repository = DepartmentRepository::new(db.clone());
        let mock_base_url = start_mock_dingtalk_org(default_tree()).await;
        let service =
            DepartmentService::new(test_dingtalk_config(&mock_base_url), repository.clone());

        let first = service
            .sync_from_dingtalk()
            .await
            .expect("first sync should succeed");
        assert_eq!(
            first,
            DepartmentSyncSummary {
                total: 3,
                created: 3,
                updated: 0,
                unchanged: 0
            }
        );

        let parent = repository
            .find_by_source_and_external_id(SOURCE_DINGTALK, "20")
            .await
            .expect("parent lookup should succeed")
            .expect("parent should exist");
        let child = repository
            .find_by_source_and_external_id(SOURCE_DINGTALK, "21")
            .await
            .expect("child lookup should succeed")
            .expect("child should exist");
        assert_eq!(parent.parent_id, None);
        assert_eq!(child.parent_id, Some(parent.id));
        assert_eq!(child.name, "后端");

        let second = service
            .sync_from_dingtalk()
            .await
            .expect("second sync should succeed");
        assert_eq!(
            second,
            DepartmentSyncSummary {
                total: 3,
                created: 0,
                updated: 0,
                unchanged: 3
            }
        );
    }

    #[tokio::test]
    async fn syncs_renames_and_reparenting_without_touching_status_or_manual_rows() {
        let db = db::connect_and_migrate(&sqlite_memory_config())
            .await
            .expect("sqlite memory database should initialize");
        let repository = DepartmentRepository::new(db.clone());
        let now = Utc.with_ymd_and_hms(2026, 7, 7, 0, 0, 0).unwrap();
        let manual = repository
            .insert_department(Uuid::new_v4(), SOURCE_MANUAL, "10", "手动部门", None, now)
            .await
            .expect("manual department should be created");

        let mock_base_url = start_mock_dingtalk_org(default_tree()).await;
        let service =
            DepartmentService::new(test_dingtalk_config(&mock_base_url), repository.clone());
        service
            .sync_from_dingtalk()
            .await
            .expect("first sync should succeed");

        let synced = repository
            .find_by_source_and_external_id(SOURCE_DINGTALK, "21")
            .await
            .expect("department lookup should succeed")
            .expect("department should exist");
        let mut active: departments::ActiveModel = synced.into();
        active.status = Set("disabled".to_string());
        active
            .update(&db)
            .await
            .expect("department should be disabled");

        // Second mock: dept 21 renamed and re-parented under dept 10.
        let changed_tree = HashMap::from([
            (
                ROOT_DEPARTMENT_ID,
                vec![
                    json!({"dept_id": 10, "name": "总裁办", "parent_id": 1}),
                    json!({"dept_id": 20, "name": "研发", "parent_id": 1}),
                ],
            ),
            (
                10,
                vec![json!({"dept_id": 21, "name": "平台后端", "parent_id": 10})],
            ),
        ]);
        let changed_base_url = start_mock_dingtalk_org(changed_tree).await;
        let changed_service =
            DepartmentService::new(test_dingtalk_config(&changed_base_url), repository.clone());

        let summary = changed_service
            .sync_from_dingtalk()
            .await
            .expect("changed sync should succeed");
        assert_eq!(
            summary,
            DepartmentSyncSummary {
                total: 3,
                created: 0,
                updated: 1,
                unchanged: 2
            }
        );

        let new_parent = repository
            .find_by_source_and_external_id(SOURCE_DINGTALK, "10")
            .await
            .expect("parent lookup should succeed")
            .expect("parent should exist");
        let updated = repository
            .find_by_source_and_external_id(SOURCE_DINGTALK, "21")
            .await
            .expect("department lookup should succeed")
            .expect("department should exist");
        assert_eq!(updated.name, "平台后端");
        assert_eq!(updated.parent_id, Some(new_parent.id));
        assert_eq!(updated.status, "disabled");

        let untouched_manual = repository
            .find_by_source_and_external_id(SOURCE_MANUAL, "10")
            .await
            .expect("manual lookup should succeed")
            .expect("manual department should exist");
        assert_eq!(untouched_manual, manual);
    }
}

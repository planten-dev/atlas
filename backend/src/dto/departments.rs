use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::departments;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DepartmentResponse {
    pub id: Uuid,
    pub source: String,
    pub external_department_id: String,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<departments::Model> for DepartmentResponse {
    fn from(department: departments::Model) -> Self {
        Self {
            id: department.id,
            source: department.source,
            external_department_id: department.external_department_id,
            parent_id: department.parent_id,
            name: department.name,
            status: department.status,
            created_at: department.created_at,
            updated_at: department.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListDepartmentsResponse {
    pub departments: Vec<DepartmentResponse>,
    pub page_number: u64,
    pub page_size: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListDepartmentsQuery {
    pub status_filter: Option<String>,
    pub source_filter: Option<String>,
    pub parent_id: Option<Uuid>,
    pub page_number: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DepartmentSyncResponse {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
}

use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

impl HealthResponse {
    pub fn ok() -> Self {
        Self { status: "ok" }
    }
}

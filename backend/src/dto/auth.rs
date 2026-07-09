use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DingTalkCallbackQuery {
    pub code: Option<String>,
    #[serde(rename = "authCode")]
    pub auth_code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DingTalkH5LoginRequest {
    #[serde(rename = "authCode")]
    pub auth_code: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

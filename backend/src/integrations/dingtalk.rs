use std::collections::{HashSet, VecDeque};

use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::config::DingTalkConfig;

const PROVIDER: &str = "dingtalk";

/// DingTalk's virtual root department id. `listsub` on this id returns the
/// top-level departments; the root itself is never returned by the API.
pub const ROOT_DEPARTMENT_ID: i64 = 1;

#[derive(Clone, Debug)]
pub struct DingTalkClient {
    config: DingTalkConfig,
    http: reqwest::Client,
}

impl DingTalkClient {
    pub fn new(config: DingTalkConfig) -> Result<Self, DingTalkError> {
        validate_config(&config)?;
        let client = Self {
            config,
            http: reqwest::Client::new(),
        };

        info!(
            provider = PROVIDER,
            auth_url = %client.config.auth_url,
            token_url = %client.config.token_url,
            user_info_url = %client.config.user_info_url,
            redirect_uri = %client.config.redirect_uri,
            scope = %client.config.scope,
            external_id_field_count = client.config.external_id_fields.len(),
            "initialized DingTalk client"
        );

        Ok(client)
    }

    #[tracing::instrument(level = "debug", skip(self, state), fields(provider = PROVIDER))]
    pub fn build_authorization_url(&self, state: &str) -> Result<String, DingTalkError> {
        validate_required("state", state)?;
        let mut params = vec![
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("response_type", "code"),
            ("clientId", self.config.client_id.as_str()),
            ("scope", self.config.scope.as_str()),
            ("state", state.trim()),
            ("prompt", "consent"),
        ];

        if scope_requests_corp_id(&self.config.scope) {
            params.push(("corpId", self.config.corp_id.as_str()));
        }

        let query = params
            .into_iter()
            .map(|(key, value)| format!("{key}={}", percent_encode(value)))
            .collect::<Vec<_>>()
            .join("&");

        let authorization_url = format!("{}?{query}", self.config.auth_url);
        debug!(
            provider = PROVIDER,
            auth_url = %self.config.auth_url,
            "built DingTalk authorization URL"
        );
        Ok(authorization_url)
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, code),
        fields(provider = PROVIDER, token_url = %self.config.token_url)
    )]
    pub async fn exchange_code_for_token(
        &self,
        code: &str,
    ) -> Result<DingTalkTokenResponse, DingTalkError> {
        validate_required("code", code)?;
        debug!(provider = PROVIDER, "exchanging DingTalk code for token");

        let response = self
            .http
            .post(&self.config.token_url)
            .json(&json!({
                "clientId": self.config.client_id,
                "clientSecret": self.config.client_secret,
                "code": code.trim(),
                "grantType": "authorization_code",
            }))
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        let parsed = parse_json_or_raw(body);

        if !status.is_success() {
            warn!(
                provider = PROVIDER,
                status = status.as_u16(),
                "DingTalk token exchange returned non-success status"
            );
            return Err(DingTalkError::ProviderHttp {
                operation: "token_exchange",
                status: status.as_u16(),
                body: parsed,
            });
        }

        debug!(
            provider = PROVIDER,
            status = status.as_u16(),
            "received DingTalk token response"
        );
        Ok(DingTalkTokenResponse::from_value(parsed))
    }

    #[tracing::instrument(level = "info", skip(self, token), fields(provider = PROVIDER))]
    pub async fn identity_from_token(
        &self,
        token: DingTalkTokenResponse,
    ) -> Result<DingTalkIdentity, DingTalkError> {
        let user_info = if token.external_id(&self.config.external_id_fields).is_some() {
            None
        } else {
            debug!(
                provider = PROVIDER,
                "token response is missing configured external id, fetching user info"
            );
            Some(self.fetch_user_info(&token).await?)
        };

        let identity = resolve_identity(token, user_info, &self.config.external_id_fields)?;
        info!(
            provider = PROVIDER,
            has_corp_id = identity.corp_id.is_some() || !self.config.corp_id.trim().is_empty(),
            has_union_id = identity.union_id.is_some(),
            has_open_id = identity.open_id.is_some(),
            has_provider_user_id = identity.provider_user_id.is_some(),
            "resolved DingTalk identity"
        );

        Ok(identity)
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, token),
        fields(provider = PROVIDER, user_info_url = %self.config.user_info_url)
    )]
    async fn fetch_user_info(
        &self,
        token: &DingTalkTokenResponse,
    ) -> Result<DingTalkUserInfoResponse, DingTalkError> {
        let access_token = token
            .access_token()
            .ok_or(DingTalkError::MissingIdentityField(
                "accessToken".to_string(),
            ))?;

        debug!(provider = PROVIDER, "fetching DingTalk user info");
        let response = self
            .http
            .get(&self.config.user_info_url)
            .header("x-acs-dingtalk-access-token", access_token)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        let parsed = parse_json_or_raw(body);

        if !status.is_success() {
            warn!(
                provider = PROVIDER,
                status = status.as_u16(),
                "DingTalk user info request returned non-success status"
            );
            return Err(DingTalkError::ProviderHttp {
                operation: "user_info",
                status: status.as_u16(),
                body: parsed,
            });
        }

        debug!(
            provider = PROVIDER,
            status = status.as_u16(),
            "received DingTalk user info response"
        );
        Ok(DingTalkUserInfoResponse { raw: parsed })
    }

    #[tracing::instrument(
        level = "debug",
        skip(self),
        fields(provider = PROVIDER, corp_token_url = %self.config.corp_token_url)
    )]
    pub async fn fetch_corp_access_token(&self) -> Result<String, DingTalkError> {
        debug!(provider = PROVIDER, "fetching DingTalk corp access token");
        let response = self
            .http
            .get(&self.config.corp_token_url)
            .query(&[
                ("appkey", self.config.client_id.as_str()),
                ("appsecret", self.config.client_secret.as_str()),
            ])
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        let parsed = parse_json_or_raw(body);

        if !status.is_success() {
            warn!(
                provider = PROVIDER,
                status = status.as_u16(),
                "DingTalk corp token request returned non-success status"
            );
            return Err(DingTalkError::ProviderHttp {
                operation: "corp_token",
                status: status.as_u16(),
                body: parsed,
            });
        }

        check_oapi_errcode("corp_token", &parsed)?;
        parsed
            .get("access_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToString::to_string)
            .ok_or(DingTalkError::MissingResponseField {
                operation: "corp_token",
                field: "access_token",
            })
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, access_token),
        fields(provider = PROVIDER, dept_id)
    )]
    pub async fn list_sub_departments(
        &self,
        access_token: &str,
        dept_id: i64,
    ) -> Result<Vec<DingTalkDepartment>, DingTalkError> {
        validate_required("access_token", access_token)?;
        let response = self
            .http
            .post(&self.config.department_listsub_url)
            .form(&[
                ("access_token", access_token),
                ("dept_id", &dept_id.to_string()),
                ("language", "zh_CN"),
            ])
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        let parsed = parse_json_or_raw(body);

        if !status.is_success() {
            warn!(
                provider = PROVIDER,
                status = status.as_u16(),
                "DingTalk department listsub request returned non-success status"
            );
            return Err(DingTalkError::ProviderHttp {
                operation: "department_listsub",
                status: status.as_u16(),
                body: parsed,
            });
        }

        check_oapi_errcode("department_listsub", &parsed)?;
        let Some(result) = parsed.get("result") else {
            return Ok(Vec::new());
        };

        let raw_departments: Vec<RawDingTalkDepartment> = serde_json::from_value(result.clone())
            .map_err(|error| {
                warn!(
                    provider = PROVIDER,
                    %error,
                    "failed to parse DingTalk department listsub result"
                );
                DingTalkError::MissingResponseField {
                    operation: "department_listsub",
                    field: "result",
                }
            })?;

        Ok(raw_departments
            .into_iter()
            .map(|raw| DingTalkDepartment {
                dept_id: raw.dept_id,
                name: raw.name,
                parent_id: raw.parent_id,
            })
            .collect())
    }

    #[tracing::instrument(level = "info", skip(self), fields(provider = PROVIDER))]
    pub async fn fetch_all_departments(&self) -> Result<Vec<DingTalkDepartment>, DingTalkError> {
        let access_token = self.fetch_corp_access_token().await?;

        let mut queue = VecDeque::from([ROOT_DEPARTMENT_ID]);
        let mut visited = HashSet::from([ROOT_DEPARTMENT_ID]);
        let mut departments = Vec::new();

        while let Some(dept_id) = queue.pop_front() {
            for department in self.list_sub_departments(&access_token, dept_id).await? {
                if visited.insert(department.dept_id) {
                    queue.push_back(department.dept_id);
                    departments.push(department);
                } else {
                    warn!(
                        provider = PROVIDER,
                        dept_id = department.dept_id,
                        "skipping duplicate department id from DingTalk"
                    );
                }
            }
        }

        info!(
            provider = PROVIDER,
            count = departments.len(),
            "fetched DingTalk department tree"
        );
        Ok(departments)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DingTalkDepartment {
    pub dept_id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawDingTalkDepartment {
    dept_id: i64,
    name: String,
    #[serde(default)]
    parent_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DingTalkIdentity {
    pub dingtalk_user_id: String,
    pub corp_id: Option<String>,
    pub union_id: Option<String>,
    pub open_id: Option<String>,
    pub provider_user_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DingTalkTokenResponse {
    raw: Value,
}

impl DingTalkTokenResponse {
    pub fn from_value(raw: Value) -> Self {
        Self { raw }
    }

    fn access_token(&self) -> Option<String> {
        first_string(&self.raw, &["accessToken", "access_token"])
    }

    fn external_id(&self, fields: &[String]) -> Option<String> {
        first_string_dynamic(&self.raw, fields)
    }

    fn corp_id(&self) -> Option<String> {
        first_string(&self.raw, &["corpId", "corp_id"])
    }

    fn union_id(&self) -> Option<String> {
        first_string(&self.raw, &["unionId", "unionid", "union_id"])
    }

    fn open_id(&self) -> Option<String> {
        first_string(&self.raw, &["openId", "openid", "open_id"])
    }

    fn provider_user_id(&self) -> Option<String> {
        first_string(&self.raw, &["userId", "userid", "user_id"])
    }
}

#[derive(Debug, Clone)]
struct DingTalkUserInfoResponse {
    raw: Value,
}

impl DingTalkUserInfoResponse {
    fn external_id(&self, fields: &[String]) -> Option<String> {
        first_string_dynamic(&self.raw, fields)
    }

    fn corp_id(&self) -> Option<String> {
        first_string(&self.raw, &["corpId", "corp_id"])
    }

    fn union_id(&self) -> Option<String> {
        first_string(&self.raw, &["unionId", "unionid", "union_id"])
    }

    fn open_id(&self) -> Option<String> {
        first_string(&self.raw, &["openId", "openid", "open_id"])
    }

    fn provider_user_id(&self) -> Option<String> {
        first_string(&self.raw, &["userId", "userid", "user_id"])
    }
}

#[derive(Debug, Error)]
pub enum DingTalkError {
    #[error("DingTalk config `{0}` is required")]
    MissingConfig(&'static str),
    #[error("DingTalk `{field}` is required")]
    MissingRequiredField { field: &'static str },
    #[error("DingTalk identity response is missing `{0}`")]
    MissingIdentityField(String),
    #[error("DingTalk {operation} request failed with HTTP {status}: {body}")]
    ProviderHttp {
        operation: &'static str,
        status: u16,
        body: Value,
    },
    #[error("DingTalk {operation} request failed with errcode {errcode}: {errmsg}")]
    ProviderApi {
        operation: &'static str,
        errcode: i64,
        errmsg: String,
    },
    #[error("DingTalk {operation} response is missing `{field}`")]
    MissingResponseField {
        operation: &'static str,
        field: &'static str,
    },
    #[error("DingTalk HTTP request failed")]
    Http(#[from] reqwest::Error),
}

pub fn resolve_identity_from_values(
    token: Value,
    user_info: Option<Value>,
    external_id_fields: &[String],
) -> Result<DingTalkIdentity, DingTalkError> {
    resolve_identity(
        DingTalkTokenResponse::from_value(token),
        user_info.map(|raw| DingTalkUserInfoResponse { raw }),
        external_id_fields,
    )
}

pub fn sanitize_token_response(mut value: Value) -> Value {
    remove_sensitive_token_fields(&mut value);
    value
}

fn resolve_identity(
    token: DingTalkTokenResponse,
    user_info: Option<DingTalkUserInfoResponse>,
    external_id_fields: &[String],
) -> Result<DingTalkIdentity, DingTalkError> {
    let dingtalk_user_id = user_info
        .as_ref()
        .and_then(|user_info| user_info.external_id(external_id_fields))
        .or_else(|| token.external_id(external_id_fields))
        .ok_or_else(|| DingTalkError::MissingIdentityField(external_id_fields.join(",")))?;

    Ok(DingTalkIdentity {
        dingtalk_user_id,
        corp_id: user_info
            .as_ref()
            .and_then(DingTalkUserInfoResponse::corp_id)
            .or_else(|| token.corp_id()),
        union_id: user_info
            .as_ref()
            .and_then(DingTalkUserInfoResponse::union_id)
            .or_else(|| token.union_id()),
        open_id: user_info
            .as_ref()
            .and_then(DingTalkUserInfoResponse::open_id)
            .or_else(|| token.open_id()),
        provider_user_id: user_info
            .as_ref()
            .and_then(DingTalkUserInfoResponse::provider_user_id)
            .or_else(|| token.provider_user_id()),
    })
}

fn validate_config(config: &DingTalkConfig) -> Result<(), DingTalkError> {
    validate_config_value("client_id", &config.client_id)?;
    validate_config_value("client_secret", &config.client_secret)?;
    validate_config_value("redirect_uri", &config.redirect_uri)?;
    validate_config_value("auth_url", &config.auth_url)?;
    validate_config_value("token_url", &config.token_url)?;
    validate_config_value("user_info_url", &config.user_info_url)?;
    validate_config_value("corp_token_url", &config.corp_token_url)?;
    validate_config_value("department_listsub_url", &config.department_listsub_url)?;
    validate_config_value("scope", &config.scope)?;

    if config.external_id_fields.is_empty() {
        return Err(DingTalkError::MissingConfig("external_id_fields"));
    }

    if scope_requests_corp_id(&config.scope) {
        validate_config_value("corp_id", &config.corp_id)?;
    }

    Ok(())
}

fn validate_config_value(field: &'static str, value: &str) -> Result<(), DingTalkError> {
    if value.trim().is_empty() {
        return Err(DingTalkError::MissingConfig(field));
    }

    Ok(())
}

fn validate_required(field: &'static str, value: &str) -> Result<(), DingTalkError> {
    if value.trim().is_empty() {
        return Err(DingTalkError::MissingRequiredField { field });
    }

    Ok(())
}

fn parse_json_or_raw(body: String) -> Value {
    serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({ "raw": body }))
}

fn check_oapi_errcode(operation: &'static str, value: &Value) -> Result<(), DingTalkError> {
    // The oapi contract always includes `errcode` on success, so a missing
    // field is treated as a failure rather than silently accepted.
    let errcode = value.get("errcode").and_then(Value::as_i64).unwrap_or(-1);
    if errcode != 0 {
        let errmsg = value
            .get("errmsg")
            .and_then(Value::as_str)
            .unwrap_or("unknown error")
            .to_string();
        warn!(
            provider = PROVIDER,
            operation, errcode, "DingTalk API returned non-zero errcode"
        );
        return Err(DingTalkError::ProviderApi {
            operation,
            errcode,
            errmsg,
        });
    }

    Ok(())
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| first_string_by_key(value, key))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn first_string_dynamic(value: &Value, keys: &[String]) -> Option<String> {
    keys.iter()
        .find_map(|key| first_string_by_key(value, key))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn first_string_by_key<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .or_else(|| nested_string(value, "result", key))
        .or_else(|| nested_string(value, "data", key))
        .or_else(|| nested_string(value, "user", key))
        .or_else(|| nested_string(value, "userInfo", key))
}

fn nested_string<'a>(value: &'a Value, parent: &str, key: &str) -> Option<&'a str> {
    value.get(parent)?.get(key)?.as_str()
}

fn remove_sensitive_token_fields(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("accessToken");
        object.remove("access_token");
        object.remove("refreshToken");
        object.remove("refresh_token");
    }
}

fn scope_requests_corp_id(scope: &str) -> bool {
    scope.split_whitespace().any(|part| part == "corpid")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config() -> DingTalkConfig {
        DingTalkConfig {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: "http://127.0.0.1:3000/api/v1/auth/callback/dingtalk".to_string(),
            auth_url: "https://login.dingtalk.com/oauth2/auth".to_string(),
            token_url: "https://api.dingtalk.com/v1.0/oauth2/userAccessToken".to_string(),
            user_info_url: "https://api.dingtalk.com/v1.0/contact/users/me".to_string(),
            corp_token_url: "https://oapi.dingtalk.com/gettoken".to_string(),
            department_listsub_url: "https://oapi.dingtalk.com/topapi/v2/department/listsub"
                .to_string(),
            scope: "openid corpid".to_string(),
            corp_id: "corp-id".to_string(),
            external_id_fields: vec![
                "userId".to_string(),
                "unionId".to_string(),
                "openId".to_string(),
                "uuid".to_string(),
            ],
        }
    }

    #[test]
    fn builds_domestic_authorization_url_with_client_id_and_corp_id() {
        let client = DingTalkClient::new(test_config()).expect("config should be valid");

        let url = client
            .build_authorization_url("state with space")
            .expect("authorization URL should be built");

        assert!(url.starts_with("https://login.dingtalk.com/oauth2/auth?"));
        assert!(url.contains("clientId=client-id"));
        assert!(url.contains(
            "redirect_uri=http%3A%2F%2F127.0.0.1%3A3000%2Fapi%2Fv1%2Fauth%2Fcallback%2Fdingtalk"
        ));
        assert!(url.contains("state=state%20with%20space"));
        assert!(url.contains("corpId=corp-id"));
    }

    #[test]
    fn does_not_require_corp_id_when_scope_omits_corpid() {
        let mut config = test_config();
        config.scope = "openid".to_string();
        config.corp_id = "".to_string();

        let client = DingTalkClient::new(config).expect("corp id should be optional");
        let url = client
            .build_authorization_url("state")
            .expect("authorization URL should be built");

        assert!(!url.contains("corpId="));
    }

    #[test]
    fn rejects_missing_required_config() {
        let mut config = test_config();
        config.client_secret = " ".to_string();

        let error = DingTalkClient::new(config).expect_err("blank secret should be rejected");

        assert!(matches!(
            error,
            DingTalkError::MissingConfig("client_secret")
        ));
    }

    #[test]
    fn resolves_identity_from_token_using_configured_field_priority() {
        let fields = vec![
            "userId".to_string(),
            "unionId".to_string(),
            "openId".to_string(),
            "uuid".to_string(),
        ];
        let token = json!({
            "unionId": "union-id",
            "openId": "open-id",
            "userId": "user-id",
            "corpId": "corp-id"
        });

        let identity =
            resolve_identity_from_values(token, None, &fields).expect("identity should resolve");

        assert_eq!(identity.dingtalk_user_id, "user-id");
        assert_eq!(identity.provider_user_id.as_deref(), Some("user-id"));
        assert_eq!(identity.union_id.as_deref(), Some("union-id"));
        assert_eq!(identity.open_id.as_deref(), Some("open-id"));
        assert_eq!(identity.corp_id.as_deref(), Some("corp-id"));
    }

    #[test]
    fn resolves_identity_from_nested_user_info_when_token_lacks_identity() {
        let fields = vec!["userId".to_string(), "unionId".to_string()];
        let token = json!({ "accessToken": "secret-token" });
        let user_info = json!({
            "result": {
                "userId": "nested-user-id",
                "unionId": "nested-union-id"
            }
        });

        let identity = resolve_identity_from_values(token, Some(user_info), &fields)
            .expect("identity should resolve from user info");

        assert_eq!(identity.dingtalk_user_id, "nested-user-id");
        assert_eq!(identity.union_id.as_deref(), Some("nested-union-id"));
    }

    #[test]
    fn reports_missing_identity_field() {
        let fields = vec!["userId".to_string(), "unionId".to_string()];

        let error = resolve_identity_from_values(json!({}), None, &fields)
            .expect_err("missing identity should fail");

        assert!(matches!(error, DingTalkError::MissingIdentityField(_)));
    }

    #[test]
    fn sanitizes_token_response_without_removing_non_secret_fields() {
        let sanitized = sanitize_token_response(json!({
            "accessToken": "secret",
            "access_token": "secret",
            "refreshToken": "secret",
            "refresh_token": "secret",
            "userId": "user-id"
        }));

        assert!(sanitized.get("accessToken").is_none());
        assert!(sanitized.get("access_token").is_none());
        assert!(sanitized.get("refreshToken").is_none());
        assert!(sanitized.get("refresh_token").is_none());
        assert_eq!(
            sanitized.get("userId").and_then(Value::as_str),
            Some("user-id")
        );
    }

    #[test]
    fn percent_encoding_uses_oauth_safe_characters() {
        assert_eq!(
            percent_encode("abc XYZ-_.~/"),
            "abc%20XYZ-_.~%2F".to_string()
        );
    }

    mod department_api {
        use super::*;
        use axum::{
            Form, Json, Router,
            extract::Query,
            routing::{get, post},
        };
        use std::collections::HashMap;
        use tokio::net::TcpListener;

        #[derive(serde::Deserialize)]
        struct GetTokenQuery {
            appkey: String,
            appsecret: String,
        }

        #[derive(serde::Deserialize)]
        struct ListSubForm {
            access_token: String,
            dept_id: i64,
            language: String,
        }

        async fn start_mock_dingtalk_org(
            token_response: Value,
            listsub_responses: HashMap<i64, Value>,
        ) -> String {
            let gettoken = move |Query(query): Query<GetTokenQuery>| {
                let token_response = token_response.clone();
                async move {
                    assert_eq!(query.appkey, "client-id");
                    assert_eq!(query.appsecret, "client-secret");
                    Json(token_response)
                }
            };

            let listsub = move |Form(form): Form<ListSubForm>| {
                let listsub_responses = listsub_responses.clone();
                async move {
                    assert_eq!(form.access_token, "corp-token");
                    assert_eq!(form.language, "zh_CN");
                    let response = listsub_responses
                        .get(&form.dept_id)
                        .cloned()
                        .unwrap_or_else(|| json!({"errcode": 0, "errmsg": "ok", "result": []}));
                    Json(response)
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

        fn mock_config(mock_base_url: &str) -> DingTalkConfig {
            let mut config = test_config();
            config.corp_token_url = format!("{mock_base_url}/gettoken");
            config.department_listsub_url = format!("{mock_base_url}/listsub");
            config
        }

        fn ok_token_response() -> Value {
            json!({
                "errcode": 0,
                "errmsg": "ok",
                "access_token": "corp-token",
                "expires_in": 7200
            })
        }

        fn default_listsub_responses() -> HashMap<i64, Value> {
            HashMap::from([
                (
                    ROOT_DEPARTMENT_ID,
                    json!({
                        "errcode": 0,
                        "errmsg": "ok",
                        "result": [
                            {"dept_id": 10, "name": "总裁办", "parent_id": 1, "auto_add_user": true},
                            {"dept_id": 20, "name": "研发", "parent_id": 1}
                        ]
                    }),
                ),
                (
                    20,
                    json!({
                        "errcode": 0,
                        "errmsg": "ok",
                        "result": [{"dept_id": 21, "name": "后端", "parent_id": 20}]
                    }),
                ),
            ])
        }

        #[tokio::test]
        async fn fetches_corp_access_token() {
            let mock_base_url = start_mock_dingtalk_org(ok_token_response(), HashMap::new()).await;
            let client =
                DingTalkClient::new(mock_config(&mock_base_url)).expect("config should be valid");

            let token = client
                .fetch_corp_access_token()
                .await
                .expect("corp token should be fetched");

            assert_eq!(token, "corp-token");
        }

        #[tokio::test]
        async fn corp_token_reports_provider_errcode() {
            let mock_base_url = start_mock_dingtalk_org(
                json!({"errcode": 40089, "errmsg": "invalid credential"}),
                HashMap::new(),
            )
            .await;
            let client =
                DingTalkClient::new(mock_config(&mock_base_url)).expect("config should be valid");

            let error = client
                .fetch_corp_access_token()
                .await
                .expect_err("non-zero errcode should fail");

            assert!(matches!(
                error,
                DingTalkError::ProviderApi {
                    operation: "corp_token",
                    errcode: 40089,
                    ..
                }
            ));
        }

        #[tokio::test]
        async fn lists_sub_departments_with_form_params() {
            let mock_base_url =
                start_mock_dingtalk_org(ok_token_response(), default_listsub_responses()).await;
            let client =
                DingTalkClient::new(mock_config(&mock_base_url)).expect("config should be valid");

            let departments = client
                .list_sub_departments("corp-token", ROOT_DEPARTMENT_ID)
                .await
                .expect("sub departments should be listed");

            assert_eq!(
                departments,
                vec![
                    DingTalkDepartment {
                        dept_id: 10,
                        name: "总裁办".to_string(),
                        parent_id: Some(1),
                    },
                    DingTalkDepartment {
                        dept_id: 20,
                        name: "研发".to_string(),
                        parent_id: Some(1),
                    },
                ]
            );
        }

        #[tokio::test]
        async fn listsub_reports_provider_errcode() {
            let mock_base_url = start_mock_dingtalk_org(
                ok_token_response(),
                HashMap::from([(
                    ROOT_DEPARTMENT_ID,
                    json!({"errcode": 60011, "errmsg": "no permission"}),
                )]),
            )
            .await;
            let client =
                DingTalkClient::new(mock_config(&mock_base_url)).expect("config should be valid");

            let error = client
                .list_sub_departments("corp-token", ROOT_DEPARTMENT_ID)
                .await
                .expect_err("non-zero errcode should fail");

            assert!(matches!(
                error,
                DingTalkError::ProviderApi {
                    operation: "department_listsub",
                    errcode: 60011,
                    ..
                }
            ));
        }

        #[tokio::test]
        async fn fetches_full_department_tree_breadth_first() {
            let mock_base_url =
                start_mock_dingtalk_org(ok_token_response(), default_listsub_responses()).await;
            let client =
                DingTalkClient::new(mock_config(&mock_base_url)).expect("config should be valid");

            let departments = client
                .fetch_all_departments()
                .await
                .expect("department tree should be fetched");

            let ids: Vec<i64> = departments
                .iter()
                .map(|department| department.dept_id)
                .collect();
            assert_eq!(ids, vec![10, 20, 21]);
            assert_eq!(departments[2].name, "后端");
            assert_eq!(departments[2].parent_id, Some(20));
        }
    }
}

use serde_json::{Value, json};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::config::DingTalkConfig;

const PROVIDER: &str = "dingtalk";

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
}

#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};
use crate::publish::validate_endpoint;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKey {
    pub id: String,
    pub name: String,
    pub system: String,
    pub token_prefix: String,
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssuedServiceKey {
    pub id: String,
    pub name: String,
    pub system: String,
    pub token_prefix: String,
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
    pub key: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl std::str::FromStr for LogLevel {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            "critical" => Ok(Self::Critical),
            _ => Err(Error::Access(
                "level must be debug, info, warn, error, or critical".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogReport {
    pub id: String,
    pub key_id: String,
    pub system: String,
    pub environment: String,
    pub level: LogLevel,
    pub event: String,
    pub summary: String,
    pub redactions: i64,
    pub source_event_id: String,
    pub occurred_at: String,
    pub received_at: String,
}

pub struct ReportSubmission {
    pub environment: String,
    pub level: LogLevel,
    pub event: String,
    pub summary: String,
    pub source_event_id: String,
    pub occurred_at: DateTime<Utc>,
}

/// Issue one scoped service key. The returned `key` is available only in this response.
///
/// # Errors
///
/// Returns an error for an unsafe endpoint, invalid request, rejected session,
/// unavailable Pro entitlement, or malformed response.
pub async fn create_key(
    endpoint: &str,
    token: &str,
    name: &str,
    system: &str,
    scope: &str,
    expires_in_days: i64,
) -> Result<IssuedServiceKey> {
    request_json(
        reqwest::Client::new()
            .post(collection_url(endpoint, "/api/keys")?)
            .bearer_auth(token)
            .json(&serde_json::json!({
                "name": name,
                "system": system,
                "scope": scope,
                "expiresInDays": expires_in_days
            })),
    )
    .await
}

/// List service key metadata without returning any secret.
///
/// # Errors
///
/// Returns an error for an unsafe endpoint, rejected session, transport failure,
/// or malformed response.
pub async fn list_keys(endpoint: &str, token: &str) -> Result<Vec<ServiceKey>> {
    request_json(
        reqwest::Client::new()
            .get(collection_url(endpoint, "/api/keys")?)
            .bearer_auth(token),
    )
    .await
}

/// Revoke one service key owned by the signed-in account.
///
/// # Errors
///
/// Returns an error for an invalid identifier, unsafe endpoint, rejected request,
/// transport failure, or malformed response.
pub async fn revoke_key(endpoint: &str, token: &str, id: &str) -> Result<ServiceKey> {
    if !valid_identifier(id, 64) {
        return Err(Error::Access("invalid service key id".to_string()));
    }
    let mut url = collection_url(endpoint, "/api/keys")?;
    url.set_path(&format!("{}/{}", url.path().trim_end_matches('/'), id));
    request_json(reqwest::Client::new().delete(url).bearer_auth(token)).await
}

/// Submit one sanitized remote-system report with a service key.
///
/// # Errors
///
/// Returns an error for an unsafe endpoint, rejected key, invalid report,
/// transport failure, or malformed response.
pub async fn create_report(
    endpoint: &str,
    service_key: &str,
    report: &ReportSubmission,
) -> Result<LogReport> {
    request_json(
        reqwest::Client::new()
            .post(collection_url(endpoint, "/api/log-reports")?)
            .bearer_auth(service_key)
            .json(&serde_json::json!({
                "environment": report.environment,
                "level": report.level,
                "event": report.event,
                "summary": report.summary,
                "sourceEventId": report.source_event_id,
                "occurredAt": report.occurred_at
            })),
    )
    .await
}

/// List recent reports visible to the supplied user session or service key.
///
/// # Errors
///
/// Returns an error for unsafe query values, rejected credentials, transport
/// failure, or malformed response.
pub async fn list_reports(
    endpoint: &str,
    token: &str,
    system: Option<&str>,
    limit: i64,
) -> Result<Vec<LogReport>> {
    if !(1..=200).contains(&limit) {
        return Err(Error::Access("limit must be between 1 and 200".to_string()));
    }
    let mut url = collection_url(endpoint, "/api/log-reports")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("limit", &limit.to_string());
        if let Some(system) = system {
            query.append_pair("system", system);
        }
    }
    request_json(reqwest::Client::new().get(url).bearer_auth(token)).await
}

async fn request_json<T: for<'de> Deserialize<'de>>(request: reqwest::RequestBuilder) -> Result<T> {
    let response = request
        .send()
        .await
        .map_err(|error| Error::Access(error.to_string()))?;
    if !response.status().is_success() {
        return Err(Error::Access(format!(
            "server returned {}",
            response.status()
        )));
    }
    response
        .json()
        .await
        .map_err(|error| Error::Access(error.to_string()))
}

fn collection_url(endpoint: &str, expected_path: &str) -> Result<Url> {
    let url = validate_endpoint(endpoint)?;
    if url.path().trim_end_matches('/') != expected_path
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Endpoint(format!(
            "endpoint must end with {expected_path} and cannot contain a query or fragment"
        )));
    }
    Ok(url)
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_endpoints_are_https_and_path_bounded() {
        assert!(collection_url("https://footon.dev/api/keys", "/api/keys").is_ok());
        assert!(collection_url("http://example.com/api/keys", "/api/keys").is_err());
        assert!(collection_url("https://footon.dev/api/shares", "/api/keys").is_err());
        assert!(collection_url("https://footon.dev/api/keys?secret=x", "/api/keys").is_err());
    }

    #[test]
    fn report_levels_use_the_wire_values() {
        assert!(matches!(
            "critical".parse::<LogLevel>(),
            Ok(LogLevel::Critical)
        ));
        assert!("fatal".parse::<LogLevel>().is_err());
        assert_eq!(serde_json::to_value(LogLevel::Warn).expect("warn"), "warn");
    }
}

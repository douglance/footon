use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};
use crate::model::{Draft, Share};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishResponse {
    pub id: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
}

/// Validate a sanitized draft and stamp the explicit approval time.
///
/// # Errors
///
/// Returns an error when the schema, title, message count, or size is invalid.
pub fn build_share(draft: Draft, approved_at: DateTime<Utc>) -> Result<Share> {
    footon_core::validate::build_share(draft, approved_at).map_err(Into::into)
}

/// Validate that publishing uses HTTPS, except for loopback integration tests.
///
/// # Errors
///
/// Returns an error for malformed URLs or non-HTTPS remote endpoints.
pub fn validate_endpoint(endpoint: &str) -> Result<Url> {
    let url = Url::parse(endpoint).map_err(|error| Error::Endpoint(error.to_string()))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !loopback {
        return Err(Error::Endpoint(
            "HTTPS is required outside loopback tests".to_string(),
        ));
    }
    Ok(url)
}

/// Publish an approved share with bearer authentication.
///
/// # Errors
///
/// Returns an error for unsafe endpoints, transport failures, non-201 responses,
/// or malformed response bodies.
pub async fn send(endpoint: &str, token: &str, share: &Share) -> Result<PublishResponse> {
    let url = validate_endpoint(endpoint)?;
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .json(share)
        .send()
        .await
        .map_err(|error| Error::Publish(error.to_string()))?;
    if response.status() != reqwest::StatusCode::CREATED {
        return Err(Error::Publish(format!(
            "server returned {}",
            response.status()
        )));
    }
    response
        .json()
        .await
        .map_err(|error| Error::Publish(error.to_string()))
}

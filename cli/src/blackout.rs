use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use footon_core::blackout::{BlackoutOutcome, blackout};
use footon_core::validate::validate_draft;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::draft;
use crate::error::{Error, Result};
use crate::publish::validate_endpoint;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LocalBlackoutOutput {
    pub draft_path: PathBuf,
    pub message: usize,
    pub replacement: &'static str,
    pub redactions: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShareBlackoutResponse {
    pub id: String,
    pub url: String,
    pub updated_at: DateTime<Utc>,
    pub message: usize,
    pub replacement: String,
    pub redactions: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareBlackoutRequest<'a> {
    message: usize,
    text: &'a str,
}

/// Black out one exact substring in a sanitized local draft.
///
/// # Errors
///
/// Returns an error when the draft cannot be read, changed, validated, or written.
pub fn local(path: &Path, message: usize, text: &str) -> Result<LocalBlackoutOutput> {
    let mut value = draft::read(path)?;
    let outcome = blackout(&mut value.messages, &mut value.report, message, text)?;
    validate_draft(&value)?;
    draft::write(path, &value)?;
    Ok(local_output(path, &outcome))
}

/// Black out one exact substring in an owner-controlled live share.
///
/// # Errors
///
/// Returns an error for unsafe endpoints, invalid share references, transport
/// failures, rejected updates, or malformed response bodies.
pub async fn remote(
    endpoint: &str,
    token: &str,
    share: &str,
    message: usize,
    text: &str,
) -> Result<ShareBlackoutResponse> {
    let url = blackout_url(endpoint, share)?;
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&ShareBlackoutRequest { message, text })
        .send()
        .await
        .map_err(|error| Error::Publish(error.to_string()))?;
    if response.status() != reqwest::StatusCode::OK {
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

fn local_output(path: &Path, outcome: &BlackoutOutcome) -> LocalBlackoutOutput {
    LocalBlackoutOutput {
        draft_path: path.to_path_buf(),
        message: outcome.message,
        replacement: outcome.replacement,
        redactions: outcome.redactions,
    }
}

fn blackout_url(endpoint: &str, share: &str) -> Result<Url> {
    let mut endpoint = validate_endpoint(endpoint)?;
    let id = share_id(&endpoint, share)?;
    let path = format!("{}/{id}/blackouts", endpoint.path().trim_end_matches('/'));
    endpoint.set_path(&path);
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    Ok(endpoint)
}

fn share_id(endpoint: &Url, share: &str) -> Result<String> {
    if valid_share_id(share) {
        return Ok(share.to_string());
    }
    let url = validate_endpoint(share)?;
    if origin(&url) != origin(endpoint) || url.query().is_some() || url.fragment().is_some() {
        return Err(Error::Endpoint(
            "share URL must use the configured Footon origin".to_string(),
        ));
    }
    let segments = match url.path_segments() {
        Some(parts) => parts.collect::<Vec<_>>(),
        None => Vec::new(),
    };
    match segments.as_slice() {
        ["s", id] if valid_share_id(id) => Ok((*id).to_string()),
        _ => Err(Error::Endpoint(
            "share must be a Footon share ID or /s/{id} URL".to_string(),
        )),
    }
}

fn origin(url: &Url) -> (&str, Option<&str>, Option<u16>) {
    (url.scheme(), url.host_str(), url.port_or_known_default())
}

fn valid_share_id(value: &str) -> bool {
    (20..=40).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

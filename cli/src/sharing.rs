#![forbid(unsafe_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};
use crate::publish::{GeneralAccess, validate_endpoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShareRole {
    Owner,
    Editor,
    Viewer,
}

impl std::str::FromStr for ShareRole {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "editor" => Ok(Self::Editor),
            "viewer" => Ok(Self::Viewer),
            _ => Err(Error::Access("role must be viewer or editor".to_string())),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShareSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub general_access: GeneralAccess,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ShareOwner {
    pub email: String,
    pub role: ShareRole,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShareMember {
    pub id: String,
    pub email: String,
    pub role: ShareRole,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccess {
    pub share_id: String,
    pub general_access: GeneralAccess,
    pub actor_role: ShareRole,
    pub owner: ShareOwner,
    pub members: Vec<ShareMember>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShareMetadata {
    pub id: String,
    pub title: String,
    pub general_access: GeneralAccess,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransferResponse {
    pub id: String,
    pub owner: ShareOwner,
    pub general_access: GeneralAccess,
}

/// List the authenticated user's active shares.
///
/// # Errors
///
/// Returns an error for unsafe endpoints, transport failures, rejected requests,
/// or malformed responses.
pub async fn list(endpoint: &str, token: &str) -> Result<Vec<ShareSummary>> {
    request_json(
        reqwest::Client::new()
            .get(collection_url(endpoint)?)
            .bearer_auth(token),
    )
    .await
}

/// Read the visibility, owner, and member roles for one share.
///
/// # Errors
///
/// Returns an error for an invalid share, unsafe endpoint, rejected request, or
/// malformed response.
pub async fn access(endpoint: &str, token: &str, share: &str) -> Result<ShareAccess> {
    let url = item_url(endpoint, share, Some("access"))?;
    request_json(reqwest::Client::new().get(url).bearer_auth(token)).await
}

/// Rename one share without changing its access setting.
///
/// # Errors
///
/// Returns an error for invalid input, transport failures, rejected requests, or
/// malformed responses.
pub async fn rename(
    endpoint: &str,
    token: &str,
    share: &str,
    title: &str,
) -> Result<ShareMetadata> {
    let url = item_url(endpoint, share, None)?;
    request_json(
        reqwest::Client::new()
            .patch(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "title": title })),
    )
    .await
}

/// Change one share between public and private access.
///
/// # Errors
///
/// Returns an error for invalid input, transport failures, rejected requests, or
/// malformed responses.
pub async fn visibility(
    endpoint: &str,
    token: &str,
    share: &str,
    general_access: GeneralAccess,
) -> Result<ShareMetadata> {
    let url = item_url(endpoint, share, None)?;
    request_json(
        reqwest::Client::new()
            .patch(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "generalAccess": general_access })),
    )
    .await
}

/// Grant or replace a Viewer or Editor role on a private share.
///
/// # Errors
///
/// Returns an error for an invalid role or email, rejected request, transport
/// failure, or malformed response.
pub async fn grant(
    endpoint: &str,
    token: &str,
    share: &str,
    email: &str,
    role: ShareRole,
) -> Result<ShareMember> {
    if role == ShareRole::Owner {
        return Err(Error::Access("role must be viewer or editor".to_string()));
    }
    let url = item_url(endpoint, share, Some("members"))?;
    request_json(
        reqwest::Client::new()
            .put(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "email": email, "role": role })),
    )
    .await
}

/// Remove one member identified by email from a private share.
///
/// # Errors
///
/// Returns an error when the member is absent or the server rejects or cannot
/// complete the request.
pub async fn remove(endpoint: &str, token: &str, share: &str, email: &str) -> Result<ShareMember> {
    let access = access(endpoint, token, share).await?;
    let normalized = email.trim().to_ascii_lowercase();
    let member = access
        .members
        .into_iter()
        .find(|member| member.email == normalized)
        .ok_or_else(|| Error::Access("member not found".to_string()))?;
    let url = item_url(endpoint, share, Some(&format!("members/{}", member.id)))?;
    let response = reqwest::Client::new()
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| Error::Access(error.to_string()))?;
    require_success(response)?;
    Ok(member)
}

/// Transfer ownership of one share to an existing Footon identity.
///
/// # Errors
///
/// Returns an error for invalid input, transport failures, rejected requests, or
/// malformed responses.
pub async fn transfer(
    endpoint: &str,
    token: &str,
    share: &str,
    email: &str,
) -> Result<TransferResponse> {
    let url = item_url(endpoint, share, Some("transfer"))?;
    request_json(
        reqwest::Client::new()
            .post(url)
            .bearer_auth(token)
            .json(&serde_json::json!({ "email": email })),
    )
    .await
}

async fn request_json<T: for<'de> Deserialize<'de>>(request: reqwest::RequestBuilder) -> Result<T> {
    let response = request
        .send()
        .await
        .map_err(|error| Error::Access(error.to_string()))?;
    let response = require_success(response)?;
    response
        .json()
        .await
        .map_err(|error| Error::Access(error.to_string()))
}

fn require_success(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(Error::Access(format!(
            "server returned {}",
            response.status()
        )))
    }
}

fn collection_url(endpoint: &str) -> Result<Url> {
    let url = validate_endpoint(endpoint)?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::Endpoint(
            "share endpoint cannot contain a query or fragment".to_string(),
        ));
    }
    Ok(url)
}

fn item_url(endpoint: &str, share: &str, suffix: Option<&str>) -> Result<Url> {
    let mut url = collection_url(endpoint)?;
    let id = share_id(share)?;
    let base = url.path().trim_end_matches('/');
    let path = suffix.map_or_else(
        || format!("{base}/{id}"),
        |suffix| format!("{base}/{id}/{suffix}"),
    );
    url.set_path(&path);
    Ok(url)
}

fn share_id(value: &str) -> Result<String> {
    if valid_share_id(value) {
        return Ok(value.to_string());
    }
    let url = crate::fetch::validate_share_url(value)?;
    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match segments.as_slice() {
        ["s", id] if valid_share_id(id) => Ok((*id).to_string()),
        _ => Err(Error::Endpoint(
            "share must be a Footon share ID or /s/{id} URL".to_string(),
        )),
    }
}

fn valid_share_id(value: &str) -> bool {
    (20..=40).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_only_bounded_same_origin_management_urls() {
        let id = "abcdefghijklmnopqrstuvwx";
        assert_eq!(
            item_url("https://footon.dev/api/shares", id, Some("access"))
                .expect("url")
                .as_str(),
            "https://footon.dev/api/shares/abcdefghijklmnopqrstuvwx/access"
        );
        assert_eq!(
            share_id("https://footon.dev/s/abcdefghijklmnopqrstuvwx").expect("id"),
            id
        );
        assert!(item_url("http://example.com/api/shares", id, None).is_err());
    }

    #[test]
    fn roles_and_visibility_use_the_public_api_values() {
        assert_eq!(
            "viewer".parse::<ShareRole>().expect("viewer"),
            ShareRole::Viewer
        );
        assert_eq!(
            "EDITOR".parse::<ShareRole>().expect("editor"),
            ShareRole::Editor
        );
        assert!("owner".parse::<ShareRole>().is_err());
        assert_eq!(
            serde_json::to_value(GeneralAccess::Private).expect("private"),
            "private"
        );
    }
}

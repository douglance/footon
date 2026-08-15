use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};

const KEYRING_SERVICE: &str = "dev.footon.cli";
const REFRESH_MARGIN_SECONDS: i64 = 60;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredSession {
    origin: String,
    email: String,
    client_id: String,
    access_token: String,
    refresh_token: String,
    scope: String,
    resource: String,
    expires_at: i64,
}

impl StoredSession {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        origin: String,
        email: String,
        client_id: String,
        access_token: String,
        refresh_token: String,
        scope: String,
        resource: String,
        expires_at: i64,
    ) -> Self {
        Self {
            origin,
            email,
            client_id,
            access_token,
            refresh_token,
            scope,
            resource,
            expires_at,
        }
    }

    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    #[must_use]
    pub fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    #[must_use]
    pub const fn expires_at(&self) -> i64 {
        self.expires_at
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignoutResponse {
    pub signed_out: bool,
    pub email: String,
    pub origin: String,
}

pub trait CredentialStore {
    /// Load the session for one normalized OAuth origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the credential store is unavailable or corrupt.
    fn load(&self, origin: &str) -> Result<Option<StoredSession>>;

    /// Create or replace the session for its OAuth origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the credential store cannot persist the session.
    fn save(&self, session: &StoredSession) -> Result<()>;

    /// Delete the session for one normalized OAuth origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the credential store cannot delete the session.
    fn delete(&self, origin: &str) -> Result<()>;
}

pub struct KeyringStore;

impl CredentialStore for KeyringStore {
    fn load(&self, origin: &str) -> Result<Option<StoredSession>> {
        let entry = keyring_entry(origin)?;
        match entry.get_password() {
            Ok(value) => serde_json::from_str(&value).map(Some).map_err(Error::from),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(session_error(error)),
        }
    }

    fn save(&self, session: &StoredSession) -> Result<()> {
        let value = serde_json::to_string(session)?;
        keyring_entry(&session.origin)?
            .set_password(&value)
            .map_err(session_error)
    }

    fn delete(&self, origin: &str) -> Result<()> {
        match keyring_entry(origin)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(session_error(error)),
        }
    }
}

/// Resolve the token for an authenticated endpoint, refreshing a stored session
/// when necessary. An explicit environment token always wins and is never read
/// from or written to the credential store.
///
/// # Errors
///
/// Returns an error for an unsafe endpoint, unavailable or missing credentials,
/// a rejected refresh, or a malformed server response.
pub async fn resolve_access_token(
    endpoint: &str,
    environment_token: Option<&str>,
    store: &impl CredentialStore,
) -> Result<String> {
    if let Some(token) = environment_token.filter(|token| !token.is_empty()) {
        return Ok(token.to_string());
    }

    let origin = endpoint_origin(endpoint)?;
    let mut session = store.load(&origin)?.ok_or_else(|| {
        Error::Session("not signed in; run `footon signin <email>` and try again".to_string())
    })?;
    if session.expires_at > Utc::now().timestamp() + REFRESH_MARGIN_SECONDS {
        return Ok(session.access_token);
    }

    refresh(&mut session).await?;
    let token = session.access_token.clone();
    store.save(&session)?;
    Ok(token)
}

/// Revoke the stored refresh token and then remove the local session.
///
/// # Errors
///
/// Returns an error when there is no session, the origin is unsafe, revocation
/// is rejected, or the credential store cannot delete the session.
pub async fn sign_out(origin: &str, store: &impl CredentialStore) -> Result<SignoutResponse> {
    let origin = endpoint_origin(origin)?;
    let session = store.load(&origin)?.ok_or_else(|| {
        Error::Session("not signed in; there is no local session to revoke".to_string())
    })?;
    let response = reqwest::Client::new()
        .post(endpoint(&origin, "/oauth/revoke")?)
        .form(&[
            ("token", session.refresh_token.as_str()),
            ("token_type_hint", "refresh_token"),
        ])
        .send()
        .await
        .map_err(session_error)?;
    if !response.status().is_success() {
        return Err(Error::Session(format!(
            "token revocation was rejected with {}",
            response.status()
        )));
    }
    store.delete(&origin)?;
    Ok(SignoutResponse {
        signed_out: true,
        email: session.email,
        origin,
    })
}

async fn refresh(session: &mut StoredSession) -> Result<()> {
    let token_endpoint = endpoint(&session.origin, "/oauth/token")?;
    let response = reqwest::Client::new()
        .post(token_endpoint)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", session.refresh_token.as_str()),
            ("client_id", session.client_id.as_str()),
            ("resource", session.resource.as_str()),
        ])
        .send()
        .await
        .map_err(session_error)?;
    if !response.status().is_success() {
        return Err(Error::Session(format!(
            "token refresh was rejected with {}",
            response.status()
        )));
    }
    let token = response
        .json::<RefreshResponse>()
        .await
        .map_err(session_error)?;
    session.access_token = token.access_token;
    session.refresh_token = token.refresh_token;
    session.scope = token.scope;
    session.expires_at = Utc::now().timestamp() + token.expires_in;
    Ok(())
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}

fn keyring_entry(origin: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, origin).map_err(session_error)
}

fn endpoint_origin(value: &str) -> Result<String> {
    let url = safe_url(value)?;
    let host = url
        .host_str()
        .ok_or_else(|| Error::Endpoint("endpoint must include a host".to_string()))?;
    let mut origin = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        origin.push(':');
        origin.push_str(&port.to_string());
    }
    Ok(origin)
}

fn endpoint(origin: &str, path: &str) -> Result<Url> {
    safe_url(origin)?.join(path).map_err(session_error)
}

fn safe_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(session_error)?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::Endpoint(
            "only HTTP(S) endpoints are supported".to_string(),
        ));
    }
    if url.scheme() != "https" && !loopback {
        return Err(Error::Endpoint(
            "HTTPS is required outside loopback tests".to_string(),
        ));
    }
    Ok(url)
}

fn session_error(error: impl std::fmt::Display) -> Error {
    Error::Session(error.to_string())
}

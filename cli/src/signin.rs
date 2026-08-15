use std::io::{BufRead, Write};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use rand::RngCore;
use reqwest::header::LOCATION;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::{Error, Result};
use crate::session::{CredentialStore, StoredSession};

const REDIRECT_URI: &str = "http://127.0.0.1/callback";
const SCOPE: &str = "shares:read shares:write";

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SigninResponse {
    pub signed_in: bool,
    pub email: String,
    pub origin: String,
    pub scope: String,
    pub expires_at: i64,
}

pub struct CompletedSignin {
    session: StoredSession,
}

pub struct PendingSignin {
    client: reqwest::Client,
    origin: Url,
    email: String,
    client_id: String,
    ticket: String,
    verifier: String,
    state: String,
    resource: String,
}

#[derive(Deserialize)]
struct RegisterResponse {
    client_id: String,
}

#[derive(Deserialize)]
struct AuthRequestResponse {
    ticket: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}

/// Register a terminal OAuth client and send a code to `email`.
///
/// # Errors
///
/// Returns an error for an unsafe origin, transport failure, rejected request,
/// or malformed server response.
pub async fn begin(origin: &str, email: &str) -> Result<PendingSignin> {
    let origin = validate_origin(origin)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(signin_error)?;
    let client_id = register_client(&client, &origin).await?;

    let verifier = random_url_token(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_url_token(24);
    let resource = endpoint(&origin, "/mcp")?.to_string();
    let ticket = request_code(
        &client, &origin, email, &client_id, &challenge, &state, &resource,
    )
    .await?;

    Ok(PendingSignin {
        client,
        origin,
        email: email.to_string(),
        client_id,
        ticket,
        verifier,
        state,
        resource,
    })
}

async fn register_client(client: &reqwest::Client, origin: &Url) -> Result<String> {
    let response = client
        .post(endpoint(origin, "/oauth/register")?)
        .json(&serde_json::json!({
            "client_name": "Footon CLI",
            "redirect_uris": [REDIRECT_URI],
            "scope": SCOPE,
        }))
        .send()
        .await
        .map_err(signin_error)?;
    Ok(success_json::<RegisterResponse>(response).await?.client_id)
}

async fn request_code(
    client: &reqwest::Client,
    origin: &Url,
    email: &str,
    client_id: &str,
    challenge: &str,
    state: &str,
    resource: &str,
) -> Result<String> {
    let response = client
        .post(endpoint(origin, "/auth/request")?)
        .json(&serde_json::json!({
            "email": email,
            "client_id": client_id,
            "redirect_uri": REDIRECT_URI,
            "scope": SCOPE,
            "state": state,
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "resource": resource,
        }))
        .send()
        .await
        .map_err(signin_error)?;
    Ok(success_json::<AuthRequestResponse>(response).await?.ticket)
}

impl PendingSignin {
    /// Verify the emailed code and exchange the resulting authorization code.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid code, transport failure, rejected
    /// request, state mismatch, missing authorization code, or malformed token
    /// response.
    pub async fn complete(self, code: &str) -> Result<CompletedSignin> {
        let code = normalize_code(code)?;
        let authorization_code = self.verify_code(code).await?;
        self.exchange_token(&authorization_code).await
    }

    async fn verify_code(&self, code: &str) -> Result<String> {
        let response = self
            .client
            .post(endpoint(&self.origin, "/auth/verify")?)
            .json(&serde_json::json!({ "ticket": self.ticket, "code": code }))
            .send()
            .await
            .map_err(signin_error)?;
        if !response.status().is_redirection() {
            return Err(response_error(response).await);
        }
        let redirect = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| Error::Signin("verification response omitted Location".to_string()))?;
        let redirect = Url::parse(redirect)
            .map_err(|error| Error::Signin(format!("invalid verification redirect: {error}")))?;
        if query(&redirect, "state").as_deref() != Some(self.state.as_str()) {
            return Err(Error::Signin(
                "verification redirect state did not match".to_string(),
            ));
        }
        query(&redirect, "code").ok_or_else(|| {
            Error::Signin("verification redirect omitted authorization code".to_string())
        })
    }

    async fn exchange_token(self, authorization_code: &str) -> Result<CompletedSignin> {
        let response = self
            .client
            .post(endpoint(&self.origin, "/oauth/token")?)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", authorization_code),
                ("redirect_uri", REDIRECT_URI),
                ("client_id", self.client_id.as_str()),
                ("code_verifier", self.verifier.as_str()),
                ("resource", self.resource.as_str()),
            ])
            .send()
            .await
            .map_err(signin_error)?;
        let tokens = success_json::<TokenResponse>(response).await?;
        let expires_at = Utc::now().timestamp() + tokens.expires_in;
        Ok(CompletedSignin {
            session: StoredSession::new(
                self.origin.as_str().trim_end_matches('/').to_string(),
                self.email,
                self.client_id,
                tokens.access_token,
                tokens.refresh_token,
                tokens.scope,
                self.resource,
                expires_at,
            ),
        })
    }
}

impl CompletedSignin {
    /// Persist the session in the configured credential store and return only
    /// non-secret account metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when the credential store cannot save the session.
    pub fn save(self, store: &impl CredentialStore) -> Result<SigninResponse> {
        let response = SigninResponse {
            signed_in: true,
            email: self.session.email().to_string(),
            origin: self.session.origin().to_string(),
            scope: self.session.scope().to_string(),
            expires_at: self.session.expires_at(),
        };
        store.save(&self.session)?;
        Ok(response)
    }
}

/// Prompt for a one-time code without placing it in process arguments.
///
/// # Errors
///
/// Returns an error when the prompt cannot be written, stdin cannot be read,
/// or the input is not exactly six ASCII digits.
pub fn read_code(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    email: &str,
) -> Result<String> {
    write!(writer, "Code sent to {email}.\nEnter the six-digit code: ").map_err(signin_error)?;
    writer.flush().map_err(signin_error)?;
    let mut code = String::new();
    reader.read_line(&mut code).map_err(signin_error)?;
    normalize_code(&code).map(str::to_string)
}

fn validate_origin(origin: &str) -> Result<Url> {
    let url = Url::parse(origin).map_err(signin_error)?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::Signin(
            "only HTTP(S) OAuth origins are supported".to_string(),
        ));
    }
    if url.scheme() != "https" && !loopback {
        return Err(Error::Signin(
            "HTTPS is required outside loopback tests".to_string(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::Signin(
            "OAuth origin cannot include a query or fragment".to_string(),
        ));
    }
    Ok(url)
}

fn endpoint(origin: &Url, path: &str) -> Result<Url> {
    origin.join(path).map_err(signin_error)
}

fn random_url_token(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    rand::rngs::OsRng.fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn normalize_code(code: &str) -> Result<&str> {
    let code = code.trim();
    if code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(code)
    } else {
        Err(Error::Signin(
            "code must contain exactly six digits".to_string(),
        ))
    }
}

fn query(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
}

async fn success_json<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> Result<T> {
    if !response.status().is_success() {
        return Err(response_error(response).await);
    }
    response.json().await.map_err(signin_error)
}

async fn response_error(response: reqwest::Response) -> Error {
    let status = response.status();
    let detail = response.text().await.unwrap_or_default();
    let detail = detail.trim();
    if detail.is_empty() {
        Error::Signin(format!("server returned {status}"))
    } else {
        Error::Signin(format!("server returned {status}: {detail}"))
    }
}

fn signin_error(error: impl std::fmt::Display) -> Error {
    Error::Signin(error.to_string())
}

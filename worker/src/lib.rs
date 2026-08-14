#![forbid(unsafe_code)]

use footon_core::accept::{ContentType, negotiate};
use footon_core::markdown::messages_to_markdown;
use footon_core::model::{Message, Share, ShareDocument, ShareRecord, validate_share};
use footon_core::validate::validate_share as validate_safe_share;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use worker::Method;
use worker::d1::D1Type;
use worker::email::SendEmailBuilder;
use worker::{
    Context, Env, Fetch, Headers, Request, RequestInit, Response, Result, ScheduledEvent,
};

use topcoat::context::Cx;
use topcoat::router::{
    Body as TopcoatBody, Method as HttpMethod, Path, RouteFn, RouteFuture, Router,
};

mod viewer;

use viewer::{VIEWER_JS, viewer_page};

const STYLE: &str = include_str!(concat!(env!("OUT_DIR"), "/tailwind.css"));
const ORIGIN: &str = "https://footon.dev";
const ACCESS_TTL_SECONDS: i64 = 3_600;
const REFRESH_TTL_SECONDS: i64 = 2_592_000;
const CLIENT_TTL_SECONDS: i64 = 7_776_000;
const CODE_TTL_SECONDS: i64 = 300;
const MAGIC_TTL_SECONDS: i64 = 600;

#[worker::event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let route_request = http::Request::builder()
        .method(
            http::Method::from_bytes(req.method().to_string().as_bytes())
                .map_err(|error| worker::Error::RustError(error.to_string()))?,
        )
        .uri(req.url()?.as_str())
        .body(TopcoatBody::empty())?;
    let route_response = router().handle(route_request).await;
    if matches!(
        route_response.status(),
        http::StatusCode::NOT_FOUND | http::StatusCode::METHOD_NOT_ALLOWED
    ) {
        return Response::error(
            route_response
                .status()
                .canonical_reason()
                .unwrap_or("request rejected"),
            route_response.status().as_u16(),
        );
    }
    let mut req = req;
    match handle(&mut req, &env).await {
        Ok(mut response) => {
            security_headers(&mut response)?;
            Ok(response)
        }
        Err(error) => internal_error(&error),
    }
}

#[worker::event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: worker::ScheduleContext) {
    if let Ok(db) = env.d1("DB") {
        let now = now_string();
        for sql in [
            "DELETE FROM oauth_codes_v2 WHERE expires_at < ?1 OR used_at IS NOT NULL",
            "DELETE FROM oauth_magic_links_v2 WHERE expires_at < ?1 OR consumed_at IS NOT NULL",
            "DELETE FROM oauth_clients_v2 WHERE expires_at < ?1",
            "DELETE FROM oauth_access_tokens_v2 WHERE expires_at < ?1 OR revoked_at IS NOT NULL",
            "DELETE FROM oauth_refresh_tokens_v2 WHERE expires_at < ?1 OR revoked_at IS NOT NULL",
        ] {
            if let Ok(stmt) = db.prepare(sql).bind_refs(&[D1Type::Text(&now)]) {
                let _ = stmt.run().await;
            }
        }
    }
}

async fn handle(req: &mut Request, env: &Env) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    if method == Method::Get && path == "/" {
        return html_response(home_page());
    }
    if method == Method::Get && path == "/install" {
        return html_response(install_page());
    }
    if method == Method::Get && path == "/connect" {
        return html_response(connect_page());
    }
    if method == Method::Get && path == "/style.css" {
        return css_response(STYLE);
    }
    if method == Method::Get && path == "/viewer.js" {
        return javascript_response(VIEWER_JS);
    }
    if method == Method::Get && path == "/.well-known/oauth-authorization-server" {
        return json_response(&authorization_server_metadata());
    }
    if method == Method::Get && path == "/.well-known/oauth-protected-resource/mcp" {
        return json_response(&protected_resource_metadata());
    }
    if method == Method::Post && path == "/oauth/register" {
        return oauth_register(req, env).await;
    }
    if method == Method::Get && path == "/authorize" {
        return authorize_response(authorize_page(&url, env));
    }
    if method == Method::Post && path == "/auth/request" {
        return auth_request(req, env, &url).await;
    }
    if method == Method::Get && path == "/auth/verify" {
        return auth_verify(env, &url).await;
    }
    if method == Method::Post && path == "/oauth/token" {
        return oauth_token(req, env).await;
    }
    if method == Method::Post && path == "/oauth/revoke" {
        return oauth_revoke(req, env).await;
    }
    if method == Method::Post && path == "/api/shares" {
        return api_create_share(req, env).await;
    }
    if method == Method::Get && path == "/api/shares" {
        return api_list_shares(req, env).await;
    }
    if method == Method::Delete && path.starts_with("/api/shares/") {
        return api_revoke_share(req, env, path.trim_start_matches("/api/shares/")).await;
    }
    if method == Method::Post && path == "/mcp" {
        return mcp(req, env).await;
    }
    if method == Method::Get && path.starts_with("/s/") {
        return public_share(req, env, path.trim_start_matches("/s/")).await;
    }

    Response::error("not found", 404)
}

fn router() -> Router {
    let mut builder = Router::builder();
    for (method, path) in [
        (HttpMethod::GET, "/"),
        (HttpMethod::GET, "/install"),
        (HttpMethod::GET, "/connect"),
        (HttpMethod::GET, "/style.css"),
        (HttpMethod::GET, "/viewer.js"),
        (HttpMethod::GET, "/.well-known/oauth-authorization-server"),
        (HttpMethod::GET, "/.well-known/oauth-protected-resource/mcp"),
        (HttpMethod::POST, "/oauth/register"),
        (HttpMethod::GET, "/authorize"),
        (HttpMethod::POST, "/auth/request"),
        (HttpMethod::GET, "/auth/verify"),
        (HttpMethod::POST, "/oauth/token"),
        (HttpMethod::POST, "/oauth/revoke"),
        (HttpMethod::POST, "/api/shares"),
        (HttpMethod::GET, "/api/shares"),
        (HttpMethod::DELETE, "/api/shares/{id}"),
        (HttpMethod::POST, "/mcp"),
        (HttpMethod::GET, "/s/{id}"),
    ] {
        builder = builder.route(RouteFn::new(
            method,
            std::borrow::Cow::Owned(Path::new(path).to_owned()),
            topcoat_match,
        ));
    }
    builder.build()
}

fn topcoat_match(_cx: &Cx, _body: TopcoatBody) -> RouteFuture<'_> {
    Box::pin(async move {
        http::Response::builder()
            .status(http::StatusCode::NO_CONTENT)
            .body(TopcoatBody::empty())
            .map_err(Into::into)
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShareInput {
    schema_version: String,
    title: String,
    approved_at: chrono::DateTime<chrono::Utc>,
    messages: Vec<Message>,
    report: footon_core::model::Report,
}

async fn api_create_share(req: &mut Request, env: &Env) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !has_scope(&user, "shares:write") {
        return Response::error("missing scope: shares:write", 403);
    }
    let input = req.json::<ShareInput>().await?;
    let share = Share {
        schema_version: input.schema_version,
        title: input.title,
        approved_at: input.approved_at,
        messages: input.messages,
        report: input.report,
    };
    if share.schema_version != footon_core::model::SCHEMA_VERSION {
        return Response::error("new shares must use footon.share.v2", 400);
    }
    if let Err(error) = validate_share(&share) {
        return Response::error(error.to_string(), 400);
    }
    if let Err(error) = validate_safe_share(&share) {
        return Response::error(error.to_string(), 400);
    }
    let id = token(18);
    let now = now_string();
    let document_json = serde_json::to_string(&share)?;
    let db = env.d1("DB")?;
    db.prepare(
        "INSERT INTO shares (id, owner_id, title, document_json, created_at, revoked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
    )
    .bind_refs(&[
        D1Type::Text(&id),
        D1Type::Text(&user.user_id),
        D1Type::Text(&share.title),
        D1Type::Text(&document_json),
        D1Type::Text(&now),
    ])?
    .run()
    .await?;

    json_response_with_status(
        &CreateShareResponse {
            id: id.clone(),
            url: format!("{ORIGIN}/s/{id}"),
            created_at: now,
        },
        201,
    )
}

async fn api_list_shares(req: &Request, env: &Env) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !has_scope(&user, "shares:read") {
        return Response::error("missing scope: shares:read", 403);
    }
    let db = env.d1("DB")?;
    let rows = db
        .prepare(
            "SELECT id, title, created_at
             FROM shares
             WHERE owner_id = ?1 AND revoked_at IS NULL
             ORDER BY created_at DESC
             LIMIT 50",
        )
        .bind_refs(&[D1Type::Text(&user.user_id)])?
        .all()
        .await?
        .results::<ListShareRow>()?;
    json_response(&rows)
}

async fn api_revoke_share(req: &Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !has_scope(&user, "shares:write") {
        return Response::error("missing scope: shares:write", 403);
    }
    validate_share_id(id)?;
    env.d1("DB")?
        .prepare("UPDATE shares SET revoked_at = ?1 WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL")
        .bind_refs(&[
            D1Type::Text(&now_string()),
            D1Type::Text(id),
            D1Type::Text(&user.user_id),
        ])?
        .run()
        .await?;
    Response::empty()
}

async fn public_share(req: &Request, env: &Env, id: &str) -> Result<Response> {
    validate_share_id(id)?;
    let Some(record) = load_share(env, id).await? else {
        return Response::error("not found", 404);
    };
    match negotiate(req.headers().get("Accept")?.as_deref()) {
        None => Response::error("not acceptable", 406),
        Some(ContentType::Markdown) => markdown_response(&messages_to_markdown(&record.document)),
        Some(ContentType::Html) => {
            let text_mode = req
                .url()?
                .query_pairs()
                .any(|(key, value)| key == "view" && value == "text");
            html_response(viewer_page(&record, text_mode))
        }
    }
}

async fn load_share(env: &Env, id: &str) -> Result<Option<ShareRecord>> {
    #[derive(Deserialize)]
    struct Row {
        id: String,
        owner_id: String,
        title: String,
        document_json: String,
        created_at: String,
        revoked_at: Option<String>,
    }
    let db = env.d1("DB")?;
    let row = db
        .prepare(
            "SELECT id, owner_id, title, document_json, created_at, revoked_at
             FROM shares
             WHERE id = ?1 AND revoked_at IS NULL",
        )
        .bind_refs(&[D1Type::Text(id)])?
        .first::<Row>(None)
        .await?;
    row.map(|row| {
        let document = ShareDocument::from_json(&row.document_json)
            .map_err(|error| worker::Error::RustError(error.to_string()))?;
        Ok(ShareRecord {
            id: row.id,
            owner_id: row.owner_id,
            title: row.title,
            document,
            created_at: parse_time(&row.created_at),
            revoked_at: row.revoked_at.as_deref().map(parse_time),
        })
    })
    .transpose()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateShareResponse {
    id: String,
    url: String,
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListShareRow {
    id: String,
    title: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct OAuthRegisterRequest {
    client_name: Option<String>,
    redirect_uris: Vec<String>,
    scope: Option<String>,
}

#[derive(Debug, Serialize)]
struct OAuthRegisterResponse {
    client_id: String,
    client_name: String,
    redirect_uris: Vec<String>,
    grant_types: Vec<&'static str>,
    response_types: Vec<&'static str>,
    token_endpoint_auth_method: &'static str,
    scope: String,
    client_id_issued_at: i64,
    client_secret_expires_at: i64,
}

async fn oauth_register(req: &mut Request, env: &Env) -> Result<Response> {
    let input = req.json::<OAuthRegisterRequest>().await?;
    if input.redirect_uris.is_empty() || input.redirect_uris.len() > 8 {
        return Response::error("redirect_uris is required", 400);
    }
    for redirect_uri in &input.redirect_uris {
        validate_redirect_uri(redirect_uri)?;
    }
    let scope = clean_scope(input.scope.as_deref().unwrap_or("shares:read shares:write"))?;
    let client_id = format!("fc_{}", token(24));
    let client_name = input
        .client_name
        .unwrap_or_else(|| "Footon client".to_string());
    let now = unix_now();
    let created_at = now_string();
    let expires_at = time_string(now + CLIENT_TTL_SECONDS);
    let redirect_uris_json = serde_json::to_string(&input.redirect_uris)?;
    env.d1("DB")?
        .prepare(
            "INSERT INTO oauth_clients_v2 (client_id, client_name, redirect_uris_json, scope, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind_refs(&[
            D1Type::Text(&client_id),
            D1Type::Text(&client_name),
            D1Type::Text(&redirect_uris_json),
            D1Type::Text(&scope),
            D1Type::Text(&created_at),
            D1Type::Text(&expires_at),
        ])?
        .run()
        .await?;
    json_response_with_status(
        &OAuthRegisterResponse {
            client_id,
            client_name,
            redirect_uris: input.redirect_uris,
            grant_types: vec!["authorization_code", "refresh_token"],
            response_types: vec!["code"],
            token_endpoint_auth_method: "none",
            scope,
            client_id_issued_at: now,
            client_secret_expires_at: 0,
        },
        201,
    )
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    email: String,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: String,
    code_challenge_method: String,
    resource: String,
    #[serde(alias = "cf-turnstile-response")]
    turnstile_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthRequestResponse {
    ok: bool,
}

#[derive(Deserialize)]
struct MagicRow {
    email: String,
    client_id: String,
    redirect_uri: String,
    scope: String,
    code_challenge: String,
    state: String,
    resource: String,
    expires_at: String,
}

#[derive(Deserialize)]
struct CodeRow {
    client_id: String,
    user_id: String,
    email: String,
    redirect_uri: String,
    scope: String,
    code_challenge: String,
    resource: String,
    expires_at: String,
    used_at: Option<String>,
}

#[derive(Deserialize)]
struct RefreshRow {
    family_id: String,
    client_id: String,
    user_id: String,
    email: String,
    scope: String,
    resource: String,
    expires_at: String,
    used_at: Option<String>,
    revoked_at: Option<String>,
}

#[derive(Deserialize)]
struct AccessTokenRow {
    user_id: String,
    scope: String,
    expires_at: String,
    revoked_at: Option<String>,
}

async fn auth_request(req: &mut Request, env: &Env, url: &Url) -> Result<Response> {
    let body = req.text().await?;
    let content_type = req.headers().get("Content-Type")?.unwrap_or_default();
    let input = if content_type.starts_with("application/json") {
        serde_json::from_str::<AuthRequest>(&body)?
    } else {
        serde_urlencoded::from_str::<AuthRequest>(&body)?
    };
    if input.code_challenge_method != "S256" {
        return Response::error("S256 PKCE is required", 400);
    }
    if !verify_turnstile(env, &input.turnstile_token).await? {
        return Response::error("Turnstile verification failed", 400);
    }
    let client = load_client(env, &input.client_id).await?;
    if !client.redirect_uris.contains(&input.redirect_uri) {
        return Response::error("redirect_uri is not registered", 400);
    }
    validate_resource(&input.resource)?;
    let scope = clean_scope(input.scope.as_deref().unwrap_or("shares:read shares:write"))?;
    if !scope_is_subset(&scope, &client.scope) {
        return Response::error("requested scope exceeds client registration", 400);
    }
    let ticket = token(32);
    let ticket_hash = hash(&ticket);
    let created_at = now_string();
    let expires_at = time_string(unix_now() + MAGIC_TTL_SECONDS);
    env.d1("DB")?
        .prepare(
            "INSERT INTO oauth_magic_links_v2
             (ticket_hash, email, client_id, redirect_uri, scope, code_challenge, state, resource, created_at, expires_at, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
        )
        .bind_refs(&[
            D1Type::Text(&ticket_hash),
            D1Type::Text(&input.email),
            D1Type::Text(&input.client_id),
            D1Type::Text(&input.redirect_uri),
            D1Type::Text(&scope),
            D1Type::Text(&input.code_challenge),
            D1Type::Text(input.state.as_deref().unwrap_or_default()),
            D1Type::Text(&input.resource),
            D1Type::Text(&created_at),
            D1Type::Text(&expires_at),
        ])?
        .run()
        .await?;
    let verify_url = format!(
        "{}/auth/verify?ticket={ticket}",
        url.origin().ascii_serialization()
    );
    send_magic_link(env, &input.email, &verify_url).await?;
    if content_type.starts_with("application/json") {
        json_response(&AuthRequestResponse { ok: true })
    } else {
        html_response(check_email_page())
    }
}

async fn auth_verify(env: &Env, url: &Url) -> Result<Response> {
    let ticket = query(url, "ticket")
        .ok_or_else(|| worker::Error::RustError("ticket missing".to_string()))?;
    let ticket_hash = hash(&ticket);
    let db = env.d1("DB")?;
    let row = db
        .prepare(
            "SELECT email, client_id, redirect_uri, scope, code_challenge, state, resource, expires_at
             FROM oauth_magic_links_v2
             WHERE ticket_hash = ?1 AND consumed_at IS NULL",
        )
        .bind_refs(&[D1Type::Text(&ticket_hash)])?
        .first::<MagicRow>(None)
        .await?;
    let Some(row) = row else {
        return Response::error("invalid magic link", 400);
    };
    if parse_time(&row.expires_at).timestamp() < unix_now() {
        return Response::error("expired magic link", 400);
    }
    let code = token(32);
    let code_hash = hash(&code);
    let user_id = format!("email:{}", row.email.to_ascii_lowercase());
    let now = now_string();
    let expires_at = time_string(unix_now() + CODE_TTL_SECONDS);
    let consumed = db
        .prepare("UPDATE oauth_magic_links_v2 SET consumed_at = ?1 WHERE ticket_hash = ?2 AND consumed_at IS NULL")
        .bind_refs(&[D1Type::Text(&now), D1Type::Text(&ticket_hash)])?
        .run()
        .await?;
    if consumed.meta()?.and_then(|meta| meta.changes) != Some(1) {
        return Response::error("invalid magic link", 400);
    }
    db.prepare(
        "INSERT INTO oauth_codes_v2
         (code_hash, client_id, user_id, email, redirect_uri, scope, code_challenge, resource, created_at, expires_at, used_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
    )
    .bind_refs(&[
        D1Type::Text(&code_hash),
        D1Type::Text(&row.client_id),
        D1Type::Text(&user_id),
        D1Type::Text(&row.email),
        D1Type::Text(&row.redirect_uri),
        D1Type::Text(&row.scope),
        D1Type::Text(&row.code_challenge),
        D1Type::Text(&row.resource),
        D1Type::Text(&now),
        D1Type::Text(&expires_at),
    ])?
    .run()
    .await?;
    let mut redirect = Url::parse(&row.redirect_uri)?;
    redirect.query_pairs_mut().append_pair("code", &code);
    if !row.state.is_empty() {
        redirect.query_pairs_mut().append_pair("state", &row.state);
    }
    Response::redirect_with_status(redirect, 302)
}

#[derive(Debug, Deserialize)]
struct TokenRequest {
    grant_type: String,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: String,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    resource: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    refresh_token: String,
    scope: String,
}

async fn oauth_token(req: &mut Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    let input = serde_urlencoded::from_str::<TokenRequest>(&body)?;
    if input.grant_type == "authorization_code" {
        return exchange_code(env, input).await;
    }
    if input.grant_type == "refresh_token" {
        return exchange_refresh(env, input).await;
    }
    Response::error("unsupported grant_type", 400)
}

async fn exchange_code(env: &Env, input: TokenRequest) -> Result<Response> {
    let code = input
        .code
        .ok_or_else(|| worker::Error::RustError("code missing".to_string()))?;
    let verifier = input
        .code_verifier
        .ok_or_else(|| worker::Error::RustError("code_verifier missing".to_string()))?;
    let code_hash = hash(&code);
    let db = env.d1("DB")?;
    let Some(row) = db
        .prepare("SELECT client_id, user_id, email, redirect_uri, scope, code_challenge, resource, expires_at, used_at FROM oauth_codes_v2 WHERE code_hash = ?1")
        .bind_refs(&[D1Type::Text(&code_hash)])?
        .first::<CodeRow>(None)
        .await?
    else {
        return Response::error("invalid code", 400);
    };
    if row.client_id != input.client_id
        || row.used_at.is_some()
        || parse_time(&row.expires_at).timestamp() < unix_now()
    {
        return Response::error("invalid code", 400);
    }
    if input.redirect_uri.as_deref() != Some(row.redirect_uri.as_str()) {
        return Response::error("redirect_uri mismatch", 400);
    }
    if s256(&verifier) != row.code_challenge {
        return Response::error("PKCE verification failed", 400);
    }
    if input.resource.as_deref() != Some(row.resource.as_str()) {
        return Response::error("resource mismatch", 400);
    }
    let consumed = db
        .prepare("UPDATE oauth_codes_v2 SET used_at = ?1 WHERE code_hash = ?2 AND used_at IS NULL")
        .bind_refs(&[D1Type::Text(&now_string()), D1Type::Text(&code_hash)])?
        .run()
        .await?;
    if consumed.meta()?.and_then(|meta| meta.changes) != Some(1) {
        return Response::error("invalid code", 400);
    }
    issue_tokens(
        env,
        &row.client_id,
        &row.user_id,
        &row.email,
        &row.scope,
        &row.resource,
        None,
    )
    .await
}

async fn exchange_refresh(env: &Env, input: TokenRequest) -> Result<Response> {
    let refresh = input
        .refresh_token
        .ok_or_else(|| worker::Error::RustError("refresh_token missing".to_string()))?;
    let refresh_hash = hash(&refresh);
    let db = env.d1("DB")?;
    let Some(row) = db
        .prepare("SELECT family_id, client_id, user_id, email, scope, resource, expires_at, used_at, revoked_at FROM oauth_refresh_tokens_v2 WHERE token_hash = ?1")
        .bind_refs(&[D1Type::Text(&refresh_hash)])?
        .first::<RefreshRow>(None)
        .await?
    else {
        return Response::error("invalid refresh_token", 400);
    };
    if row.client_id != input.client_id
        || parse_time(&row.expires_at).timestamp() < unix_now()
        || row.revoked_at.is_some()
    {
        return Response::error("invalid refresh_token", 400);
    }
    if row.used_at.is_some() {
        db.prepare("UPDATE oauth_refresh_tokens_v2 SET revoked_at = ?1 WHERE family_id = ?2")
            .bind_refs(&[D1Type::Text(&now_string()), D1Type::Text(&row.family_id)])?
            .run()
            .await?;
        return Response::error("refresh token reuse detected", 400);
    }
    let consumed = db
        .prepare("UPDATE oauth_refresh_tokens_v2 SET used_at = ?1 WHERE token_hash = ?2 AND used_at IS NULL AND revoked_at IS NULL")
        .bind_refs(&[D1Type::Text(&now_string()), D1Type::Text(&refresh_hash)])?
        .run()
        .await?;
    if consumed.meta()?.and_then(|meta| meta.changes) != Some(1) {
        db.prepare("UPDATE oauth_refresh_tokens_v2 SET revoked_at = ?1 WHERE family_id = ?2")
            .bind_refs(&[D1Type::Text(&now_string()), D1Type::Text(&row.family_id)])?
            .run()
            .await?;
        return Response::error("refresh token reuse detected", 400);
    }
    issue_tokens(
        env,
        &row.client_id,
        &row.user_id,
        &row.email,
        &row.scope,
        &row.resource,
        Some(&row.family_id),
    )
    .await
}

async fn issue_tokens(
    env: &Env,
    client_id: &str,
    user_id: &str,
    email: &str,
    scope: &str,
    resource: &str,
    family_id: Option<&str>,
) -> Result<Response> {
    let access_token = token(32);
    let refresh_token = token(32);
    let access_hash = hash(&access_token);
    let refresh_hash = hash(&refresh_token);
    let family_id = family_id.map_or_else(|| token(18), ToOwned::to_owned);
    let now = now_string();
    let access_expires = time_string(unix_now() + ACCESS_TTL_SECONDS);
    let refresh_expires = time_string(unix_now() + REFRESH_TTL_SECONDS);
    let db = env.d1("DB")?;
    db.prepare(
        "INSERT INTO oauth_access_tokens_v2 (token_hash, client_id, user_id, email, scope, resource, created_at, expires_at, revoked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
    )
    .bind_refs(&[
        D1Type::Text(&access_hash),
        D1Type::Text(client_id),
        D1Type::Text(user_id),
        D1Type::Text(email),
        D1Type::Text(scope),
        D1Type::Text(resource),
        D1Type::Text(&now),
        D1Type::Text(&access_expires),
    ])?
    .run()
    .await?;
    db.prepare(
        "INSERT INTO oauth_refresh_tokens_v2 (token_hash, family_id, client_id, user_id, email, scope, resource, created_at, expires_at, used_at, revoked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL)",
    )
    .bind_refs(&[
        D1Type::Text(&refresh_hash),
        D1Type::Text(&family_id),
        D1Type::Text(client_id),
        D1Type::Text(user_id),
        D1Type::Text(email),
        D1Type::Text(scope),
        D1Type::Text(resource),
        D1Type::Text(&now),
        D1Type::Text(&refresh_expires),
    ])?
    .run()
    .await?;
    json_response(&TokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: ACCESS_TTL_SECONDS,
        refresh_token,
        scope: scope.to_string(),
    })
}

async fn oauth_revoke(req: &mut Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    let params = serde_urlencoded::from_str::<std::collections::HashMap<String, String>>(&body)?;
    let Some(token_value) = params.get("token") else {
        return Response::error("token is required", 400);
    };
    let token_hash = hash(token_value);
    let now = now_string();
    let db = env.d1("DB")?;
    db.prepare("UPDATE oauth_access_tokens_v2 SET revoked_at = ?1 WHERE token_hash = ?2")
        .bind_refs(&[D1Type::Text(&now), D1Type::Text(&token_hash)])?
        .run()
        .await?;
    db.prepare("UPDATE oauth_refresh_tokens_v2 SET revoked_at = ?1 WHERE token_hash = ?2")
        .bind_refs(&[D1Type::Text(&now), D1Type::Text(&token_hash)])?
        .run()
        .await?;
    Response::empty()
}

#[derive(Debug, Deserialize)]
struct OAuthClientRow {
    client_id: String,
    redirect_uris_json: String,
    scope: String,
    expires_at: String,
}

struct OAuthClient {
    redirect_uris: Vec<String>,
    scope: String,
}

async fn load_client(env: &Env, client_id: &str) -> Result<OAuthClient> {
    let Some(row) = env
        .d1("DB")?
        .prepare("SELECT client_id, redirect_uris_json, scope, expires_at FROM oauth_clients_v2 WHERE client_id = ?1")
        .bind_refs(&[D1Type::Text(client_id)])?
        .first::<OAuthClientRow>(None)
        .await?
    else {
        return Err(worker::Error::RustError("unknown client".to_string()));
    };
    if row.client_id != client_id || parse_time(&row.expires_at).timestamp() < unix_now() {
        return Err(worker::Error::RustError("expired client".to_string()));
    }
    Ok(OAuthClient {
        redirect_uris: serde_json::from_str(&row.redirect_uris_json)?,
        scope: row.scope,
    })
}

#[derive(Debug, Clone)]
struct AuthUser {
    user_id: String,
    scope: String,
}

async fn bearer_user(req: &Request, env: &Env) -> Result<Option<AuthUser>> {
    let auth = req.headers().get("Authorization")?.unwrap_or_default();
    let Some(token_value) = auth.strip_prefix("Bearer ") else {
        return Ok(None);
    };
    let token_hash = hash(token_value);
    let Some(row) = env
        .d1("DB")?
        .prepare("SELECT user_id, scope, expires_at, revoked_at FROM oauth_access_tokens_v2 WHERE token_hash = ?1")
        .bind_refs(&[D1Type::Text(&token_hash)])?
        .first::<AccessTokenRow>(None)
        .await?
    else {
        return Ok(None);
    };
    if row.revoked_at.is_some() || parse_time(&row.expires_at).timestamp() < unix_now() {
        return Ok(None);
    }
    Ok(Some(AuthUser {
        user_id: row.user_id,
        scope: row.scope,
    }))
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

async fn mcp(req: &mut Request, env: &Env) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    let request = req.json::<RpcRequest>().await?;
    if request.jsonrpc != "2.0" {
        return json_response(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": request.id,
            "error": { "code": -32600, "message": "invalid request" }
        }));
    }
    let result = match request.method.as_str() {
        "initialize" => serde_json::json!({
            "protocolVersion": "2026-07-28",
            "serverInfo": { "name": "footon", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "tools": {} }
        }),
        "ping" => serde_json::json!({}),
        "tools/list" => serde_json::json!({
            "tools": [
                { "name": "share_create", "description": "Publish one approved sanitized share" },
                { "name": "share_list", "description": "List your active Footon shares" },
                { "name": "share_revoke", "description": "Revoke one Footon share" }
            ]
        }),
        "tools/call" => match call_tool(env, &user, request.params).await {
            Ok(result) => result,
            Err(error) => {
                return json_response(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "error": { "code": -32003, "message": error.to_string() }
                }));
            }
        },
        _ => {
            return json_response(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.id,
                "error": { "code": -32601, "message": "method not found" }
            }));
        }
    };
    json_response(&serde_json::json!({ "jsonrpc": "2.0", "id": request.id, "result": result }))
}

async fn call_tool(
    env: &Env,
    user: &AuthUser,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let params = params.unwrap_or_else(|| serde_json::json!({}));
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    match name {
        "share_list" => {
            require_scope(user, "shares:read")?;
            let rows = env
                .d1("DB")?
                .prepare("SELECT id, title, created_at FROM shares WHERE owner_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC LIMIT 50")
                .bind_refs(&[D1Type::Text(&user.user_id)])?
                .all()
                .await?
                .results::<ListShareRow>()?;
            Ok(serde_json::to_value(rows)?)
        }
        "share_revoke" => {
            require_scope(user, "shares:write")?;
            let id = args
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let now = now_string();
            env.d1("DB")?
                .prepare("UPDATE shares SET revoked_at = ?1 WHERE id = ?2 AND owner_id = ?3")
                .bind_refs(&[
                    D1Type::Text(&now),
                    D1Type::Text(id),
                    D1Type::Text(&user.user_id),
                ])?
                .run()
                .await?;
            Ok(serde_json::json!({ "ok": true }))
        }
        "share_create" => {
            require_scope(user, "shares:write")?;
            let share = serde_json::from_value::<Share>(args)?;
            if share.schema_version != footon_core::model::SCHEMA_VERSION {
                return Err(worker::Error::RustError(
                    "new shares must use footon.share.v2".to_string(),
                ));
            }
            validate_share(&share).map_err(|error| worker::Error::RustError(error.to_string()))?;
            validate_safe_share(&share)
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            let id = token(18);
            let now = now_string();
            let document_json = serde_json::to_string(&share)?;
            env.d1("DB")?
                .prepare("INSERT INTO shares (id, owner_id, title, document_json, created_at, revoked_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL)")
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user.user_id),
                    D1Type::Text(&share.title),
                    D1Type::Text(&document_json),
                    D1Type::Text(&now),
                ])?
                .run()
                .await?;
            Ok(serde_json::json!({ "id": id, "url": format!("{ORIGIN}/s/{id}"), "createdAt": now }))
        }
        _ => Err(worker::Error::RustError("unknown tool".to_string())),
    }
}

fn home_page() -> String {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Footon</title><link rel="stylesheet" href="/style.css"></head><body class="bg-canvas text-ink"><main class="mx-auto max-w-3xl px-4 py-8"><p class="font-mono text-xs text-muted">FOOTON / SAFE AGENT SHARING</p><h1 class="mt-2 text-3xl font-semibold tracking-tight">Share the useful thread, not the private transcript.</h1><p class="mt-3 max-w-2xl text-sm leading-6 text-muted">Footon scans locally, requires explicit approval, scans again at publish time, and serves the approved result as a dense reader or plain Markdown.</p><div class="mt-6 flex gap-2 text-sm"><a class="border border-ink bg-ink px-3 py-2 text-paper" href="/install">Install CLI</a><a class="border border-line bg-paper px-3 py-2" href="/connect">Connect an agent</a></div></main></body></html>"#.to_string()
}

fn install_page() -> String {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Install Footon</title><link rel="stylesheet" href="/style.css"></head><body class="bg-canvas text-ink"><main class="mx-auto max-w-2xl px-4 py-8"><p class="font-mono text-xs text-muted">FOOTON / INSTALL</p><h1 class="mt-2 text-2xl font-semibold">Install the Rust CLI</h1><pre class="mt-5 overflow-x-auto border border-line bg-paper p-3 text-sm"><code>cargo install footon</code></pre><p class="mt-4 text-sm text-muted">Fetch a shared thread as Markdown with <code>footon fetch https://footon.dev/s/...</code>.</p></main></body></html>"#.to_string()
}

fn connect_page() -> String {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Connect Footon</title><link rel="stylesheet" href="/style.css"></head><body class="bg-canvas text-ink"><main class="mx-auto max-w-2xl px-4 py-8"><p class="font-mono text-xs text-muted">FOOTON / CONNECT</p><h1 class="mt-2 text-2xl font-semibold">Connect an agent</h1><dl class="mt-5 grid grid-cols-[8rem_1fr] gap-x-3 gap-y-2 border-y border-line py-3 text-sm"><dt class="text-muted">MCP endpoint</dt><dd><code>https://footon.dev/mcp</code></dd><dt class="text-muted">OAuth</dt><dd>Authorization code + PKCE S256</dd><dt class="text-muted">Scopes</dt><dd><code>shares:read shares:write</code></dd></dl></main></body></html>"#.to_string()
}

fn check_email_page() -> String {
    r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Check your email</title><link rel="stylesheet" href="/style.css"></head><body class="bg-canvas text-ink"><main class="mx-auto max-w-md px-4 py-10"><p class="font-mono text-xs text-muted">FOOTON / AUTHORIZE</p><h1 class="mt-2 text-2xl font-semibold">Check your email</h1><p class="mt-3 text-sm text-muted">The sign-in link expires in 10 minutes and can be used once.</p></main></body></html>"#.to_string()
}

fn authorize_page(url: &Url, env: &Env) -> String {
    let client_id = escape_html(&query(url, "client_id").unwrap_or_default());
    let redirect_uri = escape_html(&query(url, "redirect_uri").unwrap_or_default());
    let scope =
        escape_html(&query(url, "scope").unwrap_or_else(|| "shares:read shares:write".to_string()));
    let state = escape_html(&query(url, "state").unwrap_or_default());
    let code_challenge = escape_html(&query(url, "code_challenge").unwrap_or_default());
    let resource = escape_html(&query(url, "resource").unwrap_or_else(|| format!("{ORIGIN}/mcp")));
    let turnstile = env.var("TURNSTILE_SITE_KEY").ok().map_or_else(String::new, |key| {
        format!(r#"<div class="cf-turnstile" data-sitekey="{}"></div><script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>"#, escape_html(&key.to_string()))
    });
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Authorize Footon</title><link rel="stylesheet" href="/style.css"></head><body class="bg-canvas text-ink"><main class="mx-auto max-w-md px-4 py-8"><p class="font-mono text-xs text-muted">FOOTON / AUTHORIZE</p><h1 class="mt-2 text-2xl font-semibold">Authorize agent access</h1><p class="mt-2 text-sm text-muted">Requested scopes: <code>{scope}</code></p><form class="mt-5 grid gap-3" method="post" action="/auth/request"><label class="grid gap-1 text-xs font-medium">Email<input class="border border-line bg-paper px-3 py-2 text-sm" name="email" type="email" autocomplete="email" required></label><input type="hidden" name="client_id" value="{client_id}"><input type="hidden" name="redirect_uri" value="{redirect_uri}"><input type="hidden" name="scope" value="{scope}"><input type="hidden" name="state" value="{state}"><input type="hidden" name="code_challenge" value="{code_challenge}"><input type="hidden" name="code_challenge_method" value="S256"><input type="hidden" name="resource" value="{resource}">{turnstile}<button class="border border-ink bg-ink px-3 py-2 text-sm text-paper" type="submit">Send magic link</button></form></main></body></html>"#
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn authorization_server_metadata() -> serde_json::Value {
    serde_json::json!({
        "issuer": ORIGIN,
        "authorization_endpoint": format!("{ORIGIN}/authorize"),
        "token_endpoint": format!("{ORIGIN}/oauth/token"),
        "registration_endpoint": format!("{ORIGIN}/oauth/register"),
        "revocation_endpoint": format!("{ORIGIN}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["shares:read", "shares:write"]
    })
}

fn protected_resource_metadata() -> serde_json::Value {
    serde_json::json!({
        "resource": format!("{ORIGIN}/mcp"),
        "authorization_servers": [ORIGIN],
        "scopes_supported": ["shares:read", "shares:write"],
        "bearer_methods_supported": ["header"]
    })
}

fn validate_redirect_uri(uri: &str) -> Result<()> {
    let parsed = Url::parse(uri)?;
    let loopback = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if parsed.fragment().is_some()
        || (parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback))
    {
        return Err(worker::Error::RustError(
            "redirect_uri must use HTTPS outside loopback".to_string(),
        ));
    }
    Ok(())
}

fn validate_resource(resource: &str) -> Result<()> {
    if resource == format!("{ORIGIN}/mcp") {
        Ok(())
    } else {
        Err(worker::Error::RustError(
            "resource must be https://footon.dev/mcp".to_string(),
        ))
    }
}

fn clean_scope(scope: &str) -> Result<String> {
    let mut parts = scope.split_whitespace().collect::<Vec<_>>();
    parts.sort_unstable();
    parts.dedup();
    if parts
        .iter()
        .all(|scope| matches!(*scope, "shares:read" | "shares:write"))
    {
        Ok(parts.join(" "))
    } else {
        Err(worker::Error::RustError("unsupported scope".to_string()))
    }
}

fn scope_is_subset(requested: &str, registered: &str) -> bool {
    requested.split_whitespace().all(|scope| {
        registered
            .split_whitespace()
            .any(|allowed| allowed == scope)
    })
}

fn require_scope(user: &AuthUser, required: &str) -> Result<()> {
    if has_scope(user, required) {
        Ok(())
    } else {
        Err(worker::Error::RustError(format!(
            "missing scope: {required}"
        )))
    }
}

fn has_scope(user: &AuthUser, required: &str) -> bool {
    user.scope.split_whitespace().any(|scope| scope == required)
}

fn validate_share_id(id: &str) -> Result<()> {
    if (20..=40).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(worker::Error::RustError("invalid share id".to_string()))
    }
}

fn query(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
}

async fn send_magic_link(env: &Env, email: &str, verify_url: &str) -> Result<()> {
    let binding = env.send_email("EMAIL")?;
    let message = SendEmailBuilder::builder("login@footon.dev", email, "Footon sign-in link")
        .text(&format!("Open this Footon sign-in link:\n\n{verify_url}\n"))
        .build();
    binding.send_with_builder(&message).await?;
    Ok(())
}

#[derive(Deserialize)]
struct TurnstileResponse {
    success: bool,
}

async fn verify_turnstile(env: &Env, token: &str) -> Result<bool> {
    if token.trim().is_empty() {
        return Ok(false);
    }
    let secret = env.secret("TURNSTILE_SECRET")?.to_string();
    let body = serde_urlencoded::to_string([("secret", secret.as_str()), ("response", token)])
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    let headers = Headers::new();
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&body)));
    let request = Request::new_with_init(
        "https://challenges.cloudflare.com/turnstile/v0/siteverify",
        &init,
    )?;
    let mut response = Fetch::Request(request).send().await?;
    Ok(response.json::<TurnstileResponse>().await?.success)
}

fn json_response<T: Serialize>(value: &T) -> Result<Response> {
    json_response_with_status(value, 200)
}

fn json_response_with_status<T: Serialize>(value: &T, status: u16) -> Result<Response> {
    let mut response = Response::from_json(value)?.with_status(status);
    security_headers(&mut response)?;
    Ok(response)
}

fn html_response(html: String) -> Result<Response> {
    let mut response = Response::from_html(html)?;
    security_headers(&mut response)?;
    Ok(response)
}

fn authorize_response(html: String) -> Result<Response> {
    let mut response = Response::from_html(html)?;
    security_headers(&mut response)?;
    response.headers_mut().set(
        "Content-Security-Policy",
        "default-src 'none'; style-src 'self' 'unsafe-inline' https://challenges.cloudflare.com; script-src https://challenges.cloudflare.com; frame-src https://challenges.cloudflare.com; connect-src https://challenges.cloudflare.com; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
    )?;
    Ok(response)
}

fn markdown_response(markdown: &str) -> Result<Response> {
    let mut response = Response::ok(markdown.to_string())?;
    response
        .headers_mut()
        .set("Content-Type", "text/markdown; charset=utf-8")?;
    response.headers_mut().set("Vary", "Accept")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}

fn css_response(css: &str) -> Result<Response> {
    let mut response = Response::ok(css.to_string())?;
    response
        .headers_mut()
        .set("Content-Type", "text/css; charset=utf-8")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=3600")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}

fn javascript_response(script: &str) -> Result<Response> {
    let mut response = Response::ok(script.to_string())?;
    response
        .headers_mut()
        .set("Content-Type", "text/javascript; charset=utf-8")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=3600")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}

fn internal_error(error: &worker::Error) -> Result<Response> {
    worker::console_error!("request failed: {error}");
    let mut response = Response::error("internal server error", 500)?;
    security_headers(&mut response)?;
    Ok(response)
}

fn security_headers(response: &mut Response) -> Result<()> {
    response.headers_mut().set("Content-Security-Policy", "default-src 'none'; style-src 'self'; script-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'")?;
    response
        .headers_mut()
        .set("Referrer-Policy", "no-referrer")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    response.headers_mut().set("X-Frame-Options", "DENY")?;
    Ok(())
}

fn token(bytes: usize) -> String {
    use base64::Engine;
    let mut raw = vec![0_u8; bytes];
    getrandom::fill(&mut raw).expect("secure random source");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
}

fn hash(value: &str) -> String {
    hex(&Sha256::digest(value.as_bytes()))
}

fn s256(value: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(TABLE[usize::from(byte >> 4)]));
        out.push(char::from(TABLE[usize::from(byte & 0x0f)]));
    }
    out
}

fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn now_string() -> String {
    time_string(unix_now())
}

fn time_string(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap_or_default()
        .to_rfc3339()
}

fn parse_time(value: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(chrono::DateTime::from)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topcoat_router_accepts_only_registered_routes_and_methods() {
        let accepted = http::Request::get("https://footon.dev/s/abcdefghijklmnopqrst")
            .body(TopcoatBody::empty())
            .expect("request");
        let rejected = http::Request::post("https://footon.dev/s/abcdefghijklmnopqrst")
            .body(TopcoatBody::empty())
            .expect("request");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");

        assert_eq!(
            runtime.block_on(router().handle(accepted)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime.block_on(router().handle(rejected)).status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[test]
    fn scopes_are_canonical_and_cannot_expand() {
        assert_eq!(
            clean_scope("shares:write shares:read shares:read").expect("scope"),
            "shares:read shares:write"
        );
        assert!(scope_is_subset("shares:read", "shares:read shares:write"));
        assert!(!scope_is_subset("shares:write", "shares:read"));
        assert!(clean_scope("read write").is_err());
    }

    #[test]
    fn share_ids_and_redirects_are_bounded() {
        assert!(validate_share_id("abcdefghijklmnopqrst").is_ok());
        assert!(validate_share_id("short").is_err());
        assert!(validate_redirect_uri("https://agent.example/callback").is_ok());
        assert!(validate_redirect_uri("http://localhost:4000/callback").is_ok());
        assert!(validate_redirect_uri("http://agent.example/callback").is_err());
        assert!(validate_redirect_uri("https://agent.example/callback#fragment").is_err());
    }

    #[test]
    fn rendered_view_uses_flat_teletype_rows() {
        let record = ShareRecord {
            id: "abcdefghijklmnopqrst".to_string(),
            owner_id: "email:test@example.com".to_string(),
            title: "Dense thread".to_string(),
            document: ShareDocument {
                schema_version: footon_core::model::SCHEMA_VERSION.to_string(),
                title: "Dense thread".to_string(),
                approved_at: chrono::DateTime::UNIX_EPOCH,
                messages: vec![Message {
                    role: footon_core::model::Role::Assistant,
                    text: "**Done.**".to_string(),
                }],
                report: footon_core::model::Report::default(),
            },
            created_at: chrono::DateTime::UNIX_EPOCH,
            revoked_at: None,
        };
        let html = viewer_page(&record, false);
        assert!(html.contains("<article class=\"viewer\">"));
        assert!(html.contains("<div class=\"thread\">"));
        assert!(html.contains("<section class=\"call-block\">"));
        assert!(html.contains("class=\"message assistant\""));
        assert!(html.contains(
            "<a class=\"ordinal\" href=\"#message-1\" aria-label=\"Link to message 1\">001</a><span class=\"role\">AGENT</span>"
        ));
        assert!(html.contains("id=\"filter-user\""));
        assert!(html.contains("id=\"filter-agent\""));
        assert!(html.contains("id=\"filter-tools\""));
        assert!(html.contains("class=\"filter-toggle user\""));
        assert!(html.contains("class=\"filter-toggle assistant\""));
        assert!(html.contains("class=\"filter-toggle tool\""));
        assert!(html.contains("class=\"view-icon rendered-icon\""));
        assert!(html.contains("class=\"view-icon text-icon\""));
        assert!(!html.contains(">Rendered<"));
        assert!(!html.contains(">Text<"));
        assert!(html.contains("AGENT"));
        assert!(html.contains("<strong>Done.</strong>"));
        assert!(!html.contains("shadow-sm"));
        assert!(!html.contains("bg-paper px-4 py-3"));
        let theme = include_str!("theme.css");
        assert!(theme.contains("grid-template-columns: 44px 72px minmax(0, 1fr)"));
        assert!(theme.contains("color-scheme: dark"));
        assert!(theme.contains("[hidden] { display: none !important; }"));
        assert!(theme.contains("z-index: 20;"));
        assert!(theme.contains("width: 24px;"));
        assert!(theme.contains("width: 6px;"));
        assert!(theme.contains(".message:hover, .message:focus-within"));
        assert!(!theme.contains(".message.user"));
        assert!(
            theme
                .lines()
                .all(|line| !line.trim_start().starts_with("border"))
        );
    }
}

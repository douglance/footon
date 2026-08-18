#![forbid(unsafe_code)]

use footon_core::accept::{ContentType, negotiate};
use footon_core::blackout::{BLACKOUT_TEXT, blackout};
use footon_core::markdown::messages_to_markdown;
use footon_core::model::{Message, Share, ShareDocument, ShareRecord, validate_share};
use footon_core::validate::validate_share as validate_safe_share;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use worker::Method;
use worker::d1::D1Type;
use worker::email::SendEmailBuilder;
use worker::{Context, Env, Request, Response, Result, ScheduledEvent};

use topcoat::context::Cx;
use topcoat::router::{
    Body as TopcoatBody, Method as HttpMethod, Path, RouteFn, RouteFuture, Router,
};

mod access;
mod billing;
mod billing_adapter;
mod components;
mod shares;
mod systems;
mod ui;

use access::{GeneralAccess, ShareAction, action_is_available_for_plan, load_share_access};
use billing_adapter::{
    billing_status, checkout, lemon_squeezy_webhook, user_has_private_share_capacity, user_is_pro,
};

use ui::authorization::{
    AuthorizationPage, VerificationPage, authorization_markdown, authorize_page,
    verification_markdown, verification_page,
};
use ui::commercial::{
    pricing_markdown, pricing_page, security_markdown, security_page, support_markdown,
    support_page,
};
use ui::pages::{
    LANDING_JS, home_markdown, home_page, llms_markdown, privacy_markdown, privacy_page,
    terms_markdown, terms_page,
};
use ui::response as ui_response;
use ui::thread::{VIEWER_JS, viewer_page};

const STYLE: &str = include_str!(concat!(env!("OUT_DIR"), "/tailwind.css"));
const ICON: &str = include_str!("../../assets/footon-icon.svg");
const FONT: &[u8] = include_bytes!("../../assets/departure-mono-1.500.woff2");
const ORIGIN: &str = "https://footon.dev";
const ACCESS_TTL_SECONDS: i64 = 3_600;
const REFRESH_TTL_SECONDS: i64 = 2_592_000;
const CLIENT_TTL_SECONDS: i64 = 7_776_000;
const CODE_TTL_SECONDS: i64 = 300;
const EMAIL_CODE_TTL_SECONDS: i64 = 600;
const EMAIL_CODE_RESEND_COOLDOWN_SECONDS: i64 = 60;
const MAX_EMAIL_CODE_ATTEMPTS: i32 = 5;
const REVOKED_SHARE_RETENTION_SECONDS: i64 = 2_592_000;
const REMOTE_REPORT_RETENTION_SECONDS: i64 = 2_592_000;
const MCP_INITIALIZE_PROTOCOL_VERSIONS: [&str; 4] =
    ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
const ROBOTS: &str = "User-agent: *\nAllow: /\n";
const PUBLIC_GET_ROUTES: [&str; 18] = [
    "/healthz",
    "/",
    "/privacy",
    "/terms",
    "/pricing",
    "/security",
    "/support",
    "/llms.txt",
    "/robots.txt",
    "/style.css",
    "/viewer.js",
    "/landing.js",
    "/favicon.svg",
    "/fonts/departure-mono-1.500.woff2",
    "/.well-known/oauth-authorization-server",
    "/.well-known/oauth-protected-resource/mcp",
    "/authorize",
    "/s/{id}",
];

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
            "DELETE FROM share_viewer_challenges WHERE expires_at < ?1",
            "DELETE FROM share_browser_sessions WHERE expires_at < ?1 OR revoked_at IS NOT NULL",
        ] {
            if let Ok(stmt) = db.prepare(sql).bind_refs(&[D1Type::Text(&now)]) {
                let _ = stmt.run().await;
            }
        }
        let report_cutoff = time_string(unix_now() - REMOTE_REPORT_RETENTION_SECONDS);
        if let Ok(stmt) = db
            .prepare("DELETE FROM remote_log_reports WHERE received_at < ?1")
            .bind_refs(&[D1Type::Text(&report_cutoff)])
        {
            let _ = stmt.run().await;
        }
        if let Ok(stmt) = db
            .prepare(
                "DELETE FROM service_keys
                 WHERE expires_at < ?1 OR (revoked_at IS NOT NULL AND revoked_at < ?1)",
            )
            .bind_refs(&[D1Type::Text(&report_cutoff)])
        {
            let _ = stmt.run().await;
        }
        let auth_attempt_cutoff = time_string(unix_now() - 86_400);
        if let Ok(stmt) = db
            .prepare("DELETE FROM share_auth_attempts WHERE created_at < ?1")
            .bind_refs(&[D1Type::Text(&auth_attempt_cutoff)])
        {
            let _ = stmt.run().await;
        }
        let revoked_share_cutoff = time_string(unix_now() - REVOKED_SHARE_RETENTION_SECONDS);
        if let Ok(stmt) = db
            .prepare("DELETE FROM shares WHERE revoked_at IS NOT NULL AND revoked_at < ?1")
            .bind_refs(&[D1Type::Text(&revoked_share_cutoff)])
        {
            let _ = stmt.run().await;
        }
    }
}

async fn handle(req: &mut Request, env: &Env) -> Result<Response> {
    let is_head = req.method() == Method::Head;
    let method = effective_method(req.method());
    let response = handle_method(req, env, method).await?;
    Ok(if is_head {
        without_body(response)
    } else {
        response
    })
}

async fn handle_method(req: &mut Request, env: &Env, method: Method) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();

    if method == Method::Get
        && let Some(response) = public_get(req, env, &url, path).await?
    {
        return Ok(response);
    }
    if method == Method::Post && path == "/oauth/register" {
        return oauth_register(req, env).await;
    }
    if method == Method::Post && path == "/auth/request" {
        return auth_request(req, env).await;
    }
    if method == Method::Post && path == "/auth/verify" {
        return auth_verify(req, env).await;
    }
    if method == Method::Post
        && let Some(id) = viewer_share_id(path, "/signin")
    {
        return access::request_viewer_code(req, env, id).await;
    }
    if method == Method::Post
        && let Some(id) = viewer_share_id(path, "/verify")
    {
        return access::verify_viewer_code(req, env, id).await;
    }
    if method == Method::Post && path == "/viewer/signout" {
        return access::sign_out_viewer(req, env).await;
    }
    if method == Method::Post && path == "/oauth/token" {
        return oauth_token(req, env).await;
    }
    if method == Method::Post && path == "/oauth/revoke" {
        return oauth_revoke(req, env).await;
    }
    if method == Method::Post && path == "/webhooks/lemon-squeezy" {
        return lemon_squeezy_webhook(req, env).await;
    }
    if method == Method::Post && path.starts_with("/checkout/") {
        return checkout(req, env, path.trim_start_matches("/checkout/")).await;
    }
    if let Some(response) = systems::api_route(req, env, &method, path).await? {
        return Ok(response);
    }
    if method == Method::Post && path == "/api/shares" {
        return api_create_share(req, env).await;
    }
    if method == Method::Get && path == "/api/shares" {
        return api_list_shares(req, env).await;
    }
    if method == Method::Get
        && let Some(id) = share_subresource_id(path, "/access")
    {
        return shares::api_access(req, env, id).await;
    }
    if method == Method::Patch
        && let Some(id) = share_item_id(path)
    {
        return shares::api_update(req, env, id).await;
    }
    if method == Method::Put
        && let Some(id) = share_subresource_id(path, "/members")
    {
        return shares::api_grant(req, env, id).await;
    }
    if method == Method::Delete
        && let Some((id, member_id)) = share_member_ids(path)
    {
        return shares::api_remove_member(req, env, id, member_id).await;
    }
    if method == Method::Post
        && let Some(id) = share_subresource_id(path, "/transfer")
    {
        return shares::api_transfer(req, env, id).await;
    }
    if method == Method::Get && path == "/api/billing" {
        return api_billing_status(req, env).await;
    }
    if method == Method::Post
        && let Some(id) = blackout_share_id(path)
    {
        return api_blackout_share(req, env, id).await;
    }
    if method == Method::Delete && path.starts_with("/api/shares/") {
        return api_revoke_share(req, env, path.trim_start_matches("/api/shares/")).await;
    }
    if method == Method::Post && path == "/mcp" {
        return mcp(req, env).await;
    }
    Response::error("not found", 404)
}

async fn public_get(
    req: &mut Request,
    env: &Env,
    url: &Url,
    path: &str,
) -> Result<Option<Response>> {
    let response = match path {
        "/healthz" => healthz(env).await?,
        "/" => {
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_standard(accept.as_deref(), home_page().await, home_markdown())?
        }
        "/privacy" => {
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_standard(
                accept.as_deref(),
                privacy_page().await,
                privacy_markdown(),
            )?
        }
        "/terms" => {
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_standard(
                accept.as_deref(),
                terms_page().await,
                terms_markdown(),
            )?
        }
        "/pricing" => {
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_standard(
                accept.as_deref(),
                pricing_page().await,
                pricing_markdown(),
            )?
        }
        "/security" => {
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_standard(
                accept.as_deref(),
                security_page().await,
                security_markdown(),
            )?
        }
        "/support" => {
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_standard(
                accept.as_deref(),
                support_page().await,
                support_markdown(),
            )?
        }
        "/llms.txt" => ui_response::markdown(llms_markdown())?,
        "/robots.txt" => plain_text_response(ROBOTS)?,
        "/style.css" => css_response(STYLE)?,
        "/viewer.js" => javascript_response(VIEWER_JS)?,
        "/landing.js" => javascript_response(LANDING_JS)?,
        "/favicon.svg" => svg_response(ICON)?,
        "/fonts/departure-mono-1.500.woff2" => font_response(FONT)?,
        "/.well-known/oauth-authorization-server" => {
            json_response(&authorization_server_metadata())?
        }
        "/.well-known/oauth-protected-resource/mcp" => {
            json_response(&protected_resource_metadata())?
        }
        "/authorize" => {
            let page = authorization_page(url);
            let markdown = authorization_markdown(&page);
            let accept = req.headers().get("Accept")?;
            ui_response::negotiated_authorization(
                accept.as_deref(),
                authorize_page(&page).await,
                &markdown,
            )?
        }
        _ if path.starts_with("/s/") => {
            public_share(req, env, path.trim_start_matches("/s/")).await?
        }
        _ => return Ok(None),
    };
    Ok(Some(response))
}

fn router() -> Router {
    let mut builder = Router::builder();
    for path in PUBLIC_GET_ROUTES {
        for method in [HttpMethod::GET, HttpMethod::HEAD] {
            builder = builder.route(RouteFn::new(
                method,
                std::borrow::Cow::Owned(Path::new(path).to_owned()),
                topcoat_match,
            ));
        }
    }
    for (method, path) in [
        (HttpMethod::POST, "/oauth/register"),
        (HttpMethod::POST, "/auth/request"),
        (HttpMethod::POST, "/auth/verify"),
        (HttpMethod::POST, "/s/{id}/signin"),
        (HttpMethod::POST, "/s/{id}/verify"),
        (HttpMethod::POST, "/viewer/signout"),
        (HttpMethod::POST, "/oauth/token"),
        (HttpMethod::POST, "/oauth/revoke"),
        (HttpMethod::POST, "/webhooks/lemon-squeezy"),
        (HttpMethod::POST, "/checkout/monthly"),
        (HttpMethod::POST, "/checkout/annual"),
        (HttpMethod::POST, "/api/keys"),
        (HttpMethod::GET, "/api/keys"),
        (HttpMethod::DELETE, "/api/keys/{id}"),
        (HttpMethod::POST, "/api/log-reports"),
        (HttpMethod::GET, "/api/log-reports"),
        (HttpMethod::POST, "/api/shares"),
        (HttpMethod::GET, "/api/shares"),
        (HttpMethod::GET, "/api/shares/{id}/access"),
        (HttpMethod::PATCH, "/api/shares/{id}"),
        (HttpMethod::PUT, "/api/shares/{id}/members"),
        (HttpMethod::DELETE, "/api/shares/{id}/members/{memberId}"),
        (HttpMethod::POST, "/api/shares/{id}/transfer"),
        (HttpMethod::GET, "/api/billing"),
        (HttpMethod::POST, "/api/shares/{id}/blackouts"),
        (HttpMethod::DELETE, "/api/shares/{id}"),
        (HttpMethod::POST, "/mcp"),
    ] {
        builder = builder.route(RouteFn::new(
            method,
            std::borrow::Cow::Owned(Path::new(path).to_owned()),
            topcoat_match,
        ));
    }
    builder.build()
}

fn effective_method(method: Method) -> Method {
    if method == Method::Head {
        Method::Get
    } else {
        method
    }
}

fn share_item_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix("/api/shares/")?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

fn share_subresource_id<'a>(path: &'a str, suffix: &str) -> Option<&'a str> {
    let id = path.strip_prefix("/api/shares/")?.strip_suffix(suffix)?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

fn share_member_ids(path: &str) -> Option<(&str, &str)> {
    let value = path.strip_prefix("/api/shares/")?;
    let (id, member_id) = value.split_once("/members/")?;
    (!id.is_empty() && !member_id.is_empty() && !id.contains('/') && !member_id.contains('/'))
        .then_some((id, member_id))
}

fn viewer_share_id<'a>(path: &'a str, suffix: &str) -> Option<&'a str> {
    let id = path.strip_prefix("/s/")?.strip_suffix(suffix)?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

fn without_body(response: Response) -> Response {
    let (builder, _) = response.into_parts();
    builder.empty()
}

async fn healthz(env: &Env) -> Result<Response> {
    let healthy = match env.d1("DB") {
        Ok(db) => db.prepare("SELECT 1").run().await.is_ok(),
        Err(_) => false,
    };
    if !healthy {
        worker::console_error!("healthz dependency check failed");
    }

    let response = json_response(&serde_json::json!({
        "status": if healthy { "ok" } else { "unavailable" }
    }))?;
    Ok(if healthy {
        response
    } else {
        response.with_status(503)
    })
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
    #[serde(default)]
    general_access: GeneralAccess,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShareBlackoutInput {
    message: usize,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ShareBlackoutToolInput {
    id: String,
    message: usize,
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareBlackoutResponse {
    id: String,
    url: String,
    updated_at: String,
    message: usize,
    replacement: &'static str,
    redactions: usize,
}

async fn api_create_share(req: &mut Request, env: &Env) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        log_rejection("share_create", "authentication");
        return Response::error("invalid bearer token", 401);
    };
    if !has_scope(&user, "shares:write") {
        log_rejection("share_create", "scope");
        return Response::error("missing scope: shares:write", 403);
    }
    let input = req.json::<ShareInput>().await?;
    if input.general_access == GeneralAccess::Private {
        if !private_expansion_enabled(env) {
            log_rejection("share_create", "feature_disabled");
            return Response::error("private sharing is temporarily unavailable", 503);
        }
        if !user_is_pro(env, &user.user_id).await? {
            log_rejection("share_create", "pro_required");
            return Response::error("private sharing requires Pro", 402);
        }
        if !user_has_private_share_capacity(env, &user.user_id).await? {
            log_rejection("share_create", "plan_limit");
            return Response::error("private share limit reached", 402);
        }
    }
    let general_access = input.general_access;
    let share = Share {
        schema_version: input.schema_version,
        title: input.title,
        approved_at: input.approved_at,
        messages: input.messages,
        report: input.report,
    };
    if share.schema_version != footon_core::model::SCHEMA_VERSION {
        log_rejection("share_create", "schema");
        return Response::error("new shares must use footon.share.v2", 400);
    }
    if let Err(error) = validate_share(&share) {
        log_rejection("share_create", "validation");
        return Response::error(error.to_string(), 400);
    }
    if let Err(error) = validate_safe_share(&share) {
        log_rejection("share_create", "safety");
        return Response::error(error.to_string(), 400);
    }
    let id = token(18);
    let now = now_string();
    let document_json = serde_json::to_string(&share)?;
    let db = env.d1("DB")?;
    db.prepare(
        "INSERT INTO shares
         (id, owner_id, title, document_json, created_at, revoked_at, general_access)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
    )
    .bind_refs(&[
        D1Type::Text(&id),
        D1Type::Text(&user.user_id),
        D1Type::Text(&share.title),
        D1Type::Text(&document_json),
        D1Type::Text(&now),
        D1Type::Text(general_access.as_db_str()),
    ])?
    .run()
    .await?;

    json_response_with_status(
        &CreateShareResponse {
            id: id.clone(),
            url: format!("{ORIGIN}/s/{id}"),
            created_at: now,
            general_access,
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
            "SELECT id, title, created_at, general_access
             FROM shares
             WHERE owner_id = ?1 AND revoked_at IS NULL
             ORDER BY created_at DESC
             LIMIT 50",
        )
        .bind_refs(&[D1Type::Text(&user.user_id)])?
        .all()
        .await?
        .results::<ListShareDbRow>()?
        .into_iter()
        .map(ListShareRow::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    json_response(&rows)
}

async fn api_billing_status(req: &Request, env: &Env) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    billing_status(env, &user.user_id).await
}

async fn api_blackout_share(req: &mut Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        log_rejection("share_blackout", "authentication");
        return Response::error("invalid bearer token", 401);
    };
    if !has_scope(&user, "shares:write") {
        log_rejection("share_blackout", "scope");
        return Response::error("missing scope: shares:write", 403);
    }
    let Some(access) = load_share_access(env, id, Some(&user.user_id), Some(&user.email)).await?
    else {
        log_rejection("share_blackout", "not_found");
        return Response::error("not found", 404);
    };
    if !access.allows(ShareAction::Blackout) {
        log_rejection("share_blackout", "not_found");
        return Response::error("not found", 404);
    }
    if !action_is_available_for_plan(env, &access, ShareAction::Blackout, None).await? {
        log_rejection("share_blackout", "plan");
        return Response::error("private sharing requires Pro", 402);
    }
    let input = req.json::<ShareBlackoutInput>().await?;
    let Some(response) = blackout_owned_share(env, &user, id, &input).await? else {
        log_rejection("share_blackout", "not_found");
        return Response::error("not found", 404);
    };
    match response {
        Ok(response) => json_response(&response),
        Err(message) => {
            log_rejection("share_blackout", "validation");
            Response::error(message, 400)
        }
    }
}

async fn blackout_owned_share(
    env: &Env,
    actor: &AuthUser,
    id: &str,
    input: &ShareBlackoutInput,
) -> Result<Option<std::result::Result<ShareBlackoutResponse, String>>> {
    validate_share_id(id)?;
    let Some(access) = load_share_access(env, id, Some(&actor.user_id), Some(&actor.email)).await?
    else {
        return Ok(None);
    };
    if !access.allows(ShareAction::Blackout) {
        return Ok(None);
    }
    if !action_is_available_for_plan(env, &access, ShareAction::Blackout, None).await? {
        return Ok(Some(Err("private sharing requires Pro".to_string())));
    }
    let Some(record) = load_share(env, id).await? else {
        return Ok(None);
    };
    let mut share = Share {
        schema_version: record.document.schema_version,
        title: record.document.title,
        approved_at: record.document.approved_at,
        messages: record.document.messages,
        report: record.document.report,
    };
    let outcome = match blackout(
        &mut share.messages,
        &mut share.report,
        input.message,
        &input.text,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return Ok(Some(Err(error.to_string()))),
    };
    if let Err(error) = validate_share(&share) {
        return Ok(Some(Err(error.to_string())));
    }
    if let Err(error) = validate_safe_share(&share) {
        return Ok(Some(Err(error.to_string())));
    }

    let updated_at = now_string();
    let document_json = serde_json::to_string(&share)?;
    env.d1("DB")?
        .prepare(
            "UPDATE shares SET document_json = ?1
             WHERE id = ?2 AND revoked_at IS NULL AND (
               owner_id = ?3 OR EXISTS (
                 SELECT 1 FROM share_members
                 WHERE share_id = ?2 AND email = ?4 AND role = 'editor'
               )
             )",
        )
        .bind_refs(&[
            D1Type::Text(&document_json),
            D1Type::Text(id),
            D1Type::Text(&actor.user_id),
            D1Type::Text(&actor.email),
        ])?
        .run()
        .await?;
    Ok(Some(Ok(ShareBlackoutResponse {
        id: id.to_string(),
        url: format!("{ORIGIN}/s/{id}"),
        updated_at,
        message: outcome.message,
        replacement: BLACKOUT_TEXT,
        redactions: outcome.redactions,
    })))
}

async fn api_revoke_share(req: &Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = bearer_user(req, env).await? else {
        log_rejection("share_revoke", "authentication");
        return Response::error("invalid bearer token", 401);
    };
    if !has_scope(&user, "shares:write") {
        log_rejection("share_revoke", "scope");
        return Response::error("missing scope: shares:write", 403);
    }
    validate_share_id(id)?;
    let Some(access) = load_share_access(env, id, Some(&user.user_id), Some(&user.email)).await?
    else {
        return Response::error("not found", 404);
    };
    if !access.allows(ShareAction::Revoke) {
        return Response::error("not found", 404);
    }
    let db = env.d1("DB")?;
    let statements = vec![
        db.prepare("DELETE FROM share_members WHERE share_id = ?1")
            .bind_refs(&[D1Type::Text(id)])?,
        db.prepare("DELETE FROM share_viewer_challenges WHERE share_id = ?1")
            .bind_refs(&[D1Type::Text(id)])?,
        db.prepare("UPDATE shares SET revoked_at = ?1 WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL")
            .bind_refs(&[
                D1Type::Text(&now_string()),
                D1Type::Text(id),
                D1Type::Text(&user.user_id),
            ])?,
    ];
    let results = db.batch(statements).await?;
    if results
        .last()
        .and_then(|result| result.meta().ok().flatten())
        .and_then(|meta| meta.changes)
        == Some(0)
    {
        return Response::error("not found", 404);
    }
    Response::empty()
}

async fn public_share(req: &Request, env: &Env, id: &str) -> Result<Response> {
    validate_share_id(id)?;
    let user = bearer_user(req, env).await?;
    let browser = if user.is_none() {
        access::browser_principal(req, env).await?
    } else {
        None
    };
    let Some(share_access) = load_share_access(
        env,
        id,
        user.as_ref()
            .map(|user| user.user_id.as_str())
            .or_else(|| browser.as_ref().map(|user| user.user_id.as_str())),
        user.as_ref()
            .map(|user| user.email.as_str())
            .or_else(|| browser.as_ref().map(|user| user.email.as_str())),
    )
    .await?
    else {
        return Response::error("not found", 404);
    };
    if !share_access.allows(ShareAction::Read) {
        let content_type = negotiate(req.headers().get("Accept")?.as_deref());
        if share_access.general_access == GeneralAccess::Private
            && user.is_none()
            && browser.is_none()
            && content_type == Some(ContentType::Html)
        {
            return access::viewer_signin_page(id);
        }
        let anonymous = user.is_none() && browser.is_none();
        return Response::error(
            if anonymous {
                "authentication required"
            } else {
                "not found"
            },
            if anonymous { 401 } else { 404 },
        );
    }
    if !action_is_available_for_plan(env, &share_access, ShareAction::Read, None).await? {
        return Response::error("private sharing requires Pro", 402);
    }
    let Some(record) = load_share(env, id).await? else {
        return Response::error("not found", 404);
    };
    let mut response = match negotiate(req.headers().get("Accept")?.as_deref()) {
        None => Response::error("not acceptable", 406),
        Some(ContentType::Markdown) => {
            ui_response::markdown(&messages_to_markdown(&record.document))
        }
        Some(ContentType::Html) => {
            let text_mode = req
                .url()?
                .query_pairs()
                .any(|(key, value)| key == "view" && value == "text");
            ui_response::standard(viewer_page(&record, text_mode).await)
        }
    }?;
    if share_access.general_access == GeneralAccess::Private {
        response
            .headers_mut()
            .set("Cache-Control", "private, no-store")?;
    }
    Ok(response)
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
    general_access: GeneralAccess,
}

#[derive(Debug, Deserialize)]
struct ListShareDbRow {
    id: String,
    title: String,
    created_at: String,
    general_access: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListShareRow {
    id: String,
    title: String,
    created_at: String,
    general_access: GeneralAccess,
}

impl TryFrom<ListShareDbRow> for ListShareRow {
    type Error = worker::Error;

    fn try_from(row: ListShareDbRow) -> std::result::Result<Self, Self::Error> {
        let general_access = GeneralAccess::from_db(&row.general_access)
            .ok_or_else(|| worker::Error::RustError("invalid stored general access".to_string()))?;
        Ok(Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            general_access,
        })
    }
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
    let scope = clean_scope(
        input
            .scope
            .as_deref()
            .unwrap_or("keys:manage logs:read shares:read shares:write"),
    )?;
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthRequestResponse {
    ok: bool,
    ticket: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct CountRow {
    count: i32,
}

#[derive(Debug, Deserialize)]
struct AuthVerifyRequest {
    ticket: String,
    code: String,
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
    verification_code_hash: String,
    attempts: i32,
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
    email: String,
    scope: String,
    expires_at: String,
    revoked_at: Option<String>,
}

async fn auth_request(req: &mut Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    let content_type = req.headers().get("Content-Type")?.unwrap_or_default();
    let input = if content_type.starts_with("application/json") {
        serde_json::from_str::<AuthRequest>(&body)?
    } else {
        serde_urlencoded::from_str::<AuthRequest>(&body)?
    };
    if input.code_challenge_method != "S256" {
        log_rejection("auth_request", "pkce_method");
        return Response::error("S256 PKCE is required", 400);
    }
    let Some(email) = normalize_email(&input.email) else {
        log_rejection("auth_request", "email");
        return Response::error("enter a valid email address", 400);
    };
    let client = load_client(env, &input.client_id).await?;
    if !client.redirect_uris.contains(&input.redirect_uri) {
        log_rejection("auth_request", "redirect_uri");
        return Response::error("redirect_uri is not registered", 400);
    }
    validate_resource(&input.resource)?;
    let scope = clean_scope(
        input
            .scope
            .as_deref()
            .unwrap_or("keys:manage logs:read shares:read shares:write"),
    )?;
    if !scope_is_subset(&scope, &client.scope) {
        log_rejection("auth_request", "scope");
        return Response::error("requested scope exceeds client registration", 400);
    }
    let resend_cutoff = time_string(unix_now() - EMAIL_CODE_RESEND_COOLDOWN_SECONDS);
    let recent = env
        .d1("DB")?
        .prepare(
            "SELECT COUNT(*) AS count
             FROM oauth_magic_links_v2
             WHERE email = ?1 AND created_at >= ?2 AND consumed_at IS NULL",
        )
        .bind_refs(&[D1Type::Text(&email), D1Type::Text(&resend_cutoff)])?
        .first::<CountRow>(None)
        .await?
        .map_or(0, |row| row.count);
    if recent > 0 {
        log_rejection("auth_request", "rate_limit");
        return Response::error("a code was sent recently; try again shortly", 429);
    }
    let ticket = token(32);
    let ticket_hash = hash(&ticket);
    let verification_code = email_code();
    let verification_code_hash = hash(&verification_code);
    let created_at = now_string();
    let expires_at = time_string(unix_now() + EMAIL_CODE_TTL_SECONDS);
    env.d1("DB")?
        .prepare(
            "INSERT INTO oauth_magic_links_v2
             (ticket_hash, email, client_id, redirect_uri, scope, code_challenge, state, resource, created_at, expires_at, consumed_at, verification_code_hash, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, 0)",
        )
        .bind_refs(&[
            D1Type::Text(&ticket_hash),
            D1Type::Text(&email),
            D1Type::Text(&input.client_id),
            D1Type::Text(&input.redirect_uri),
            D1Type::Text(&scope),
            D1Type::Text(&input.code_challenge),
            D1Type::Text(input.state.as_deref().unwrap_or_default()),
            D1Type::Text(&input.resource),
            D1Type::Text(&created_at),
            D1Type::Text(&expires_at),
            D1Type::Text(&verification_code_hash),
        ])?
        .run()
        .await?;
    send_email_code(env, &email, &verification_code).await?;
    if content_type.starts_with("application/json") {
        json_response(&AuthRequestResponse {
            ok: true,
            ticket,
            expires_in: EMAIL_CODE_TTL_SECONDS,
        })
    } else {
        let accept = req.headers().get("Accept")?;
        ui_response::negotiated_authorization(
            accept.as_deref(),
            verification_page(&VerificationPage { ticket }).await,
            verification_markdown(),
        )
    }
}

async fn auth_verify(req: &mut Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    let content_type = req.headers().get("Content-Type")?.unwrap_or_default();
    let input = if content_type.starts_with("application/json") {
        serde_json::from_str::<AuthVerifyRequest>(&body)?
    } else {
        serde_urlencoded::from_str::<AuthVerifyRequest>(&body)?
    };
    let Some(code) = normalize_email_code(&input.code) else {
        log_rejection("auth_verify", "code");
        return Response::error("invalid or expired code", 400);
    };
    let ticket_hash = hash(&input.ticket);
    let verification_code_hash = hash(code);
    let db = env.d1("DB")?;
    let row = db
        .prepare(
            "SELECT email, client_id, redirect_uri, scope, code_challenge, state, resource, expires_at, verification_code_hash, attempts
             FROM oauth_magic_links_v2
             WHERE ticket_hash = ?1 AND consumed_at IS NULL",
        )
        .bind_refs(&[D1Type::Text(&ticket_hash)])?
        .first::<MagicRow>(None)
        .await?;
    let Some(row) = row else {
        log_rejection("auth_verify", "ticket");
        return Response::error("invalid or expired code", 400);
    };
    if parse_time(&row.expires_at).timestamp() < unix_now()
        || row.attempts >= MAX_EMAIL_CODE_ATTEMPTS
    {
        log_rejection("auth_verify", "expired_or_attempts");
        return Response::error("invalid or expired code", 400);
    }
    if row.verification_code_hash != verification_code_hash {
        db.prepare(
            "UPDATE oauth_magic_links_v2
             SET attempts = attempts + 1
             WHERE ticket_hash = ?1 AND consumed_at IS NULL AND attempts < ?2",
        )
        .bind_refs(&[
            D1Type::Text(&ticket_hash),
            D1Type::Integer(MAX_EMAIL_CODE_ATTEMPTS),
        ])?
        .run()
        .await?;
        log_rejection("auth_verify", "code");
        return Response::error("invalid or expired code", 400);
    }
    let code = token(32);
    let code_hash = hash(&code);
    let user_id = format!("email:{}", row.email.to_ascii_lowercase());
    let now = now_string();
    let expires_at = time_string(unix_now() + CODE_TTL_SECONDS);
    let consumed = db
        .prepare(
            "UPDATE oauth_magic_links_v2
             SET consumed_at = ?1
             WHERE ticket_hash = ?2 AND verification_code_hash = ?3 AND consumed_at IS NULL AND attempts < ?4",
        )
        .bind_refs(&[
            D1Type::Text(&now),
            D1Type::Text(&ticket_hash),
            D1Type::Text(&verification_code_hash),
            D1Type::Integer(MAX_EMAIL_CODE_ATTEMPTS),
        ])?
        .run()
        .await?;
    if consumed.meta()?.and_then(|meta| meta.changes) != Some(1) {
        log_rejection("auth_verify", "replay");
        return Response::error("invalid or expired code", 400);
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
    redirect_response(&redirect)
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
enum CredentialKind {
    Interactive,
    Service { key_id: String, system: String },
}

#[derive(Debug, Clone)]
struct AuthUser {
    user_id: String,
    email: String,
    scope: String,
    credential: CredentialKind,
}

impl AuthUser {
    const fn is_interactive(&self) -> bool {
        matches!(self.credential, CredentialKind::Interactive)
    }
}

async fn bearer_user(req: &Request, env: &Env) -> Result<Option<AuthUser>> {
    let auth = req.headers().get("Authorization")?.unwrap_or_default();
    let Some(token_value) = auth.strip_prefix("Bearer ") else {
        return Ok(None);
    };
    let token_hash = hash(token_value);
    let row = env
        .d1("DB")?
        .prepare("SELECT user_id, email, scope, expires_at, revoked_at FROM oauth_access_tokens_v2 WHERE token_hash = ?1")
        .bind_refs(&[D1Type::Text(&token_hash)])?
        .first::<AccessTokenRow>(None)
        .await?;
    let Some(row) = row else {
        return systems::authenticate_service_key(env, &token_hash).await;
    };
    if row.revoked_at.is_some() || parse_time(&row.expires_at).timestamp() < unix_now() {
        return Ok(None);
    }
    Ok(Some(AuthUser {
        user_id: row.user_id,
        email: row.email,
        scope: row.scope,
        credential: CredentialKind::Interactive,
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
    if is_mcp_notification(&request) {
        return accepted_response();
    }
    let result = match request.method.as_str() {
        "initialize" => serde_json::json!({
            "protocolVersion": mcp_protocol_version(request.params.as_ref()),
            "serverInfo": { "name": "footon", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "tools": {} }
        }),
        "ping" => serde_json::json!({}),
        "tools/list" => serde_json::json!({
            "tools": mcp_tools()
        }),
        "tools/call" => match call_tool(env, &user, request.params).await {
            Ok(result) => mcp_tool_result(&result),
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

fn is_mcp_notification(request: &RpcRequest) -> bool {
    request.id.is_none()
}

fn mcp_protocol_version(params: Option<&serde_json::Value>) -> &str {
    params
        .and_then(|value| value.get("protocolVersion"))
        .and_then(serde_json::Value::as_str)
        .filter(|version| MCP_INITIALIZE_PROTOCOL_VERSIONS.contains(version))
        .unwrap_or("2025-11-25")
}

#[allow(clippy::too_many_lines)]
fn mcp_tools() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "share_create",
            "description": "Publish one approved sanitized share",
            "inputSchema": {
                "type": "object",
                "required": ["schemaVersion", "title", "approvedAt", "messages", "report"],
                "properties": {
                    "schemaVersion": { "type": "string", "const": "footon.share.v2" },
                    "title": { "type": "string" },
                    "approvedAt": { "type": "string", "format": "date-time" },
                    "messages": { "type": "array", "items": { "type": "object" } },
                    "report": { "type": "object" },
                    "generalAccess": { "type": "string", "enum": ["public", "private"], "default": "public" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "share_list",
            "description": "List your active Footon shares",
            "inputSchema": { "type": "object", "additionalProperties": false }
        },
        {
            "name": "share_access",
            "description": "Show a share owner, visibility, and private members",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } },
                "additionalProperties": false
            }
        },
        {
            "name": "share_update",
            "description": "Rename a share or change public/private visibility",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string", "minLength": 1, "maxLength": 160 },
                    "generalAccess": { "type": "string", "enum": ["public", "private"] }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "share_grant",
            "description": "Grant a Viewer or Editor role on a private share",
            "inputSchema": {
                "type": "object",
                "required": ["id", "email", "role"],
                "properties": {
                    "id": { "type": "string" },
                    "email": { "type": "string" },
                    "role": { "type": "string", "enum": ["viewer", "editor"] }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "share_remove",
            "description": "Remove one member from a private share",
            "inputSchema": {
                "type": "object",
                "required": ["id", "email"],
                "properties": {
                    "id": { "type": "string" },
                    "email": { "type": "string" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "share_transfer",
            "description": "Transfer ownership of one share",
            "inputSchema": {
                "type": "object",
                "required": ["id", "email"],
                "properties": {
                    "id": { "type": "string" },
                    "email": { "type": "string" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "share_blackout",
            "description": "Black out one exact substring in an owner-controlled live share",
            "inputSchema": {
                "type": "object",
                "required": ["id", "message", "text"],
                "properties": {
                    "id": { "type": "string" },
                    "message": { "type": "integer", "minimum": 0 },
                    "text": { "type": "string", "minLength": 1 }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "share_revoke",
            "description": "Revoke one Footon share",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } },
                "additionalProperties": false
            }
        },
        {
            "name": "service_key_create",
            "description": "Issue one scoped key for a named remote system; the secret is returned once",
            "inputSchema": {
                "type": "object",
                "required": ["name", "system"],
                "properties": {
                    "name": { "type": "string", "minLength": 1, "maxLength": 80 },
                    "system": { "type": "string", "minLength": 1, "maxLength": 64 },
                    "scope": { "type": "string", "default": "logs:write" },
                    "expiresInDays": { "type": "integer", "minimum": 1, "maximum": 365, "default": 90 }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "service_key_list",
            "description": "List service key metadata without returning secrets",
            "inputSchema": { "type": "object", "additionalProperties": false }
        },
        {
            "name": "service_key_revoke",
            "description": "Revoke one service key immediately",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "properties": { "id": { "type": "string" } },
                "additionalProperties": false
            }
        },
        {
            "name": "log_report_create",
            "description": "Submit one bounded, automatically redacted remote-system log report",
            "inputSchema": {
                "type": "object",
                "required": ["environment", "level", "event", "summary", "sourceEventId", "occurredAt"],
                "properties": {
                    "environment": { "type": "string", "minLength": 1, "maxLength": 64 },
                    "level": { "type": "string", "enum": ["debug", "info", "warn", "error", "critical"] },
                    "event": { "type": "string", "minLength": 1, "maxLength": 100 },
                    "summary": { "type": "string", "minLength": 1, "maxLength": 2000 },
                    "sourceEventId": { "type": "string", "minLength": 1, "maxLength": 160 },
                    "occurredAt": { "type": "string", "format": "date-time" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "log_report_list",
            "description": "List recent remote-system reports visible to this credential",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "system": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 50 }
                },
                "additionalProperties": false
            }
        }
    ])
}

fn mcp_tool_result(value: &serde_json::Value) -> serde_json::Value {
    let text = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
    serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": value
    })
}

#[allow(clippy::too_many_lines)]
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
        "service_key_create" => {
            require_scope(user, "keys:manage")?;
            systems::tool_create_key(env, user, args).await
        }
        "service_key_list" => {
            require_scope(user, "keys:manage")?;
            systems::tool_list_keys(env, user).await
        }
        "service_key_revoke" => {
            require_scope(user, "keys:manage")?;
            systems::tool_revoke_key(env, user, args).await
        }
        "log_report_create" => {
            require_scope(user, "logs:write")?;
            systems::tool_create_report(env, user, args).await
        }
        "log_report_list" => {
            require_scope(user, "logs:read")?;
            systems::tool_list_reports(env, user, args).await
        }
        "share_access" => {
            require_scope(user, "shares:read")?;
            shares::tool_access(env, user, args).await
        }
        "share_update" => {
            require_scope(user, "shares:write")?;
            shares::tool_update(env, user, args).await
        }
        "share_grant" => {
            require_scope(user, "shares:write")?;
            shares::tool_grant(env, user, args).await
        }
        "share_remove" => {
            require_scope(user, "shares:write")?;
            shares::tool_remove(env, user, args).await
        }
        "share_transfer" => {
            require_scope(user, "shares:write")?;
            shares::tool_transfer(env, user, args).await
        }
        "share_list" => {
            require_scope(user, "shares:read")?;
            let rows = env
                .d1("DB")?
                .prepare("SELECT id, title, created_at, general_access FROM shares WHERE owner_id = ?1 AND revoked_at IS NULL ORDER BY created_at DESC LIMIT 50")
                .bind_refs(&[D1Type::Text(&user.user_id)])?
                .all()
                .await?
                .results::<ListShareDbRow>()?
                .into_iter()
                .map(ListShareRow::try_from)
                .collect::<std::result::Result<Vec<_>, _>>()?;
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
        "share_blackout" => {
            require_scope(user, "shares:write")?;
            let tool_input = serde_json::from_value::<ShareBlackoutToolInput>(args)?;
            let input = ShareBlackoutInput {
                message: tool_input.message,
                text: tool_input.text,
            };
            blackout_owned_share(env, user, &tool_input.id, &input)
                .await?
                .ok_or_else(|| worker::Error::RustError("share not found".to_string()))?
                .map_err(worker::Error::RustError)
                .and_then(|response| serde_json::to_value(response).map_err(Into::into))
        }
        "share_create" => {
            require_scope(user, "shares:write")?;
            let mut args = args;
            let general_access = args
                .get("generalAccess")
                .cloned()
                .map(serde_json::from_value::<GeneralAccess>)
                .transpose()?
                .unwrap_or_default();
            if let Some(object) = args.as_object_mut() {
                object.remove("generalAccess");
            }
            if general_access == GeneralAccess::Private
                && (!private_expansion_enabled(env)
                    || !user_is_pro(env, &user.user_id).await?
                    || !user_has_private_share_capacity(env, &user.user_id).await?)
            {
                return Err(worker::Error::RustError(
                    "private sharing requires available Pro capacity".to_string(),
                ));
            }
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
                .prepare("INSERT INTO shares (id, owner_id, title, document_json, created_at, revoked_at, general_access) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)")
                .bind_refs(&[
                    D1Type::Text(&id),
                    D1Type::Text(&user.user_id),
                    D1Type::Text(&share.title),
                    D1Type::Text(&document_json),
                    D1Type::Text(&now),
                    D1Type::Text(general_access.as_db_str()),
                ])?
                .run()
                .await?;
            Ok(
                serde_json::json!({ "id": id, "url": format!("{ORIGIN}/s/{id}"), "createdAt": now, "generalAccess": general_access }),
            )
        }
        _ => Err(worker::Error::RustError("unknown tool".to_string())),
    }
}

fn authorization_page(url: &Url) -> AuthorizationPage {
    AuthorizationPage {
        client_id: query(url, "client_id").unwrap_or_default(),
        redirect_uri: query(url, "redirect_uri").unwrap_or_default(),
        scope: query(url, "scope")
            .unwrap_or_else(|| "keys:manage logs:read shares:read shares:write".to_string()),
        state: query(url, "state").unwrap_or_default(),
        code_challenge: query(url, "code_challenge").unwrap_or_default(),
        resource: query(url, "resource").unwrap_or_else(|| format!("{ORIGIN}/mcp")),
    }
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
        "scopes_supported": ["keys:manage", "logs:read", "logs:write", "shares:read", "shares:write"]
    })
}

fn protected_resource_metadata() -> serde_json::Value {
    serde_json::json!({
        "resource": format!("{ORIGIN}/mcp"),
        "authorization_servers": [ORIGIN],
        "scopes_supported": ["keys:manage", "logs:read", "logs:write", "shares:read", "shares:write"],
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
    if parts.iter().all(|scope| {
        matches!(
            *scope,
            "keys:manage" | "logs:read" | "logs:write" | "shares:read" | "shares:write"
        )
    }) {
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

fn blackout_share_id(path: &str) -> Option<&str> {
    path.strip_prefix("/api/shares/")?
        .strip_suffix("/blackouts")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

fn query(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
}

async fn send_email_code(env: &Env, email: &str, code: &str) -> Result<()> {
    let binding = env.send_email("EMAIL")?;
    let message = SendEmailBuilder::builder("login@footon.dev", email, "Your Footon code")
        .text(&format!(
            "Your Footon sign-in code is:\n\n{code}\n\nIt expires in 10 minutes and can be used once.\n"
        ))
        .build();
    binding.send_with_builder(&message).await?;
    Ok(())
}

fn normalize_email(value: &str) -> Option<String> {
    let email = value.trim().to_lowercase();
    if email.is_empty() || email.len() > 254 || email.chars().any(char::is_whitespace) {
        return None;
    }
    let (local, domain) = email.split_once('@')?;
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
    {
        return None;
    }
    let valid_domain = domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    });
    valid_domain.then_some(email)
}

fn json_response<T: Serialize>(value: &T) -> Result<Response> {
    json_response_with_status(value, 200)
}

fn json_response_with_status<T: Serialize>(value: &T, status: u16) -> Result<Response> {
    let mut response = Response::from_json(value)?.with_status(status);
    security_headers(&mut response)?;
    Ok(response)
}

fn accepted_response() -> Result<Response> {
    Ok(Response::empty()?.with_status(202))
}

fn redirect_response(location: &Url) -> Result<Response> {
    let mut response = Response::empty()?.with_status(302);
    response.headers_mut().set("Location", location.as_str())?;
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

fn plain_text_response(text: &str) -> Result<Response> {
    let mut response = Response::ok(text.to_string())?;
    response
        .headers_mut()
        .set("Content-Type", "text/plain; charset=utf-8")?;
    security_headers(&mut response)?;
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

fn svg_response(svg: &str) -> Result<Response> {
    let mut response = Response::ok(svg.to_string())?;
    response
        .headers_mut()
        .set("Content-Type", "image/svg+xml; charset=utf-8")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=86400")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}

fn font_response(font: &[u8]) -> Result<Response> {
    let mut response = Response::from_bytes(font.to_vec())?;
    response.headers_mut().set("Content-Type", "font/woff2")?;
    response
        .headers_mut()
        .set("Cache-Control", "public, max-age=31536000, immutable")?;
    response
        .headers_mut()
        .set("X-Content-Type-Options", "nosniff")?;
    Ok(response)
}

fn log_rejection(operation: &str, reason: &str) {
    worker::console_error!("operation={operation} result=rejected reason={reason}");
}

fn private_expansion_enabled(env: &Env) -> bool {
    let value = env
        .var("SHARE_ACCESS_WRITES_ENABLED")
        .ok()
        .map(|value| value.to_string());
    flag_enabled(value.as_deref())
}

fn flag_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
}

fn internal_error(_error: &worker::Error) -> Result<Response> {
    worker::console_error!("operation=request result=unavailable reason=internal");
    let mut response = Response::error("internal server error", 500)?;
    security_headers(&mut response)?;
    Ok(response)
}

fn security_headers(response: &mut Response) -> Result<()> {
    response.headers_mut().set("Content-Security-Policy", "default-src 'none'; style-src 'self'; script-src 'self'; img-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'")?;
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

fn email_code() -> String {
    const CODE_SPACE: u32 = 1_000_000;
    const UNBIASED_LIMIT: u32 = u32::MAX - (u32::MAX % CODE_SPACE);
    loop {
        let mut raw = [0_u8; 4];
        getrandom::fill(&mut raw).expect("secure random source");
        let value = u32::from_le_bytes(raw);
        if value < UNBIASED_LIMIT {
            return format!("{:06}", value % CODE_SPACE);
        }
    }
}

fn normalize_email_code(value: &str) -> Option<&str> {
    let value = value.trim();
    (value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_digit())).then_some(value)
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
    use scraper::{Html, Selector};

    use super::*;

    fn selector(value: &str) -> Selector {
        Selector::parse(value).expect("valid selector")
    }

    #[test]
    fn topcoat_router_accepts_only_registered_routes_and_methods() {
        let accepted = http::Request::get("https://footon.dev/s/abcdefghijklmnopqrst")
            .body(TopcoatBody::empty())
            .expect("request");
        let favicon = http::Request::get("https://footon.dev/favicon.svg")
            .body(TopcoatBody::empty())
            .expect("request");
        let rejected = http::Request::post("https://footon.dev/s/abcdefghijklmnopqrst")
            .body(TopcoatBody::empty())
            .expect("request");
        let blackout =
            http::Request::post("https://footon.dev/api/shares/abcdefghijklmnopqrst/blackouts")
                .body(TopcoatBody::empty())
                .expect("request");
        let verify_code = http::Request::post("https://footon.dev/auth/verify")
            .body(TopcoatBody::empty())
            .expect("request");
        let verify_link = http::Request::get("https://footon.dev/auth/verify?ticket=private")
            .body(TopcoatBody::empty())
            .expect("request");
        let billing_webhook = http::Request::post("https://footon.dev/webhooks/lemon-squeezy")
            .body(TopcoatBody::empty())
            .expect("request");
        let billing_webhook_get = http::Request::get("https://footon.dev/webhooks/lemon-squeezy")
            .body(TopcoatBody::empty())
            .expect("request");
        let checkout_monthly = http::Request::post("https://footon.dev/checkout/monthly")
            .body(TopcoatBody::empty())
            .expect("request");
        let checkout_annual = http::Request::post("https://footon.dev/checkout/annual")
            .body(TopcoatBody::empty())
            .expect("request");
        let checkout_get = http::Request::get("https://footon.dev/checkout/monthly")
            .body(TopcoatBody::empty())
            .expect("request");
        let billing_status = http::Request::get("https://footon.dev/api/billing")
            .body(TopcoatBody::empty())
            .expect("request");
        let billing_status_post = http::Request::post("https://footon.dev/api/billing")
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
            runtime.block_on(router().handle(favicon)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime.block_on(router().handle(blackout)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime.block_on(router().handle(verify_code)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime.block_on(router().handle(billing_webhook)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime
                .block_on(router().handle(billing_webhook_get))
                .status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            runtime.block_on(router().handle(checkout_monthly)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime.block_on(router().handle(checkout_annual)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime.block_on(router().handle(checkout_get)).status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            runtime.block_on(router().handle(billing_status)).status(),
            http::StatusCode::NO_CONTENT
        );
        assert_eq!(
            runtime
                .block_on(router().handle(billing_status_post))
                .status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            runtime.block_on(router().handle(verify_link)).status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            runtime.block_on(router().handle(rejected)).status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[test]
    fn topcoat_router_accepts_health_and_head_for_every_public_get() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");

        for path in [
            "/healthz",
            "/",
            "/privacy",
            "/terms",
            "/pricing",
            "/security",
            "/support",
            "/llms.txt",
            "/robots.txt",
            "/style.css",
            "/viewer.js",
            "/landing.js",
            "/favicon.svg",
            "/fonts/departure-mono-1.500.woff2",
            "/.well-known/oauth-authorization-server",
            "/.well-known/oauth-protected-resource/mcp",
            "/authorize",
            "/s/abcdefghijklmnopqrst",
        ] {
            let head = http::Request::head(format!("https://footon.dev{path}"))
                .body(TopcoatBody::empty())
                .expect("request");
            assert_eq!(
                runtime.block_on(router().handle(head)).status(),
                http::StatusCode::NO_CONTENT,
                "HEAD {path}"
            );
        }

        let health_get = http::Request::get("https://footon.dev/healthz")
            .body(TopcoatBody::empty())
            .expect("request");
        assert_eq!(
            runtime.block_on(router().handle(health_get)).status(),
            http::StatusCode::NO_CONTENT
        );

        let health_post = http::Request::post("https://footon.dev/healthz")
            .body(TopcoatBody::empty())
            .expect("request");
        assert_eq!(
            runtime.block_on(router().handle(health_post)).status(),
            http::StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[test]
    fn head_uses_get_semantics() {
        assert_eq!(effective_method(Method::Head), Method::Get);
        assert_eq!(effective_method(Method::Post), Method::Post);
    }

    #[test]
    fn private_share_capacity_rejects_free_and_exactly_at_the_pro_limit() {
        assert_eq!(billing_adapter::FREE_PRIVATE_SHARE_LIMIT, 0);
        assert!(!billing_adapter::has_private_share_capacity(0, 0));
        assert!(billing_adapter::has_private_share_capacity(0, 100));
        assert!(billing_adapter::has_private_share_capacity(99, 100));
        assert!(!billing_adapter::has_private_share_capacity(100, 100));
        assert!(!billing_adapter::has_private_share_capacity(101, 100));
    }

    #[test]
    fn private_expansion_flag_fails_closed() {
        assert!(flag_enabled(Some("true")));
        assert!(flag_enabled(Some("1")));
        assert!(!flag_enabled(Some("false")));
        assert!(!flag_enabled(None));
    }

    #[test]
    fn checkout_url_is_bounded_and_associates_the_normalized_buyer() {
        let url = billing_adapter::build_checkout_url(
            "https://footon.lemonsqueezy.com/checkout/buy/monthly?discount=launch",
            " Buyer@Example.COM ",
        )
        .expect("checkout URL");
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(pairs.get("discount").map(AsRef::as_ref), Some("launch"));
        assert_eq!(
            pairs.get("checkout[email]").map(AsRef::as_ref),
            Some("buyer@example.com")
        );
        assert_eq!(
            pairs.get("checkout[custom][email]").map(AsRef::as_ref),
            Some("buyer@example.com")
        );
        assert_eq!(
            pairs.get("checkout[custom][user_id]").map(AsRef::as_ref),
            Some("email:buyer@example.com")
        );
        assert!(
            billing_adapter::build_checkout_url(
                "https://attacker.example/checkout/buy/monthly",
                "buyer@example.com"
            )
            .is_err()
        );
    }

    #[test]
    fn scopes_are_canonical_and_cannot_expand() {
        assert_eq!(
            clean_scope("shares:write shares:read shares:read").expect("scope"),
            "shares:read shares:write"
        );
        assert_eq!(
            clean_scope("logs:read keys:manage").expect("interactive scopes"),
            "keys:manage logs:read"
        );
        assert!(scope_is_subset("shares:read", "shares:read shares:write"));
        assert!(scope_is_subset(
            "logs:read",
            "keys:manage logs:read shares:read"
        ));
        assert!(!scope_is_subset("shares:write", "shares:read"));
        assert!(clean_scope("read write").is_err());
    }

    #[test]
    fn share_ids_and_redirects_are_bounded() {
        assert!(validate_share_id("abcdefghijklmnopqrst").is_ok());
        assert!(validate_share_id("short").is_err());
        assert_eq!(
            blackout_share_id("/api/shares/abcdefghijklmnopqrst/blackouts"),
            Some("abcdefghijklmnopqrst")
        );
        assert_eq!(blackout_share_id("/api/shares/x/blackouts/more"), None);
        assert!(validate_redirect_uri("https://agent.example/callback").is_ok());
        assert!(validate_redirect_uri("http://localhost:4000/callback").is_ok());
        assert!(validate_redirect_uri("http://agent.example/callback").is_err());
        assert!(validate_redirect_uri("https://agent.example/callback#fragment").is_err());
    }

    #[test]
    fn email_codes_are_six_digits_and_stored_as_hashes() {
        for _ in 0..100 {
            let code = email_code();
            assert_eq!(code.len(), 6);
            assert!(code.bytes().all(|byte| byte.is_ascii_digit()));
        }
        assert_eq!(normalize_email_code(" 123456 "), Some("123456"));
        assert_eq!(normalize_email_code("12345"), None);
        assert_eq!(normalize_email_code("12345x"), None);

        let migration = include_str!("../../migrations/0004_email_codes.sql");
        assert!(migration.contains("verification_code_hash"));
        assert!(migration.contains("attempts"));
        assert!(!migration.contains("verification_code TEXT"));
    }

    #[test]
    fn email_addresses_are_normalized_before_throttling_and_delivery() {
        assert_eq!(
            normalize_email(" Test.User@Example.com "),
            Some("test.user@example.com".to_string())
        );
        assert_eq!(normalize_email("missing-at.example.com"), None);
        assert_eq!(normalize_email("two@@example.com"), None);
        assert_eq!(normalize_email("person@example"), None);
        assert_eq!(normalize_email("person@-example.com"), None);
        assert_eq!(
            normalize_email(&format!("{}@example.com", "a".repeat(65))),
            None
        );
    }

    #[test]
    fn oauth_redirect_uses_a_mutable_worker_response() {
        let source = include_str!("lib.rs");
        let start = source
            .find("fn redirect_response")
            .expect("redirect helper");
        let end = source[start..]
            .find("fn css_response")
            .map(|offset| start + offset)
            .expect("next helper");
        let implementation = &source[start..end];
        let immutable_helper = ["Response::", "redirect_with_status"].concat();

        assert!(!implementation.contains(&immutable_helper));
        assert!(implementation.contains("Response::empty()?.with_status(302)"));
        assert!(implementation.contains("response.headers_mut().set(\"Location\""));
    }

    #[test]
    fn mcp_lifecycle_and_tool_contracts_match_streamable_http() {
        let initialized = serde_json::from_value::<RpcRequest>(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .expect("initialized notification");
        assert!(is_mcp_notification(&initialized));

        assert_eq!(
            mcp_protocol_version(Some(&serde_json::json!({
                "protocolVersion": "2025-06-18"
            }))),
            "2025-06-18"
        );
        assert_eq!(
            mcp_protocol_version(Some(&serde_json::json!({
                "protocolVersion": "2026-07-28"
            }))),
            "2025-11-25"
        );

        let tools = mcp_tools();
        let tools = tools.as_array().expect("tool list");
        assert_eq!(tools.len(), 14);
        assert!(tools.iter().all(|tool| tool.get("inputSchema").is_some()));
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "share_create",
                "share_list",
                "share_access",
                "share_update",
                "share_grant",
                "share_remove",
                "share_transfer",
                "share_blackout",
                "share_revoke",
                "service_key_create",
                "service_key_list",
                "service_key_revoke",
                "log_report_create",
                "log_report_list",
            ]
        );

        let result = mcp_tool_result(&serde_json::json!({ "ok": true }));
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(result["structuredContent"]["ok"], true);
    }

    #[tokio::test]
    async fn home_page_explains_and_installs_prompt_chain_sharing() {
        let html = home_page().await.expect("home page").render(&Cx::default());
        let document = Html::parse_document(&html);
        let page_text = document.root_element().text().collect::<String>();
        let commands = document
            .select(&selector("pre code"))
            .map(|node| node.text().collect::<String>())
            .collect::<Vec<_>>();

        assert_eq!(
            document
                .select(&selector("h1"))
                .next()
                .expect("heading")
                .text()
                .collect::<String>(),
            "Share the thread. Keep your secrets."
        );
        assert!(page_text.contains("You keep the raw transcript local"));
        assert!(page_text.contains("publishes only the safe copy you approve"));
        assert!(page_text.contains("original transcript stays local"));
        for role in ["USER", "AGENT", "TOOL", "FILE"] {
            assert!(page_text.contains(role), "missing {role} demo row");
        }
        assert_eq!(
            document.select(&selector(".thread-demo .message")).count(),
            15
        );
        assert_eq!(
            document
                .select(&selector(".thread-demo[data-thread-scroll=\"true\"]"))
                .count(),
            1
        );
        assert_eq!(
            document.select(&selector(".thread-demo .minimap")).count(),
            1
        );
        assert_eq!(
            document
                .select(&selector(".thread-demo .filter-input"))
                .count(),
            3
        );
        assert_eq!(
            document
                .select(&selector(
                    "script[src=\"/viewer.js?v=20260818-departure-mono\"]",
                ))
                .count(),
            1
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("cargo install footon --locked"))
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("footon draft thread.jsonl"))
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("footon publish footon-draft.json"))
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("footon fetch https://footon.dev/s/..."))
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("footon blackout footon-draft.json"))
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("footon blackout-share https://footon.dev/s/..."))
        );
        for removed in [
            "FOR AGENTS",
            "FOOTON / SAFE AGENT HANDOFFS",
            "LOCAL RAW · SERVER RESCAN · MARKDOWN NATIVE",
            "Connect MCP",
            "Install details",
        ] {
            assert!(!page_text.contains(removed), "still contains {removed}");
        }
        assert_eq!(document.select(&selector("a[href='/privacy']")).count(), 1);
        assert_eq!(document.select(&selector("a[href='/terms']")).count(), 1);
        assert_eq!(
            document
                .select(&selector("link[rel=stylesheet]"))
                .next()
                .and_then(|node| node.value().attr("href")),
            Some("/style.css?v=20260818-departure-mono")
        );
    }

    #[tokio::test]
    async fn home_page_has_a_copyable_agent_install_prompt() {
        let html = home_page().await.expect("home page").render(&Cx::default());
        let document = Html::parse_document(&html);
        let prompt = document
            .select(&selector("#install-agent-prompt"))
            .next()
            .expect("agent prompt");
        let button = document
            .select(&selector("button[data-copy-target]"))
            .next()
            .expect("copy button");

        assert!(
            prompt
                .text()
                .collect::<String>()
                .contains("Install Footon for this workspace")
        );
        assert_eq!(
            button.value().attr("data-copy-target"),
            Some("install-agent-prompt")
        );
        assert_eq!(button.text().collect::<String>(), "COPY PROMPT");
        assert_eq!(
            document
                .select(&selector(
                    "script[src='/landing.js?v=20260818-departure-mono']",
                ))
                .count(),
            1
        );
        for contract in [
            "navigator.clipboard",
            "data-copy-target",
            "COPIED",
            "COPY FAILED",
        ] {
            assert!(LANDING_JS.contains(contract), "missing {contract}");
        }
    }

    #[tokio::test]
    async fn home_page_includes_complete_mcp_connection_details() {
        let html = home_page().await.expect("home page").render(&Cx::default());
        let document = Html::parse_document(&html);
        let page_text = document.root_element().text().collect::<String>();

        for detail in [
            "https://footon.dev/mcp",
            "Authorization code + PKCE S256",
            "shares:read shares:write",
        ] {
            assert!(page_text.contains(detail), "missing {detail}");
        }
    }

    #[tokio::test]
    async fn public_copy_does_not_expose_internal_implementation_details() {
        let rendered_pages = [
            home_page().await.expect("home page").render(&Cx::default()),
            privacy_page()
                .await
                .expect("privacy page")
                .render(&Cx::default()),
            terms_page()
                .await
                .expect("terms page")
                .render(&Cx::default()),
        ]
        .join("\n");
        let public_copy = format!(
            "{rendered_pages}\n{}\n{}\n{}\n{}\n{}",
            home_markdown(),
            privacy_markdown(),
            terms_markdown(),
            verification_markdown(),
            llms_markdown(),
        );

        for internal_detail in [
            "Incurs",
            "Code Mode",
            "typed commands",
            "server-side",
            "before storage",
            "Worker version",
            "workers-rs",
            "Topcoat",
            "Turnstile",
            "database record",
            "hashed authentication records",
        ] {
            assert!(
                !public_copy.contains(internal_detail),
                "public copy exposes {internal_detail}"
            );
        }
        assert!(privacy_markdown().contains("Cloudflare"));
        assert!(privacy_markdown().contains("Lemon Squeezy"));
    }

    #[test]
    fn every_browser_page_has_an_agent_first_markdown_version() {
        let pages = [
            home_markdown(),
            privacy_markdown(),
            terms_markdown(),
            verification_markdown(),
        ];

        for markdown in pages {
            assert!(markdown.starts_with("# "));
            assert!(markdown.contains("Footon"));
        }
        assert!(home_markdown().contains("## Actual Footon output"));
        assert!(home_markdown().contains("## USER"));
        assert!(home_markdown().contains("## AGENT"));

        let authorization = authorization_markdown(&AuthorizationPage {
            client_id: "private-client".to_string(),
            redirect_uri: "https://agent.example/callback".to_string(),
            scope: "shares:read shares:write".to_string(),
            state: "private-state".to_string(),
            code_challenge: "private-challenge".to_string(),
            resource: "https://footon.dev/mcp".to_string(),
        });
        assert!(authorization.contains("shares:read shares:write"));
        assert!(authorization.contains("https://footon.dev/mcp"));
        assert!(!authorization.contains("private-client"));
        assert!(!authorization.contains("private-state"));
        assert!(!authorization.contains("private-challenge"));
    }

    #[tokio::test]
    async fn favicon_uses_the_green_footon_icon() {
        let html = home_page().await.expect("home page").render(&Cx::default());

        assert!(html.contains("<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\">"));
        assert!(ICON.contains("#72e39f"));
        assert!(ICON.contains("A footprint pressed onto a folded sheet of paper"));
    }

    #[tokio::test]
    async fn authorization_page_preserves_form_contract_and_escapes_values() {
        let html = authorize_page(&AuthorizationPage {
            client_id: "client\" autofocus=\"true".to_string(),
            redirect_uri: "https://agent.example/callback?x=<unsafe>".to_string(),
            scope: "shares:read shares:write".to_string(),
            state: "state&value".to_string(),
            code_challenge: "challenge".to_string(),
            resource: "https://footon.dev/mcp".to_string(),
        })
        .await
        .expect("authorization page")
        .render(&Cx::default());
        let document = Html::parse_document(&html);
        let form = document.select(&selector("form")).next().expect("form");

        assert_eq!(form.value().attr("method"), Some("post"));
        assert_eq!(form.value().attr("action"), Some("/auth/request"));
        for name in [
            "client_id",
            "redirect_uri",
            "scope",
            "state",
            "code_challenge",
            "code_challenge_method",
            "resource",
        ] {
            assert_eq!(
                form.select(&selector(&format!("input[name='{name}']")))
                    .count(),
                1
            );
        }
        assert_eq!(form.select(&selector(".cf-turnstile")).count(), 0);
        assert_eq!(form.select(&selector("script")).count(), 0);
        assert!(
            form.select(&selector("#email"))
                .next()
                .and_then(|node| node.value().attr("class"))
                .is_some_and(|classes| classes
                    .split_whitespace()
                    .any(|class| class == "authorization-input"))
        );
        assert!(html.contains("client&quot; autofocus=&quot;true"));
        assert!(html.contains("state&amp;value"));
        assert!(!html.contains("value=\"client\" autofocus=\"true\""));
    }

    #[tokio::test]
    async fn authorization_flow_uses_an_email_code_the_agent_can_submit() {
        let authorize_html = authorize_page(&AuthorizationPage {
            client_id: "client".to_string(),
            redirect_uri: "https://agent.example/callback".to_string(),
            scope: "shares:read shares:write".to_string(),
            state: "state".to_string(),
            code_challenge: "challenge".to_string(),
            resource: "https://footon.dev/mcp".to_string(),
        })
        .await
        .expect("authorization page")
        .render(&Cx::default());
        let authorize_document = Html::parse_document(&authorize_html);
        assert_eq!(
            authorize_document
                .select(&selector("button[type='submit']"))
                .next()
                .expect("submit button")
                .text()
                .collect::<String>(),
            "Send code"
        );

        let verify_html =
            ui::authorization::verification_page(&ui::authorization::VerificationPage {
                ticket: "private-ticket".to_string(),
            })
            .await
            .expect("verification page")
            .render(&Cx::default());
        let verify_document = Html::parse_document(&verify_html);
        let form = verify_document
            .select(&selector("form"))
            .next()
            .expect("verification form");
        assert_eq!(form.value().attr("method"), Some("post"));
        assert_eq!(form.value().attr("action"), Some("/auth/verify"));
        let code = form
            .select(&selector("input[name='code']"))
            .next()
            .expect("code input");
        assert_eq!(code.value().attr("autocomplete"), Some("one-time-code"));
        assert_eq!(code.value().attr("inputmode"), Some("numeric"));
        assert_eq!(code.value().attr("pattern"), Some("[0-9]{6}"));
        assert_eq!(code.value().attr("maxlength"), Some("6"));
        assert_eq!(
            form.select(&selector("input[name='ticket']"))
                .next()
                .and_then(|input| input.value().attr("value")),
            Some("private-ticket")
        );
    }

    fn assert_viewer_shell(document: &Html) {
        assert_eq!(document.select(&selector("body.viewer-page")).count(), 1);
        assert_eq!(document.select(&selector("article.viewer")).count(), 1);
        assert_eq!(
            document
                .select(&selector(".minimap-frame .minimap"))
                .count(),
            1
        );
        assert_eq!(document.select(&selector(".thread .call-block")).count(), 1);
        assert_eq!(
            document
                .select(&selector("link[rel=stylesheet]"))
                .next()
                .and_then(|node| node.value().attr("href")),
            Some("/style.css?v=20260818-departure-mono")
        );
        assert_eq!(
            document
                .select(&selector("script[src]"))
                .next()
                .and_then(|node| node.value().attr("src")),
            Some("/viewer.js?v=20260818-departure-mono")
        );
    }

    fn assert_viewer_controls(document: &Html) {
        let role_filters = document
            .select(&selector(".role-filters"))
            .next()
            .expect("message filter group");
        assert_eq!(role_filters.value().attr("role"), Some("group"));
        assert_eq!(
            role_filters.value().attr("aria-label"),
            Some("Message filters")
        );
        let thread_heading = document
            .select(&selector("#thread-messages-heading"))
            .next()
            .expect("thread messages heading");
        assert_eq!(thread_heading.value().name(), "h2");
        let thread = document
            .select(&selector("#thread-messages"))
            .next()
            .expect("thread messages region");
        assert_eq!(thread.value().attr("role"), Some("region"));
        assert_eq!(
            thread.value().attr("aria-labelledby"),
            Some("thread-messages-heading")
        );
        for id in ["filter-user", "filter-agent", "filter-tools"] {
            assert_eq!(document.select(&selector(&format!("#{id}"))).count(), 1);
        }
        for class in ["user", "assistant", "tool"] {
            assert_eq!(
                document
                    .select(&selector(&format!(".filter-toggle.{class}")))
                    .count(),
                1
            );
        }
        assert_eq!(
            document
                .select(&selector(".view-icon.rendered-icon"))
                .count(),
            1
        );
        assert_eq!(
            document.select(&selector(".view-icon.text-icon")).count(),
            1
        );
    }

    fn assert_viewer_message(document: &Html) {
        let message = document
            .select(&selector(".message.assistant"))
            .next()
            .expect("assistant message");
        let ordinal = message
            .select(&selector("a.ordinal"))
            .next()
            .expect("ordinal");
        assert_eq!(ordinal.value().attr("href"), Some("#message-1"));
        assert_eq!(
            ordinal.value().attr("aria-label"),
            Some("001, link to message 1")
        );
        assert_eq!(ordinal.text().collect::<String>(), "001");
        assert_eq!(
            message
                .select(&selector(".role"))
                .next()
                .expect("role")
                .text()
                .collect::<String>(),
            "AGENT"
        );
        assert_eq!(
            message
                .select(&selector("strong"))
                .next()
                .expect("strong")
                .text()
                .collect::<String>(),
            "Done."
        );
    }

    fn assert_viewer_assets() {
        let theme = include_str!("theme.css");
        for contract in [
            "src: url(\"/fonts/departure-mono-1.500.woff2\") format(\"woff2\");",
            "font: 13px/1.5 \"Departure Mono\", ui-monospace,",
            "grid-template-columns: 44px 72px minmax(0, 1fr)",
            "padding: 7px 0 8px 0;",
            "--document-width: calc(140px + 80ch);",
            "max-width: 80ch;",
            ".minimap-frame",
            "color-scheme: dark",
            "[hidden] { display: none !important; }",
            "z-index: 20;",
            "width: 24px;",
            "width: 6px;",
            "touch-action: none;",
            ".message:hover, .message:focus-within",
            ".viewer:has(.thread-view-toggle:checked)",
            ".authorization-input",
            ".visually-hidden",
        ] {
            assert!(theme.contains(contract));
        }
        assert!(!theme.contains(".message.user"));
        let message_rule = theme
            .split(".message {")
            .nth(1)
            .and_then(|rules| rules.split('}').next())
            .expect("message CSS rule");
        assert!(!message_rule.contains("border"));
        for contract in [
            "setPointerCapture",
            "pointermove",
            "pointerup",
            "pointercancel",
            "keydown",
            "PageDown",
            "aria-valuenow",
            "viewer.dataset.threadScroll",
            "scrollTarget.addEventListener(\"scroll\"",
            "addEventListener(\"load\", layout",
        ] {
            assert!(VIEWER_JS.contains(contract));
        }
    }

    #[tokio::test]
    async fn rendered_view_uses_flat_teletype_rows() {
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
        let html = viewer_page(&record, false)
            .await
            .expect("viewer page")
            .render(&Cx::default());
        let document = Html::parse_document(&html);
        assert_viewer_shell(&document);
        assert_viewer_controls(&document);
        assert_viewer_message(&document);
        assert!(!html.contains(">Rendered<"));
        assert!(!html.contains(">Text<"));
        assert!(!html.contains("shadow-sm"));
        assert!(!html.contains("bg-paper px-4 py-3"));
        assert_viewer_assets();
    }
}

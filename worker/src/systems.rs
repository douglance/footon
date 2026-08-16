#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use worker::Method;
use worker::d1::D1Type;
use worker::{Env, Request, Response, Result};

use crate::{AuthUser, CredentialKind};

const SERVICE_SCOPES: [&str; 4] = ["logs:read", "logs:write", "shares:read", "shares:write"];
const MAX_ACTIVE_KEYS: i64 = 20;
const MAX_REPORTS_PER_HOUR: i64 = 1_000;
const MAX_REPORT_SUMMARY_CHARS: usize = 2_000;
const DEFAULT_KEY_TTL_DAYS: i64 = 90;
const MAX_KEY_TTL_DAYS: i64 = 365;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteLimit {
    ActiveKeys,
    Reports,
}

impl WriteLimit {
    const fn http_status(self) -> u16 {
        match self {
            Self::ActiveKeys => 409,
            Self::Reports => 429,
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::ActiveKeys => "active service key limit reached",
            Self::Reports => "remote report rate limit reached",
        }
    }
}

enum WriteOutcome<T> {
    Stored(T),
    Limit(WriteLimit),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateKeyInput {
    name: String,
    system: String,
    #[serde(default = "default_service_scope")]
    scope: String,
    #[serde(default = "default_key_ttl_days")]
    expires_in_days: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServiceKey {
    id: String,
    name: String,
    system: String,
    token_prefix: String,
    scope: String,
    created_at: String,
    expires_at: String,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IssuedServiceKey {
    #[serde(flatten)]
    metadata: ServiceKey,
    key: String,
}

#[derive(Debug, Deserialize)]
struct ServiceKeyRow {
    id: String,
    name: String,
    system: String,
    token_prefix: String,
    scope: String,
    created_at: String,
    expires_at: String,
    last_used_at: Option<String>,
    revoked_at: Option<String>,
}

impl From<ServiceKeyRow> for ServiceKey {
    fn from(row: ServiceKeyRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            system: row.system,
            token_prefix: row.token_prefix,
            scope: row.scope,
            created_at: row.created_at,
            expires_at: row.expires_at,
            last_used_at: row.last_used_at,
            revoked_at: row.revoked_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ServiceAuthRow {
    id: String,
    owner_id: String,
    owner_email: String,
    system: String,
    scope: String,
    expires_at: String,
    revoked_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LogReportInput {
    environment: String,
    level: LogLevel,
    event: String,
    summary: String,
    source_event_id: String,
    occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl LogLevel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogReport {
    id: String,
    key_id: String,
    system: String,
    environment: String,
    level: LogLevel,
    event: String,
    summary: String,
    redactions: i64,
    source_event_id: String,
    occurred_at: String,
    received_at: String,
}

#[derive(Debug, Deserialize)]
struct LogReportRow {
    id: String,
    key_id: String,
    system: String,
    environment: String,
    level: String,
    event: String,
    summary: String,
    redactions: i64,
    source_event_id: String,
    occurred_at: String,
    received_at: String,
}

impl TryFrom<LogReportRow> for LogReport {
    type Error = worker::Error;

    fn try_from(row: LogReportRow) -> std::result::Result<Self, Self::Error> {
        let level = match row.level.as_str() {
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "critical" => LogLevel::Critical,
            _ => {
                return Err(worker::Error::RustError(
                    "invalid stored report level".to_string(),
                ));
            }
        };
        Ok(Self {
            id: row.id,
            key_id: row.key_id,
            system: row.system,
            environment: row.environment,
            level,
            event: row.event,
            summary: row.summary,
            redactions: row.redactions,
            source_event_id: row.source_event_id,
            occurred_at: row.occurred_at,
            received_at: row.received_at,
        })
    }
}

pub(crate) async fn api_create_key(req: &mut Request, env: &Env) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !user.is_interactive() || !crate::has_scope(&user, "keys:manage") {
        return Response::error("missing scope: keys:manage", 403);
    }
    let input = req.json::<CreateKeyInput>().await?;
    let input = match validate_key_input(input) {
        Ok(input) => input,
        Err(message) => return Response::error(message, 400),
    };
    if !crate::user_is_pro(env, &user.user_id).await? {
        return Response::error("service keys require Pro", 402);
    }
    match create_key(env, &user, input).await? {
        WriteOutcome::Stored(issued) => Ok(crate::json_response(&issued)?.with_status(201)),
        WriteOutcome::Limit(limit) => Response::error(limit.message(), limit.http_status()),
    }
}

pub(crate) async fn api_route(
    req: &mut Request,
    env: &Env,
    method: &Method,
    path: &str,
) -> Result<Option<Response>> {
    let response = if *method == Method::Post && path == "/api/keys" {
        api_create_key(req, env).await?
    } else if *method == Method::Get && path == "/api/keys" {
        api_list_keys(req, env).await?
    } else if *method == Method::Delete {
        let Some(id) = service_key_id(path) else {
            return Ok(None);
        };
        api_revoke_key(req, env, id).await?
    } else if *method == Method::Post && path == "/api/log-reports" {
        api_create_report(req, env).await?
    } else if *method == Method::Get && path == "/api/log-reports" {
        api_list_reports(req, env).await?
    } else {
        return Ok(None);
    };
    Ok(Some(response))
}

pub(crate) async fn api_list_keys(req: &Request, env: &Env) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !user.is_interactive() || !crate::has_scope(&user, "keys:manage") {
        return Response::error("missing scope: keys:manage", 403);
    }
    crate::json_response(&list_keys(env, &user).await?)
}

pub(crate) async fn api_revoke_key(req: &Request, env: &Env, id: &str) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !user.is_interactive() || !crate::has_scope(&user, "keys:manage") {
        return Response::error("missing scope: keys:manage", 403);
    }
    if !valid_identifier(id, 64) {
        return Response::error("invalid service key id", 400);
    }
    let Some(key) = revoke_key(env, &user, id).await? else {
        return Response::error("not found", 404);
    };
    crate::json_response(&key)
}

pub(crate) async fn api_create_report(req: &mut Request, env: &Env) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "logs:write") {
        return Response::error("missing scope: logs:write", 403);
    }
    if !matches!(user.credential, CredentialKind::Service { .. }) {
        return Response::error("a service key is required", 403);
    }
    let input = req.json::<LogReportInput>().await?;
    let input = match validate_report_input(input) {
        Ok(input) => input,
        Err(message) => return Response::error(message, 400),
    };
    match create_report(env, &user, input).await? {
        WriteOutcome::Stored(report) => Ok(crate::json_response(&report)?.with_status(201)),
        WriteOutcome::Limit(limit) => Response::error(limit.message(), limit.http_status()),
    }
}

pub(crate) async fn api_list_reports(req: &Request, env: &Env) -> Result<Response> {
    let Some(user) = crate::bearer_user(req, env).await? else {
        return Response::error("invalid bearer token", 401);
    };
    if !crate::has_scope(&user, "logs:read") {
        return Response::error("missing scope: logs:read", 403);
    }
    let url = req.url()?;
    let system = crate::query(&url, "system");
    let system = match system {
        Some(value) => match normalize_system(&value) {
            Some(value) => Some(value),
            None => return Response::error("invalid system", 400),
        },
        None => None,
    };
    let limit = crate::query(&url, "limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Response::error("limit must be between 1 and 200", 400);
    }
    crate::json_response(&list_reports(env, &user, system.as_deref(), limit).await?)
}

pub(crate) async fn authenticate_service_key(
    env: &Env,
    token_hash: &str,
) -> Result<Option<AuthUser>> {
    let db = env.d1("DB")?;
    let Some(row) = db
        .prepare(
            "SELECT id, owner_id, owner_email, system, scope, expires_at, revoked_at
             FROM service_keys WHERE token_hash = ?1",
        )
        .bind_refs(&[D1Type::Text(token_hash)])?
        .first::<ServiceAuthRow>(None)
        .await?
    else {
        return Ok(None);
    };
    let revoked = row.revoked_at.is_some();
    let expired = crate::parse_time(&row.expires_at).timestamp() < crate::unix_now();
    if revoked || expired {
        return Ok(None);
    }
    let owner_is_pro = crate::user_is_pro(env, &row.owner_id).await?;
    if !service_key_is_usable(revoked, expired, owner_is_pro) {
        return Ok(None);
    }
    db.prepare(
        "UPDATE service_keys SET last_used_at = ?1
         WHERE id = ?2 AND revoked_at IS NULL AND expires_at >= ?1",
    )
    .bind_refs(&[D1Type::Text(&crate::now_string()), D1Type::Text(&row.id)])?
    .run()
    .await?;
    Ok(Some(AuthUser {
        user_id: row.owner_id,
        email: row.owner_email,
        scope: row.scope,
        credential: CredentialKind::Service {
            key_id: row.id,
            system: row.system,
        },
    }))
}

const fn service_key_is_usable(revoked: bool, expired: bool, owner_is_pro: bool) -> bool {
    !revoked && !expired && owner_is_pro
}

pub(crate) async fn tool_create_key(
    env: &Env,
    user: &AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    if !user.is_interactive() {
        return Err(worker::Error::RustError(
            "an interactive session is required".to_string(),
        ));
    }
    if !crate::user_is_pro(env, &user.user_id).await? {
        return Err(worker::Error::RustError(
            "service keys require Pro".to_string(),
        ));
    }
    let input = serde_json::from_value::<CreateKeyInput>(args)?;
    let input = validate_key_input(input).map_err(worker::Error::RustError)?;
    let issued = match create_key(env, user, input).await? {
        WriteOutcome::Stored(issued) => issued,
        WriteOutcome::Limit(limit) => {
            return Err(worker::Error::RustError(limit.message().to_string()));
        }
    };
    serde_json::to_value(issued).map_err(Into::into)
}

pub(crate) async fn tool_list_keys(env: &Env, user: &AuthUser) -> Result<serde_json::Value> {
    if !user.is_interactive() {
        return Err(worker::Error::RustError(
            "an interactive session is required".to_string(),
        ));
    }
    serde_json::to_value(list_keys(env, user).await?).map_err(Into::into)
}

pub(crate) async fn tool_revoke_key(
    env: &Env,
    user: &AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        id: String,
    }
    if !user.is_interactive() {
        return Err(worker::Error::RustError(
            "an interactive session is required".to_string(),
        ));
    }
    let input = serde_json::from_value::<Input>(args)?;
    if !valid_identifier(&input.id, 64) {
        return Err(worker::Error::RustError(
            "invalid service key id".to_string(),
        ));
    }
    let key = revoke_key(env, user, &input.id)
        .await?
        .ok_or_else(|| worker::Error::RustError("service key not found".to_string()))?;
    serde_json::to_value(key).map_err(Into::into)
}

pub(crate) async fn tool_create_report(
    env: &Env,
    user: &AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    if !matches!(user.credential, CredentialKind::Service { .. }) {
        return Err(worker::Error::RustError(
            "a service key is required".to_string(),
        ));
    }
    let input = serde_json::from_value::<LogReportInput>(args)?;
    let input = validate_report_input(input).map_err(worker::Error::RustError)?;
    let report = match create_report(env, user, input).await? {
        WriteOutcome::Stored(report) => report,
        WriteOutcome::Limit(limit) => {
            return Err(worker::Error::RustError(limit.message().to_string()));
        }
    };
    serde_json::to_value(report).map_err(Into::into)
}

pub(crate) async fn tool_list_reports(
    env: &Env,
    user: &AuthUser,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Input {
        system: Option<String>,
        #[serde(default = "default_report_limit")]
        limit: i64,
    }
    let input = serde_json::from_value::<Input>(args)?;
    if !(1..=200).contains(&input.limit) {
        return Err(worker::Error::RustError(
            "limit must be between 1 and 200".to_string(),
        ));
    }
    let system = match input.system {
        Some(value) => Some(
            normalize_system(&value)
                .ok_or_else(|| worker::Error::RustError("invalid system".to_string()))?,
        ),
        None => None,
    };
    serde_json::to_value(list_reports(env, user, system.as_deref(), input.limit).await?)
        .map_err(Into::into)
}

async fn create_key(
    env: &Env,
    user: &AuthUser,
    input: CreateKeyInput,
) -> Result<WriteOutcome<IssuedServiceKey>> {
    let db = env.d1("DB")?;
    let active = db
        .prepare(
            "SELECT COUNT(*) AS count FROM service_keys
             WHERE owner_id = ?1 AND revoked_at IS NULL AND expires_at >= ?2",
        )
        .bind_refs(&[
            D1Type::Text(&user.user_id),
            D1Type::Text(&crate::now_string()),
        ])?
        .first::<CountRow>(None)
        .await?
        .map_or(0, |row| row.count);
    if active >= MAX_ACTIVE_KEYS {
        return Ok(WriteOutcome::Limit(WriteLimit::ActiveKeys));
    }
    let id = crate::token(18);
    let secret = format!("ftn_sk_{}", crate::token(32));
    let token_prefix = secret.chars().take(15).collect::<String>();
    let now = crate::now_string();
    let expires_at =
        crate::time_string(crate::unix_now() + input.expires_in_days.saturating_mul(86_400));
    db.prepare(
        "INSERT INTO service_keys
         (id, owner_id, owner_email, name, system, token_hash, token_prefix, scope,
          created_at, expires_at, last_used_at, revoked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL)",
    )
    .bind_refs(&[
        D1Type::Text(&id),
        D1Type::Text(&user.user_id),
        D1Type::Text(&user.email),
        D1Type::Text(&input.name),
        D1Type::Text(&input.system),
        D1Type::Text(&crate::hash(&secret)),
        D1Type::Text(&token_prefix),
        D1Type::Text(&input.scope),
        D1Type::Text(&now),
        D1Type::Text(&expires_at),
    ])?
    .run()
    .await?;
    Ok(WriteOutcome::Stored(IssuedServiceKey {
        metadata: ServiceKey {
            id,
            name: input.name,
            system: input.system,
            token_prefix,
            scope: input.scope,
            created_at: now,
            expires_at,
            last_used_at: None,
            revoked_at: None,
        },
        key: secret,
    }))
}

async fn list_keys(env: &Env, user: &AuthUser) -> Result<Vec<ServiceKey>> {
    env.d1("DB")?
        .prepare(
            "SELECT id, name, system, token_prefix, scope, created_at, expires_at,
                    last_used_at, revoked_at
             FROM service_keys WHERE owner_id = ?1
             ORDER BY created_at DESC LIMIT 100",
        )
        .bind_refs(&[D1Type::Text(&user.user_id)])?
        .all()
        .await?
        .results::<ServiceKeyRow>()
        .map(|rows| rows.into_iter().map(Into::into).collect())
}

async fn revoke_key(env: &Env, user: &AuthUser, id: &str) -> Result<Option<ServiceKey>> {
    let db = env.d1("DB")?;
    let result = db
        .prepare(
            "UPDATE service_keys SET revoked_at = ?1
             WHERE id = ?2 AND owner_id = ?3 AND revoked_at IS NULL",
        )
        .bind_refs(&[
            D1Type::Text(&crate::now_string()),
            D1Type::Text(id),
            D1Type::Text(&user.user_id),
        ])?
        .run()
        .await?;
    if result.meta()?.and_then(|meta| meta.changes) == Some(0) {
        return Ok(None);
    }
    db.prepare(
        "SELECT id, name, system, token_prefix, scope, created_at, expires_at,
                last_used_at, revoked_at
         FROM service_keys WHERE id = ?1 AND owner_id = ?2",
    )
    .bind_refs(&[D1Type::Text(id), D1Type::Text(&user.user_id)])?
    .first::<ServiceKeyRow>(None)
    .await
    .map(|row| row.map(Into::into))
}

async fn create_report(
    env: &Env,
    user: &AuthUser,
    input: LogReportInput,
) -> Result<WriteOutcome<LogReport>> {
    let CredentialKind::Service { key_id, system } = &user.credential else {
        return Err(worker::Error::RustError(
            "a service key is required".to_string(),
        ));
    };
    let db = env.d1("DB")?;
    if let Some(report) = load_report_by_source(env, key_id, &input.source_event_id).await? {
        return Ok(WriteOutcome::Stored(report));
    }
    let hour_cutoff = crate::time_string(crate::unix_now() - 3_600);
    let count = db
        .prepare(
            "SELECT COUNT(*) AS count FROM remote_log_reports
             WHERE key_id = ?1 AND received_at >= ?2",
        )
        .bind_refs(&[D1Type::Text(key_id), D1Type::Text(&hour_cutoff)])?
        .first::<CountRow>(None)
        .await?
        .map_or(0, |row| row.count);
    if count >= MAX_REPORTS_PER_HOUR {
        return Ok(WriteOutcome::Limit(WriteLimit::Reports));
    }
    let (summary, redactions) = sanitize_report_summary(&input.summary)?;
    let id = crate::token(18);
    let occurred_at = input.occurred_at.to_rfc3339();
    let received_at = crate::now_string();
    db.prepare(
        "INSERT INTO remote_log_reports
         (id, owner_id, key_id, system, environment, level, event, summary,
          redactions, source_event_id, occurred_at, received_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(key_id, source_event_id) DO NOTHING",
    )
    .bind_refs(&[
        D1Type::Text(&id),
        D1Type::Text(&user.user_id),
        D1Type::Text(key_id),
        D1Type::Text(system),
        D1Type::Text(&input.environment),
        D1Type::Text(input.level.as_str()),
        D1Type::Text(&input.event),
        D1Type::Text(&summary),
        D1Type::Integer(i32::try_from(redactions).unwrap_or(i32::MAX)),
        D1Type::Text(&input.source_event_id),
        D1Type::Text(&occurred_at),
        D1Type::Text(&received_at),
    ])?
    .run()
    .await?;
    let report = load_report_by_source(env, key_id, &input.source_event_id)
        .await?
        .ok_or_else(|| worker::Error::RustError("remote report was not stored".to_string()))?;
    Ok(WriteOutcome::Stored(report))
}

async fn load_report_by_source(
    env: &Env,
    key_id: &str,
    source_event_id: &str,
) -> Result<Option<LogReport>> {
    env.d1("DB")?
        .prepare(
            "SELECT id, key_id, system, environment, level, event, summary, redactions,
                    source_event_id, occurred_at, received_at
             FROM remote_log_reports WHERE key_id = ?1 AND source_event_id = ?2",
        )
        .bind_refs(&[D1Type::Text(key_id), D1Type::Text(source_event_id)])?
        .first::<LogReportRow>(None)
        .await?
        .map(TryInto::try_into)
        .transpose()
}

async fn list_reports(
    env: &Env,
    user: &AuthUser,
    requested_system: Option<&str>,
    limit: i64,
) -> Result<Vec<LogReport>> {
    let db = env.d1("DB")?;
    let rows = match &user.credential {
        CredentialKind::Interactive => {
            if let Some(system) = requested_system {
                db.prepare(
                    "SELECT id, key_id, system, environment, level, event, summary, redactions,
                            source_event_id, occurred_at, received_at
                     FROM remote_log_reports WHERE owner_id = ?1 AND system = ?2
                     ORDER BY received_at DESC LIMIT ?3",
                )
                .bind_refs(&[
                    D1Type::Text(&user.user_id),
                    D1Type::Text(system),
                    D1Type::Integer(i32::try_from(limit).unwrap_or(200)),
                ])?
                .all()
                .await?
                .results::<LogReportRow>()?
            } else {
                db.prepare(
                    "SELECT id, key_id, system, environment, level, event, summary, redactions,
                            source_event_id, occurred_at, received_at
                     FROM remote_log_reports WHERE owner_id = ?1
                     ORDER BY received_at DESC LIMIT ?2",
                )
                .bind_refs(&[
                    D1Type::Text(&user.user_id),
                    D1Type::Integer(i32::try_from(limit).unwrap_or(200)),
                ])?
                .all()
                .await?
                .results::<LogReportRow>()?
            }
        }
        CredentialKind::Service { key_id, system } => {
            if requested_system.is_some_and(|requested| requested != system) {
                return Ok(Vec::new());
            }
            db.prepare(
                "SELECT id, key_id, system, environment, level, event, summary, redactions,
                        source_event_id, occurred_at, received_at
                 FROM remote_log_reports WHERE key_id = ?1
                 ORDER BY received_at DESC LIMIT ?2",
            )
            .bind_refs(&[
                D1Type::Text(key_id),
                D1Type::Integer(i32::try_from(limit).unwrap_or(200)),
            ])?
            .all()
            .await?
            .results::<LogReportRow>()?
        }
    };
    rows.into_iter().map(TryInto::try_into).collect()
}

fn validate_key_input(mut input: CreateKeyInput) -> std::result::Result<CreateKeyInput, String> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty()
        || input.name.chars().count() > 80
        || input.name.chars().any(char::is_control)
    {
        return Err("name must contain 1 to 80 printable characters".to_string());
    }
    input.system = normalize_system(&input.system)
        .ok_or_else(|| "system must be a 1 to 64 character identifier".to_string())?;
    input.scope = canonical_service_scope(&input.scope).map_err(str::to_string)?;
    if !(1..=MAX_KEY_TTL_DAYS).contains(&input.expires_in_days) {
        return Err("expiresInDays must be between 1 and 365".to_string());
    }
    Ok(input)
}

fn validate_report_input(mut input: LogReportInput) -> std::result::Result<LogReportInput, String> {
    input.environment = normalize_system(&input.environment)
        .ok_or_else(|| "environment must be a 1 to 64 character identifier".to_string())?;
    input.event = input.event.trim().to_string();
    input.source_event_id = input.source_event_id.trim().to_string();
    input.summary = input.summary.trim().to_string();
    if !valid_identifier(&input.event, 100) {
        return Err("event must be a 1 to 100 character identifier".to_string());
    }
    if !valid_identifier(&input.source_event_id, 160) {
        return Err("sourceEventId must be a 1 to 160 character identifier".to_string());
    }
    if input.summary.is_empty() || input.summary.chars().count() > MAX_REPORT_SUMMARY_CHARS {
        return Err("summary must contain 1 to 2000 characters".to_string());
    }
    if input.occurred_at.timestamp() > crate::unix_now() + 300 {
        return Err("occurredAt cannot be more than 5 minutes in the future".to_string());
    }
    Ok(input)
}

fn canonical_service_scope(scope: &str) -> std::result::Result<String, &'static str> {
    let mut scopes = scope.split_whitespace().collect::<Vec<_>>();
    scopes.sort_unstable();
    scopes.dedup();
    if scopes.is_empty() || !scopes.iter().all(|scope| SERVICE_SCOPES.contains(scope)) {
        return Err("unsupported service key scope");
    }
    Ok(scopes.join(" "))
}

fn normalize_system(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    valid_identifier(&value, 64).then_some(value)
}

fn service_key_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix("/api/keys/")?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn sanitize_report_summary(value: &str) -> worker::Result<(String, usize)> {
    footon_core::safety::sanitize_text(value).map_err(|error| {
        worker::Error::RustError(format!("could not sanitize remote report: {error}"))
    })
}

fn default_service_scope() -> String {
    "logs:write".to_string()
}

const fn default_key_ttl_days() -> i64 {
    DEFAULT_KEY_TTL_DAYS
}

const fn default_report_limit() -> i64 {
    50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_key_scopes_are_bounded_canonical_and_cannot_issue_keys() {
        assert_eq!(
            canonical_service_scope("logs:write logs:read logs:write").expect("valid scopes"),
            "logs:read logs:write"
        );
        assert!(canonical_service_scope("keys:manage").is_err());
        assert!(canonical_service_scope("").is_err());
    }

    #[test]
    fn service_keys_pause_when_the_owner_no_longer_has_pro() {
        assert!(service_key_is_usable(false, false, true));
        assert!(!service_key_is_usable(false, false, false));
        assert!(!service_key_is_usable(true, false, true));
        assert!(!service_key_is_usable(false, true, true));
    }

    #[test]
    fn system_identifiers_are_provider_neutral_and_url_safe() {
        assert_eq!(normalize_system(" Auth0-EU ").as_deref(), Some("auth0-eu"));
        assert_eq!(
            normalize_system("custom.auth:v2").as_deref(),
            Some("custom.auth:v2")
        );
        assert!(normalize_system("auth system").is_none());
        assert!(normalize_system("../auth0").is_none());
        assert!(normalize_system(&"a".repeat(65)).is_none());
    }

    #[test]
    fn remote_report_summaries_redact_credentials_before_storage() {
        let (summary, redactions) = sanitize_report_summary(
            "Auth0 failed with Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456",
        )
        .expect("summary");
        assert_eq!(redactions, 1);
        assert!(summary.contains("[REDACTED:BEARER:"));
        assert!(!summary.contains("abcdefghijklmnopqrstuvwxyz123456"));
    }

    #[test]
    fn migration_stores_only_key_hashes_and_idempotent_report_ids() {
        let migration = include_str!("../../migrations/0007_service_keys_and_log_reports.sql");
        assert!(migration.contains("token_hash TEXT NOT NULL UNIQUE"));
        assert!(migration.contains("UNIQUE (key_id, source_event_id)"));
        assert!(migration.contains("FOREIGN KEY (key_id) REFERENCES service_keys(id)"));
        assert!(!migration.contains("token TEXT"));
        assert!(!migration.contains("secret TEXT"));
    }

    #[test]
    fn write_limits_map_to_specific_http_responses() {
        assert_eq!(WriteLimit::ActiveKeys.http_status(), 409);
        assert_eq!(
            WriteLimit::ActiveKeys.message(),
            "active service key limit reached"
        );
        assert_eq!(WriteLimit::Reports.http_status(), 429);
        assert_eq!(
            WriteLimit::Reports.message(),
            "remote report rate limit reached"
        );
    }
}

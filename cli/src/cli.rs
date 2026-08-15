use std::path::Path;

use incurs::cli::Cli;
use incurs::command::{CommandDef, McpAnnotations, McpCommandOptions, TypedContext, TypedResult};
use incurs::output::Format;
use serde::Deserialize;

use crate::blackout::{self, LocalBlackoutOutput, ShareBlackoutResponse};
use crate::draft::{self, DraftOutput};
use crate::error::Error;
use crate::fetch;
use crate::parse::Source;
use crate::publish::{self, GeneralAccess, PublishResponse};
use crate::session::{self, KeyringStore, SignoutResponse};
use crate::sharing::{
    self, ShareAccess, ShareMember, ShareMetadata, ShareRole, ShareSummary, TransferResponse,
};
use crate::signin::{self, SigninResponse};
use crate::systems::{self, IssuedServiceKey, LogLevel, LogReport, ReportSubmission, ServiceKey};

#[derive(Deserialize, incurs::Args)]
struct DraftArgs {
    /// Claude Code or Codex JSONL file. Raw content never leaves this machine.
    input: String,
}

#[derive(Deserialize, incurs::Options)]
struct DraftOptions {
    /// Public title for the sanitized conversation.
    title: String,
    /// Sanitized draft destination.
    #[incurs(default = "footon-draft.json")]
    output: String,
    /// Input shape: auto, claude, or codex.
    #[incurs(default = "auto")]
    source: String,
}

#[derive(Deserialize, incurs::Args)]
struct PublishArgs {
    /// Previously generated sanitized draft JSON.
    draft: String,
}

#[derive(Deserialize, incurs::Args)]
struct FetchArgs {
    /// HTTPS Footon share URL. Loopback HTTP is accepted for tests.
    url: String,
}

#[derive(Deserialize, incurs::Args)]
struct SigninArgs {
    /// Email address that receives the one-time Footon code.
    email: Option<String>,
}

#[derive(Deserialize, incurs::Options)]
struct SigninOptions {
    /// Footon OAuth origin. Loopback HTTP is accepted for tests.
    #[incurs(default = "https://footon.dev")]
    origin: String,
}

#[derive(Deserialize, incurs::Options)]
struct SignoutOptions {
    /// Footon OAuth origin. Loopback HTTP is accepted for tests.
    #[incurs(default = "https://footon.dev")]
    origin: String,
}

#[derive(Deserialize, incurs::Args)]
struct BlackoutArgs {
    /// Sanitized local draft JSON to update in place.
    draft: String,
    /// One-based transcript message number.
    message: usize,
    /// Exact substring that must occur once in the selected message.
    text: String,
}

#[derive(Deserialize, incurs::Args)]
struct BlackoutShareArgs {
    /// Footon share ID or full Footon share URL.
    share: String,
    /// One-based transcript message number.
    message: usize,
    /// Exact substring that must occur once in the selected message.
    text: String,
}

#[derive(Deserialize, incurs::Options)]
struct BlackoutShareOptions {
    /// Protected Footon share collection endpoint.
    #[incurs(default = "https://footon.dev/api/shares")]
    endpoint: String,
}

#[derive(Deserialize, incurs::Options)]
struct PublishOptions {
    /// Protected Footon share endpoint.
    #[incurs(default = "https://footon.dev/api/shares")]
    endpoint: String,
    /// Require authenticated private access. Public is free and remains the default.
    private: bool,
}

#[derive(Deserialize, incurs::Options)]
struct ShareEndpointOptions {
    /// Protected Footon share collection endpoint.
    #[incurs(default = "https://footon.dev/api/shares")]
    endpoint: String,
}

#[derive(Deserialize, incurs::Args)]
struct ShareArgs {
    /// Footon share ID or full Footon share URL.
    share: String,
}

#[derive(Deserialize, incurs::Args)]
struct ShareRenameArgs {
    /// Footon share ID or full Footon share URL.
    share: String,
    /// New public title.
    title: String,
}

#[derive(Deserialize, incurs::Args)]
struct ShareVisibilityArgs {
    /// Footon share ID or full Footon share URL.
    share: String,
    /// Visibility: public or private.
    visibility: String,
}

#[derive(Deserialize, incurs::Args)]
struct ShareGrantArgs {
    /// Footon share ID or full Footon share URL.
    share: String,
    /// Normalized member email.
    email: String,
    /// Private role: viewer or editor.
    role: String,
}

#[derive(Deserialize, incurs::Args)]
struct ShareEmailArgs {
    /// Footon share ID or full Footon share URL.
    share: String,
    /// Member or new-owner email.
    email: String,
}

#[derive(Deserialize, incurs::Args)]
struct KeyCreateArgs {
    /// Human-readable key name.
    name: String,
    /// Provider-neutral system identifier, such as auth0-prod or cloudflare-access.
    system: String,
}

#[derive(Deserialize, incurs::Options)]
struct KeyCreateOptions {
    /// Footon service-key collection endpoint.
    #[incurs(default = "https://footon.dev/api/keys")]
    endpoint: String,
    /// Space-separated service scopes.
    #[incurs(default = "logs:write")]
    scope: String,
    /// Credential lifetime from 1 to 365 days.
    #[incurs(default = 90)]
    expires_in_days: i64,
}

#[derive(Deserialize, incurs::Options)]
struct KeyEndpointOptions {
    /// Footon service-key collection endpoint.
    #[incurs(default = "https://footon.dev/api/keys")]
    endpoint: String,
}

#[derive(Deserialize, incurs::Args)]
struct KeyRevokeArgs {
    /// Service key ID returned by key-create or key-list.
    id: String,
}

#[derive(Deserialize, incurs::Args)]
struct ReportArgs {
    /// Stable event identifier, such as auth.login.failed.
    event: String,
    /// Bounded summary. Footon redacts recognized sensitive text before storage.
    summary: String,
    /// Provider event identifier used to make retries idempotent.
    source_event_id: String,
}

#[derive(Deserialize, incurs::Options)]
struct ReportOptions {
    /// Footon remote-report collection endpoint.
    #[incurs(default = "https://footon.dev/api/log-reports")]
    endpoint: String,
    /// Deployment environment identifier.
    #[incurs(default = "production")]
    environment: String,
    /// Report level: debug, info, warn, error, or critical.
    #[incurs(default = "error")]
    level: String,
    /// RFC 3339 source time. Defaults to the current UTC time.
    occurred_at: Option<String>,
}

#[derive(Deserialize, incurs::Options)]
struct ReportsOptions {
    /// Footon remote-report collection endpoint.
    #[incurs(default = "https://footon.dev/api/log-reports")]
    endpoint: String,
    /// Optional remote-system identifier filter.
    system: Option<String>,
    /// Maximum reports to return, from 1 to 200.
    #[incurs(default = 50)]
    limit: i64,
}

/// Build Footon's typed incurs command graph.
#[must_use]
pub fn app() -> Cli {
    Cli::create("footon")
        .version(env!("CARGO_PKG_VERSION"))
        .description("Sanitize agent threads locally, then publish only an approved safe draft")
        .command("signin", signin_command())
        .command("signout", signout_command())
        .command("draft", draft_command())
        .command("blackout", blackout_command())
        .command("blackout-share", blackout_share_command())
        .command("publish", publish_command())
        .command("fetch", fetch_command())
        .command("shares", shares_command())
        .command("share-access", share_access_command())
        .command("share-rename", share_rename_command())
        .command("share-visibility", share_visibility_command())
        .command("share-grant", share_grant_command())
        .command("share-remove", share_remove_command())
        .command("share-transfer", share_transfer_command())
        .command("key-create", key_create_command())
        .command("key-list", key_list_command())
        .command("key-revoke", key_revoke_command())
        .command("report", report_command())
        .command("reports", reports_command())
}

fn signin_command() -> CommandDef {
    CommandDef::typed::<SigninArgs, SigninOptions, (), SigninResponse, _, _>(
        "signin",
        |context| async move {
            let email = match context.args.email {
                Some(email) => email,
                None => match session::last_email(&context.options.origin, &KeyringStore) {
                    Ok(Some(email)) => email,
                    Ok(None) => {
                        let stdin = std::io::stdin();
                        let stderr = std::io::stderr();
                        match signin::read_email(&mut stdin.lock(), &mut stderr.lock()) {
                            Ok(email) => email,
                            Err(error) => return typed_error(&error),
                        }
                    }
                    Err(error) => return typed_error(&error),
                },
            };
            let pending = match signin::begin(&context.options.origin, &email).await {
                Ok(pending) => pending,
                Err(error) => return typed_error(&error),
            };
            let code = {
                let stdin = std::io::stdin();
                let stderr = std::io::stderr();
                match signin::read_code(&mut stdin.lock(), &mut stderr.lock(), &email) {
                    Ok(code) => code,
                    Err(error) => return typed_error(&error),
                }
            };
            match pending.complete(&code).await {
                Ok(completed) => match completed.save(&KeyringStore) {
                    Ok(output) => TypedResult::ok(output),
                    Err(error) => typed_error(&error),
                },
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Sign in from the terminal with an emailed six-digit code")
    .done()
}

fn signout_command() -> CommandDef {
    CommandDef::typed::<(), SignoutOptions, (), SignoutResponse, _, _>(
        "signout",
        |context| async move {
            match session::sign_out(&context.options.origin, &KeyringStore).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Revoke the current Footon session and remove it from secure storage")
    .mcp(mutation_options(true))
    .done()
}

fn blackout_command() -> CommandDef {
    CommandDef::typed::<BlackoutArgs, (), (), LocalBlackoutOutput, _, _>(
        "blackout",
        |context| async move {
            match blackout::local(
                Path::new(&context.args.draft),
                context.args.message,
                &context.args.text,
            ) {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Black out one exact substring in a sanitized local draft")
    .mcp(mutation_options(false))
    .done()
}

fn blackout_share_command() -> CommandDef {
    CommandDef::typed::<BlackoutShareArgs, BlackoutShareOptions, (), ShareBlackoutResponse, _, _>(
        "blackout-share",
        |context| async move {
            let environment_token = std::env::var("FOOTON_TOKEN").ok();
            let token = match session::resolve_access_token(
                &context.options.endpoint,
                environment_token.as_deref(),
                &KeyringStore,
            )
            .await
            {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match blackout::remote(
                &context.options.endpoint,
                &token,
                &context.args.share,
                context.args.message,
                &context.args.text,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Black out one exact substring in an owner-controlled live share")
    .mcp(mutation_options(true))
    .done()
}

fn mutation_options(open_world: bool) -> McpCommandOptions {
    McpCommandOptions {
        annotations: Some(McpAnnotations {
            read_only_hint: Some(false),
            destructive_hint: Some(true),
            idempotent_hint: Some(false),
            open_world_hint: Some(open_world),
            ..McpAnnotations::default()
        }),
        destructive: true,
        ..McpCommandOptions::default()
    }
}

fn draft_command() -> CommandDef {
    CommandDef::typed::<DraftArgs, DraftOptions, (), DraftOutput, _, _>(
        "draft",
        |context| async move {
            match create_draft(context) {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Create a sanitized local draft and safety report without network access")
    .done()
}

fn publish_command() -> CommandDef {
    CommandDef::typed::<PublishArgs, PublishOptions, (), PublishResponse, _, _>(
        "publish",
        |context| async move {
            match publish_draft(context).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Explicitly approve and publish one sanitized footon.share.v2 draft")
    .done()
}

fn fetch_command() -> CommandDef {
    CommandDef::typed::<FetchArgs, (), (), String, _, _>("fetch", |context| async move {
        match fetch_share(context).await {
            Ok(output) => TypedResult::ok(output),
            Err(error) => typed_error(&error),
        }
    })
    .description("Fetch one Footon share as Markdown")
    .format(Format::Markdown)
    .done()
}

fn shares_command() -> CommandDef {
    CommandDef::typed::<(), ShareEndpointOptions, (), Vec<ShareSummary>, _, _>(
        "shares",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::list(&context.options.endpoint, &token).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("List shares you own")
    .done()
}

fn share_access_command() -> CommandDef {
    CommandDef::typed::<ShareArgs, ShareEndpointOptions, (), ShareAccess, _, _>(
        "share-access",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::access(&context.options.endpoint, &token, &context.args.share).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Show a share owner, visibility, and private members")
    .done()
}

fn share_rename_command() -> CommandDef {
    CommandDef::typed::<ShareRenameArgs, ShareEndpointOptions, (), ShareMetadata, _, _>(
        "share-rename",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::rename(
                &context.options.endpoint,
                &token,
                &context.args.share,
                &context.args.title,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Rename an owned or editable share")
    .mcp(mutation_options(true))
    .done()
}

fn share_visibility_command() -> CommandDef {
    CommandDef::typed::<ShareVisibilityArgs, ShareEndpointOptions, (), ShareMetadata, _, _>(
        "share-visibility",
        |context| async move {
            let visibility = match context.args.visibility.to_ascii_lowercase().as_str() {
                "public" => GeneralAccess::Public,
                "private" => GeneralAccess::Private,
                _ => {
                    return typed_error(&Error::Access(
                        "visibility must be public or private".to_string(),
                    ));
                }
            };
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::visibility(
                &context.options.endpoint,
                &token,
                &context.args.share,
                visibility,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Make an owned share public or private")
    .mcp(mutation_options(true))
    .done()
}

fn share_grant_command() -> CommandDef {
    CommandDef::typed::<ShareGrantArgs, ShareEndpointOptions, (), ShareMember, _, _>(
        "share-grant",
        |context| async move {
            let role = match context.args.role.parse::<ShareRole>() {
                Ok(role) => role,
                Err(error) => return typed_error(&error),
            };
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::grant(
                &context.options.endpoint,
                &token,
                &context.args.share,
                &context.args.email,
                role,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Grant a Viewer or Editor role on a private share")
    .mcp(mutation_options(true))
    .done()
}

fn share_remove_command() -> CommandDef {
    CommandDef::typed::<ShareEmailArgs, ShareEndpointOptions, (), ShareMember, _, _>(
        "share-remove",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::remove(
                &context.options.endpoint,
                &token,
                &context.args.share,
                &context.args.email,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Remove a member from a private share")
    .mcp(mutation_options(true))
    .done()
}

fn share_transfer_command() -> CommandDef {
    CommandDef::typed::<ShareEmailArgs, ShareEndpointOptions, (), TransferResponse, _, _>(
        "share-transfer",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match sharing::transfer(
                &context.options.endpoint,
                &token,
                &context.args.share,
                &context.args.email,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Transfer ownership of one share")
    .mcp(mutation_options(true))
    .done()
}

fn key_create_command() -> CommandDef {
    CommandDef::typed::<KeyCreateArgs, KeyCreateOptions, (), IssuedServiceKey, _, _>(
        "key-create",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match systems::create_key(
                &context.options.endpoint,
                &token,
                &context.args.name,
                &context.args.system,
                &context.options.scope,
                context.options.expires_in_days,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description(
        "Issue a scoped Pro service key; save the returned secret because it is shown once",
    )
    .mcp(mutation_options(true))
    .done()
}

fn key_list_command() -> CommandDef {
    CommandDef::typed::<(), KeyEndpointOptions, (), Vec<ServiceKey>, _, _>(
        "key-list",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match systems::list_keys(&context.options.endpoint, &token).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("List service key metadata without exposing secrets")
    .done()
}

fn key_revoke_command() -> CommandDef {
    CommandDef::typed::<KeyRevokeArgs, KeyEndpointOptions, (), ServiceKey, _, _>(
        "key-revoke",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match systems::revoke_key(&context.options.endpoint, &token, &context.args.id).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Revoke one service key immediately")
    .mcp(mutation_options(true))
    .done()
}

fn report_command() -> CommandDef {
    CommandDef::typed::<ReportArgs, ReportOptions, (), LogReport, _, _>(
        "report",
        |context| async move {
            let service_key = match std::env::var("FOOTON_SERVICE_KEY") {
                Ok(value) if !value.trim().is_empty() => value,
                _ => {
                    return typed_error(&Error::Access(
                        "set FOOTON_SERVICE_KEY to a key with logs:write".to_string(),
                    ));
                }
            };
            let level = match context.options.level.parse::<LogLevel>() {
                Ok(level) => level,
                Err(error) => return typed_error(&error),
            };
            let occurred_at = match report_timestamp(context.options.occurred_at.as_deref()) {
                Ok(value) => value,
                Err(error) => return typed_error(&error),
            };
            let report = ReportSubmission {
                environment: context.options.environment,
                level,
                event: context.args.event,
                summary: context.args.summary,
                source_event_id: context.args.source_event_id,
                occurred_at,
            };
            match systems::create_report(&context.options.endpoint, service_key.trim(), &report)
                .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Submit one automatically redacted report using FOOTON_SERVICE_KEY")
    .mcp(mutation_options(true))
    .done()
}

fn report_timestamp(value: Option<&str>) -> crate::error::Result<chrono::DateTime<chrono::Utc>> {
    value.map_or_else(
        || Ok(chrono::Utc::now()),
        |value| {
            chrono::DateTime::parse_from_rfc3339(value)
                .map(|value| value.with_timezone(&chrono::Utc))
                .map_err(|error| Error::Access(format!("occurred-at must be RFC 3339: {error}")))
        },
    )
}

fn reports_command() -> CommandDef {
    CommandDef::typed::<(), ReportsOptions, (), Vec<LogReport>, _, _>(
        "reports",
        |context| async move {
            let token = match share_token(&context.options.endpoint).await {
                Ok(token) => token,
                Err(error) => return typed_error(&error),
            };
            match systems::list_reports(
                &context.options.endpoint,
                &token,
                context.options.system.as_deref(),
                context.options.limit,
            )
            .await
            {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("List recent remote-system reports")
    .done()
}

fn create_draft(
    context: TypedContext<DraftArgs, DraftOptions, ()>,
) -> crate::error::Result<DraftOutput> {
    let source = context.options.source.parse::<Source>()?;
    draft::create(
        Path::new(&context.args.input),
        Path::new(&context.options.output),
        context.options.title,
        source,
    )
}

async fn fetch_share(context: TypedContext<FetchArgs, (), ()>) -> crate::error::Result<String> {
    fetch_share_url(&context.args.url).await
}

/// Fetch public Markdown anonymously and retry a private share with the stored session.
///
/// # Errors
///
/// Returns an error for unsafe URLs, unavailable credentials, or rejected responses.
pub async fn fetch_share_url(url: &str) -> crate::error::Result<String> {
    match fetch::fetch_markdown(url).await {
        Ok(markdown) => Ok(markdown),
        Err(error) if fetch::authentication_required(&error) => {
            let environment_token = std::env::var("FOOTON_TOKEN").ok();
            let token =
                session::resolve_access_token(url, environment_token.as_deref(), &KeyringStore)
                    .await?;
            fetch::fetch_markdown_authenticated(url, &token).await
        }
        Err(error) => Err(error),
    }
}

async fn publish_draft(
    context: TypedContext<PublishArgs, PublishOptions, ()>,
) -> crate::error::Result<PublishResponse> {
    let draft = draft::read(Path::new(&context.args.draft))?;
    let share = publish::build_share(draft, chrono::Utc::now())?;
    let environment_token = std::env::var("FOOTON_TOKEN").ok();
    let token = session::resolve_access_token(
        &context.options.endpoint,
        environment_token.as_deref(),
        &KeyringStore,
    )
    .await?;
    publish::send_with_access(
        &context.options.endpoint,
        &token,
        &share,
        if context.options.private {
            GeneralAccess::Private
        } else {
            GeneralAccess::Public
        },
    )
    .await
}

async fn share_token(endpoint: &str) -> crate::error::Result<String> {
    let environment_token = std::env::var("FOOTON_TOKEN").ok();
    session::resolve_access_token(endpoint, environment_token.as_deref(), &KeyringStore).await
}

fn typed_error<T>(error: &Error) -> TypedResult<T> {
    TypedResult::error(error_code(error), error.to_string())
}

fn error_code(error: &Error) -> &'static str {
    match error {
        Error::Read { .. } | Error::Write { .. } => "LOCAL_IO_ERROR",
        Error::Json(_)
        | Error::NoMessages
        | Error::Source(_)
        | Error::Core(
            footon_core::Error::Json(_)
            | footon_core::Error::NoMessages
            | footon_core::Error::Source(_),
        ) => "INVALID_THREAD",
        Error::Safety(_) | Error::Core(footon_core::Error::Safety(_)) => "SAFETY_FAILED",
        Error::Share(_) | Error::Core(footon_core::Error::Share(_)) => "INVALID_DRAFT",
        Error::Endpoint(_) => "UNSAFE_ENDPOINT",
        Error::Publish(_) => "PUBLISH_FAILED",
        Error::Fetch(_) => "FETCH_FAILED",
        Error::Signin(_) => "SIGNIN_FAILED",
        Error::Session(_) => "SESSION_FAILED",
        Error::Access(_) => "ACCESS_FAILED",
    }
}

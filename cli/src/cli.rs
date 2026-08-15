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
use crate::publish::{self, PublishResponse};
use crate::session::{self, KeyringStore, SignoutResponse};
use crate::signin::{self, SigninResponse};

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
    email: String,
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
}

fn signin_command() -> CommandDef {
    CommandDef::typed::<SigninArgs, SigninOptions, (), SigninResponse, _, _>(
        "signin",
        |context| async move {
            let pending = match signin::begin(&context.options.origin, &context.args.email).await {
                Ok(pending) => pending,
                Err(error) => return typed_error(&error),
            };
            let code = {
                let stdin = std::io::stdin();
                let stderr = std::io::stderr();
                match signin::read_code(&mut stdin.lock(), &mut stderr.lock(), &context.args.email)
                {
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
    fetch::fetch_markdown(&context.args.url).await
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
    publish::send(&context.options.endpoint, &token, &share).await
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
    }
}

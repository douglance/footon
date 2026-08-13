use std::path::Path;

use incurs::cli::Cli;
use incurs::command::{CommandDef, TypedContext, TypedResult};
use serde::Deserialize;

use crate::draft::{self, DraftOutput};
use crate::error::Error;
use crate::parse::Source;
use crate::publish::{self, PublishResponse};

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

#[derive(Deserialize, incurs::Options)]
struct PublishOptions {
    /// Protected Footon share endpoint.
    #[incurs(default = "https://footon.dev/api/shares")]
    endpoint: String,
}

#[derive(Deserialize, incurs::Env)]
struct PublishEnv {
    /// Magic-link session bearer token. Never stored in a draft.
    #[incurs(env = "FOOTON_TOKEN")]
    footon_token: String,
}

/// Build Footon's typed incurs command graph.
#[must_use]
pub fn app() -> Cli {
    Cli::create("footon")
        .version(env!("CARGO_PKG_VERSION"))
        .description("Sanitize agent threads locally, then publish only an approved safe draft")
        .command("draft", draft_command())
        .command("publish", publish_command())
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
    CommandDef::typed::<PublishArgs, PublishOptions, PublishEnv, PublishResponse, _, _>(
        "publish",
        |context| async move {
            match publish_draft(context).await {
                Ok(output) => TypedResult::ok(output),
                Err(error) => typed_error(&error),
            }
        },
    )
    .description("Explicitly approve and publish one sanitized footon.share.v1 draft")
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

async fn publish_draft(
    context: TypedContext<PublishArgs, PublishOptions, PublishEnv>,
) -> crate::error::Result<PublishResponse> {
    let draft = draft::read(Path::new(&context.args.draft))?;
    let share = publish::build_share(draft, chrono::Utc::now())?;
    publish::send(&context.options.endpoint, &context.env.footon_token, &share).await
}

fn typed_error<T>(error: &Error) -> TypedResult<T> {
    TypedResult::error(error_code(error), error.to_string())
}

fn error_code(error: &Error) -> &'static str {
    match error {
        Error::Read { .. } | Error::Write { .. } => "LOCAL_IO_ERROR",
        Error::Json(_) | Error::NoMessages | Error::Source(_) => "INVALID_THREAD",
        Error::Safety(_) => "SAFETY_FAILED",
        Error::Share(_) => "INVALID_DRAFT",
        Error::Endpoint(_) => "UNSAFE_ENDPOINT",
        Error::Publish(_) => "PUBLISH_FAILED",
    }
}

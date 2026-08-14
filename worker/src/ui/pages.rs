use footon_core::model::{Message, Role};
use topcoat::{
    Result,
    context::Cx,
    view::{component, view},
};

use crate::ui::{
    layout::{command_block, page_layout, page_shell},
    thread::thread_demo,
};

const INSTALL_COMMAND: &str = "cargo install --git https://github.com/douglance/footon footon";
const SHARE_COMMAND: &str = "footon draft thread.jsonl --title \"Public title\" --output footon-draft.json\nFOOTON_TOKEN=... footon publish footon-draft.json";
const BLACKOUT_COMMAND: &str = "footon blackout footon-draft.json 12 \"exact private text\"\nFOOTON_TOKEN=... footon blackout-share https://footon.dev/s/... 12 \"exact private text\"";
const FETCH_COMMAND: &str = "footon fetch https://footon.dev/s/...";

pub(crate) fn home_markdown() -> &'static str {
    r#"# Footon

Share the thread. Keep the secrets.

You are an agent. Footon gives you a small, safe handoff path for a prompt chain or transcript: keep the raw thread local, create a sanitized draft, review it, and publish only the safe copy that was explicitly approved.

## Actual Footon output

## USER

Share the deployment plan with the review agent. Keep credentials and local paths private.

## AGENT

I sanitized the thread and kept only the context needed for review.

### TOOL

exec cargo 2 arguments

### FILE

update release-plan.md

## AGENT

Ready to share. 4 redactions. [BLACKED OUT]

## Install

```sh
cargo install --git https://github.com/douglance/footon footon
```

## Draft, review, and publish

```sh
footon draft thread.jsonl --title "Public title" --output footon-draft.json
FOOTON_TOKEN=... footon publish footon-draft.json
```

Your raw transcript never leaves your machine. Footon publishes only the approved draft and rescans it server-side before storage.

## Read a share as Markdown

```sh
footon fetch https://footon.dev/s/...
```

You can also request any Footon page with `Accept: text/markdown`.

## Connect by MCP

Endpoint: `https://footon.dev/mcp`

Scopes: `shares:read shares:write`
"#
}

pub(crate) fn install_markdown() -> &'static str {
    r"# Install Footon

Footon is a Rust CLI for agents that need to share a safe, reviewed transcript.

```sh
cargo install footon
```

Fetch a shared thread as Markdown:

```sh
footon fetch https://footon.dev/s/...
```
"
}

pub(crate) fn connect_markdown() -> &'static str {
    r"# Connect Footon

Connect your agent to the Footon remote MCP server.

- MCP endpoint: `https://footon.dev/mcp`
- OAuth: Authorization code with PKCE S256
- Scopes: `shares:read shares:write`
"
}

pub(crate) fn check_email_markdown() -> &'static str {
    r"# Check your email for Footon

Footon sent a one-time sign-in link. It expires in 10 minutes and can be used once. The human who owns the email address must complete this step.
"
}

pub(crate) fn llms_markdown() -> &'static str {
    r"# Footon

> Footon helps agents share a safe, reviewed version of a prompt chain or transcript.

Keep the raw transcript local. Create and review a sanitized draft, then publish only the approved copy as an unlisted, revocable URL.

## Agent entry points

- [Home](https://footon.dev/): Product purpose, actual output, and the complete CLI workflow.
- [Install](https://footon.dev/install): Install the Rust CLI and fetch shares as Markdown.
- [Connect](https://footon.dev/connect): Remote MCP endpoint, OAuth method, and scopes.
- MCP endpoint: `https://footon.dev/mcp`

Request any Footon page with `Accept: text/markdown` for the agent-readable representation.
"
}

pub(crate) async fn home_page() -> Result {
    let cx = &Cx::default();
    let demo_messages = vec![
        Message::new(
            Role::User,
            "Share the deployment plan with the review agent. Keep credentials and local paths private.",
        ),
        Message::new(
            Role::Assistant,
            "I’ll inspect the release context, remove private details, and keep the decisions needed for review.",
        ),
        Message::new(Role::Tool, "exec cargo 2 arguments"),
        Message::new(Role::File, "read release-plan.md"),
        Message::new(
            Role::Assistant,
            "The release is buildable, but the rollout checklist still needs an owner and a recovery step.",
        ),
        Message::new(
            Role::User,
            "Add those gaps. Do not expose the signing token or my machine path.",
        ),
        Message::new(
            Role::Assistant,
            "I’ll update the public plan with bounded ownership and rollback details.",
        ),
        Message::new(Role::Tool, "exec git 3 arguments"),
        Message::new(Role::File, "update release-plan.md"),
        Message::new(
            Role::Assistant,
            "## Review summary\n\n- Build and smoke test before rollout.\n- Assign the release owner.\n- Restore the previous Worker version if health checks fail.",
        ),
        Message::new(
            Role::User,
            "Good. Black out the remaining private value and prepare the reviewed copy.",
        ),
        Message::new(
            Role::Assistant,
            "I found one remaining credential reference and replaced it before publishing.",
        ),
        Message::new(Role::Tool, "exec footon 4 arguments"),
        Message::new(Role::File, "write footon-draft.json"),
        Message::new(
            Role::Assistant,
            "Ready to share. The reviewed thread has 4 redactions and no local paths. [BLACKED OUT]",
        ),
    ];
    view! {
        cx =>
        page_layout(
            title: "Footon",
            description: Some("Footon lets agents share a safe, reviewed version of a thread or transcript."),
            body_class: "landing-page",
            main_class: "landing",
            <div class="landing-nav">
                <a class="brand" href="/">"footon"</a>
                <span>"FOR AGENTS"</span>
            </div>
            landing_hero(messages: &demo_messages)
            landing_workflow()
            landing_commands()
            landing_blackout()
            landing_footer()
        )
    }
}

#[component]
async fn landing_hero(messages: &[Message]) -> Result {
    view! {
        <section class="landing-hero">
            <div class="landing-intro">
                <p class="landing-eyebrow">"FOOTON / SAFE AGENT HANDOFFS"</p>
                <h1>"Share the thread. Keep the secrets."</h1>
                <p class="landing-lede">
                    "You keep the raw transcript local. Footon creates a sanitized draft for your review, then publishes only the safe copy you approve."
                </p>
                <p class="landing-flow">"DRAFT LOCALLY → REVIEW → SHARE"</p>
                command_block(command: INSTALL_COMMAND, class: "landing-install")
                <p class="landing-proof">"LOCAL RAW · SERVER RESCAN · MARKDOWN NATIVE"</p>
            </div>
            thread_demo(messages: messages)
        </section>
    }
}

#[component]
async fn landing_workflow() -> Result {
    view! {
        <section class="landing-workflow" aria-labelledby="workflow-title">
            <p class="landing-eyebrow">"THE HANDOFF"</p>
            <h2 id="workflow-title">"From live context to a safe link."</h2>
            <div class="workflow-grid">
                <section>
                    <p class="workflow-number">"01 / DRAFT"</p>
                    <h3>"Sanitize locally"</h3>
                    <p>"Convert your agent transcript into a bounded draft. Secrets and noisy tool details are removed before anything is uploaded."</p>
                </section>
                <section>
                    <p class="workflow-number">"02 / REVIEW"</p>
                    <h3>"Approve the copy"</h3>
                    <p>"Read the exact file that will be shared. Add explicit blackouts when one message still contains private text."</p>
                </section>
                <section>
                    <p class="workflow-number">"03 / SHARE"</p>
                    <h3>"Send one URL"</h3>
                    <p>"Publish an unlisted, revocable link. Humans get the rendered transcript; agents request the same page as Markdown."</p>
                </section>
            </div>
        </section>
    }
}

#[component]
async fn landing_commands() -> Result {
    view! {
        <section class="landing-commands" aria-label="Footon commands">
            <div>
                <p class="landing-eyebrow">"CLI WORKFLOW"</p>
                <h2>"Draft, inspect, publish."</h2>
                <p>"The original transcript stays local. Footon uploads only the approved draft and scans it again before storage."</p>
                command_block(command: SHARE_COMMAND, class: "landing-command")
            </div>
            <div>
                <p class="landing-eyebrow">"AGENT ACCESS"</p>
                <h2>"Fetch Markdown directly."</h2>
                <p>"Use the CLI or send `Accept: text/markdown` to any Footon page. Connect through MCP when your agent should create, list, blackout, or revoke shares."</p>
                command_block(command: FETCH_COMMAND, class: "landing-command")
                <p class="landing-links"><a href="/connect">"Connect MCP"</a> " · " <a href="/install">"Install details"</a></p>
            </div>
        </section>
    }
}

#[component]
async fn landing_blackout() -> Result {
    view! {
        <section class="landing-blackout" aria-labelledby="blackout-title">
            <div>
                <p class="landing-eyebrow">"PRECISE REDACTION"</p>
                <h2 id="blackout-title">"Black out one exact substring."</h2>
                <p>"Target one message number and one literal value. Footon replaces it with [BLACKED OUT] locally or in a live share you own. The same typed commands work in Incurs Code Mode."</p>
            </div>
            <div>
                command_block(command: BLACKOUT_COMMAND, class: "landing-command")
            </div>
        </section>
    }
}

#[component]
async fn landing_footer() -> Result {
    view! {
        <footer class="landing-footer">
            <a class="brand" href="/">"footon"</a>
            <p>"A small, safe handoff layer for agent transcripts."</p>
            <code>"https://footon.dev/mcp"</code>
        </footer>
    }
}

pub(crate) async fn install_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Install Footon",
            body_class: "bg-canvas text-ink",
            main_class: "mx-auto max-w-2xl px-4 py-8",
            page_shell(
                eyebrow_text: "FOOTON / INSTALL",
                title: "Install the Rust CLI",
                title_class: "mt-2 text-2xl font-semibold",
                command_block(
                    command: "cargo install footon",
                    class: "mt-5 overflow-x-auto border border-line bg-paper p-3 text-sm"
                )
                <p class="mt-4 text-sm text-muted">
                    "Fetch a shared thread as Markdown with "
                    <code>(FETCH_COMMAND)</code>
                    "."
                </p>
            )
        )
    }
}

pub(crate) async fn connect_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Connect Footon",
            body_class: "bg-canvas text-ink",
            main_class: "mx-auto max-w-2xl px-4 py-8",
            page_shell(
                eyebrow_text: "FOOTON / CONNECT",
                title: "Connect an agent",
                title_class: "mt-2 text-2xl font-semibold",
                <dl class="mt-5 grid grid-cols-[8rem_1fr] gap-x-3 gap-y-2 border-y border-line py-3 text-sm">
                    <dt class="text-muted">"MCP endpoint"</dt>
                    <dd><code>"https://footon.dev/mcp"</code></dd>
                    <dt class="text-muted">"OAuth"</dt>
                    <dd>"Authorization code + PKCE S256"</dd>
                    <dt class="text-muted">"Scopes"</dt>
                    <dd><code>"shares:read shares:write"</code></dd>
                </dl>
            )
        )
    }
}

pub(crate) async fn check_email_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Check your email",
            body_class: "bg-canvas text-ink",
            main_class: "mx-auto max-w-md px-4 py-10",
            page_shell(
                eyebrow_text: "FOOTON / AUTHORIZE",
                title: "Check your email",
                title_class: "mt-2 text-2xl font-semibold",
                <p class="mt-3 text-sm text-muted">
                    "The sign-in link expires in 10 minutes and can be used once."
                </p>
            )
        )
    }
}

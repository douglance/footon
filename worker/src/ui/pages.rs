use footon_core::model::{Message, Role};
use topcoat::{
    Result,
    context::Cx,
    view::{component, view},
};

use crate::ui::{
    layout::{ASSET_VERSION, command_block, page_layout},
    thread::thread_demo,
};

pub(crate) const LANDING_JS: &str = include_str!("../landing.js");
const INSTALL_PROMPT: &str = "Install Footon for this workspace. Run `cargo install footon --locked`, then run `footon --help` to verify the install. Report any error without exposing credentials or private paths.";
const SHARE_COMMAND: &str = "footon signin you@example.com\nfooton draft thread.jsonl --title \"Public title\" --output footon-draft.json\nfooton publish footon-draft.json";
const BLACKOUT_COMMAND: &str = "footon blackout footon-draft.json 12 \"exact private text\"\nfooton blackout-share https://footon.dev/s/... 12 \"exact private text\"";
const FETCH_COMMAND: &str = "footon fetch https://footon.dev/s/...";

pub(crate) fn home_markdown() -> &'static str {
    r#"# Footon

Share the thread. Keep your secrets.

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

## Agent prompt

```text
Install Footon for this workspace. Run `cargo install footon --locked`, then run `footon --help` to verify the install. Report any error without exposing credentials or private paths.
```

## Draft, review, and publish

```sh
footon draft thread.jsonl --title "Public title" --output footon-draft.json
footon publish footon-draft.json
```

Your raw transcript never leaves your machine. Footon publishes only the approved draft and checks it again before it can be shared.

## Read a share as Markdown

```sh
footon fetch https://footon.dev/s/...
```

You can also request any Footon page with `Accept: text/markdown`.

## Connect by MCP

Endpoint: `https://footon.dev/mcp`

OAuth: Authorization code with PKCE S256

Scopes: `shares:read shares:write`

## Product and policy

- [Pricing](https://footon.dev/pricing): Free and Pro prices, limits, and hosted checkout.
- [Security](https://footon.dev/security): Safety boundaries and private reporting.
- [Support](https://footon.dev/support): Private account, billing, privacy, and security help.
- [Privacy](https://footon.dev/privacy): Stored data, processors, retention, and deletion controls.
- [Terms](https://footon.dev/terms): Paid-plan, cancellation, refund, and acceptable-use terms.
"#
}

pub(crate) fn privacy_markdown() -> &'static str {
    r"# Privacy Policy

Effective August 14, 2026.

Footon is operated by Douglas Lance as an individual developer. Footon keeps raw transcripts local while you draft. The service receives a thread only when you publish the reviewed draft.

## Data Footon processes

Footon stores the published title and sanitized thread, an account identifier, and creation and revocation timestamps. For sign-in, it stores your normalized email address, approved OAuth scopes and client details, hashed one-time codes and tokens, expiry times, and security state. It never stores raw sign-in codes or raw access and refresh tokens.

For paid plans, Footon stores your billing email, Lemon Squeezy customer, order, product, variant, subscription and event identifiers, subscription state and dates, entitlement limit, test-mode flag, and customer-portal URL. Lemon Squeezy is the merchant of record and hosts checkout. Footon does not receive or store payment-card details.

## Processors and purposes

Cloudflare provides network delivery, Worker hosting, D1 storage, operational logs, and sign-in email delivery. Lemon Squeezy provides checkout, tax handling, payment processing, subscription administration, and billing records. Footon uses the data only to provide, secure, support, account for, and improve the service. Footon does not include advertising or third-party behavioral tracking code and does not sell personal information.

## Retention and deletion

Active shares remain available until you revoke them. Revocation removes public access immediately; the stored share content is deleted by the daily cleanup within 30 days. OAuth authorization codes expire after 5 minutes, email codes after 10 minutes, access tokens after 1 hour, refresh tokens after 30 days, and registered clients after 90 days; expired, used, or revoked credential records are removed by the daily cleanup.

Billing event, subscription, and entitlement records may be retained for up to 7 years after the related transaction or subscription ends for accounting, tax, fraud prevention, dispute handling, and legal compliance. Support email is retained only as long as reasonably needed to resolve the request and meet those obligations. Provider backups and security logs may persist for their configured retention windows before aging out.

Email support@footon.dev to request access, correction, export, or deletion of personal information. Footon will verify the request and honor it where applicable law requires, subject to security, accounting, dispute, and legal retention duties.

## Your responsibility

Do not publish information you do not have permission to share. Review the exact draft before publishing it. Shares are unlisted, not private: anyone with the URL can read an active share.

## Contact

For private privacy questions or requests, email support@footon.dev. Do not put private information in a public GitHub issue.
"
}

pub(crate) fn terms_markdown() -> &'static str {
    r"# Terms of Use

Effective August 14, 2026.

Footon is operated by Douglas Lance as an individual developer. These terms apply when you access or use the Footon website, CLI, MCP server, sharing service, or paid plan. By using Footon, you agree to them.

## Using Footon

Footon provides tools for creating and sharing a reviewed, sanitized copy of an agent thread. You are responsible for the content you publish, confirming that you have permission to share it, and reviewing the final draft before publication. Shares are unlisted, not private: anyone with a working share URL can read that share until it is revoked.

## Plans, renewal, and cancellation

The Free plan includes up to 3 active shares. Footon Pro includes up to 100 active shares and is offered for $12 per month or $120 per year in USD, plus any amount shown at checkout. Lemon Squeezy is the merchant of record and hosts checkout.

Paid subscriptions renew automatically for the selected monthly or annual period until canceled. You can cancel through the customer-portal link returned by Footon or by emailing support@footon.dev. Cancel before the renewal date to avoid the next charge. After cancellation, Pro remains active through the end of the paid period unless the subscription is refunded, reversed, or terminated for cause.

Except where applicable law requires otherwise, completed subscription payments are non-refundable. Email support@footon.dev for a duplicate charge, billing error, or exceptional refund request. Approved refunds are processed through Lemon Squeezy and may revoke Pro access. Future prices or plan limits may change with notice before a later renewal; a change does not alter a period already paid.

## Acceptable use

Do not use Footon to break the law, harm others, expose private information without permission, interfere with the service, or distribute malicious content.

## Your content

You keep your rights to your content and give Footon only the permission needed to host, process, display, and deliver it. Shares are unlisted, not private: anyone with a working share URL can read that share until it is revoked.

## Availability, warranties, and liability

Footon may change, suspend, or stop all or part of the service. Automated scanning reduces risk but cannot guarantee that a draft is safe, complete, available, or error-free. To the fullest extent permitted by law, Footon is provided as available without express or implied warranties. Footon and its operator are not liable for indirect, incidental, special, consequential, or punitive loss. For any claim relating to paid Footon service, aggregate liability will not exceed the amount you paid for Footon during the 12 months before the event giving rise to the claim. These limits do not apply where law prohibits them.

## Termination and law

You may stop using Footon at any time. Footon may suspend or terminate access for a material breach, security risk, legal requirement, or harm to the service or others. Terms that by their nature should survive termination remain effective. These terms are governed by applicable United States law, without overriding consumer protections that apply to you.

## Contact

For private legal or billing questions, email support@footon.dev. Public product bugs may be reported at https://github.com/douglance/footon/issues without including private information.
"
}

pub(crate) fn llms_markdown() -> &'static str {
    r"# Footon

> Footon helps agents share a safe, reviewed version of a prompt chain or transcript.

Keep the raw transcript local. Create and review a sanitized draft, then publish only the approved copy as an unlisted, revocable URL.

## Agent entry points

- [Home](https://footon.dev/): Product purpose, actual output, installation, CLI workflow, and MCP connection details.
- [Pricing](https://footon.dev/pricing): Free and Pro prices, limits, and hosted checkout.
- [Security](https://footon.dev/security): Safety boundaries and private reporting.
- [Support](https://footon.dev/support): Private account, billing, privacy, and security help.
- [Privacy](https://footon.dev/privacy): Stored data, processors, retention, and deletion controls.
- [Terms](https://footon.dev/terms): Paid-plan, cancellation, refund, and acceptable-use terms.
- MCP endpoint: `https://footon.dev/mcp`

Request any Footon page with `Accept: text/markdown` for the agent-readable representation.
"
}

pub(crate) async fn home_page() -> Result {
    let cx = &Cx::default();
    let landing_script = format!("/landing.js?v={ASSET_VERSION}");
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
            "## Review summary\n\n- Build and smoke test before rollout.\n- Assign the release owner.\n- Restore the previous release if health checks fail.",
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
                <nav aria-label="Product">
                    <a href="/pricing">"Pricing"</a>
                    <a href="/security">"Security"</a>
                    <a href="/support">"Support"</a>
                </nav>
            </div>
            landing_hero(messages: &demo_messages)
            landing_workflow()
            landing_commands()
            landing_blackout()
            landing_footer()
            <script src=(landing_script) defer="defer"></script>
        )
    }
}

#[component]
async fn landing_hero(messages: &[Message]) -> Result {
    view! {
        <section class="landing-hero">
            <div class="landing-intro">
                <h1>
                    <span>"Share the thread. "</span>
                    <span>"Keep your secrets."</span>
                </h1>
                <p class="landing-lede">
                    "You keep the raw transcript local. Footon creates a sanitized draft for your review, then publishes only the safe copy you approve."
                </p>
                <p class="landing-flow">"DRAFT LOCALLY → REVIEW → SHARE"</p>
                <section class="landing-agent-prompt" aria-labelledby="install-prompt-label">
                    <div class="agent-prompt-bar">
                        <span id="install-prompt-label">"AGENT PROMPT"</span>
                        <button type="button" data-copy-target="install-agent-prompt">"COPY PROMPT"</button>
                        <span class="copy-status" data-copy-status="" aria-live="polite"></span>
                    </div>
                    <pre id="install-agent-prompt"><code>(INSTALL_PROMPT)</code></pre>
                </section>
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
                <p>"The original transcript stays local. Footon checks the approved draft again before it can be shared."</p>
                command_block(command: SHARE_COMMAND, class: "landing-command")
            </div>
            <div>
                <p class="landing-eyebrow">"AGENT ACCESS"</p>
                <h2>"Fetch Markdown directly."</h2>
                <p>"Use the CLI or send `Accept: text/markdown` to any Footon page. Connect through MCP when your agent should create, list, blackout, or revoke shares."</p>
                command_block(command: FETCH_COMMAND, class: "landing-command")
                <dl class="landing-connect">
                    <dt>"MCP endpoint"</dt>
                    <dd><code>"https://footon.dev/mcp"</code></dd>
                    <dt>"OAuth"</dt>
                    <dd>"Authorization code + PKCE S256"</dd>
                    <dt>"Scopes"</dt>
                    <dd><code>"shares:read shares:write"</code></dd>
                </dl>
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
                <p>"Choose one message and the exact text to remove. Footon replaces only that text with [BLACKED OUT] in your draft or an active share you own."</p>
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
            <p>"Share reviewed agent transcripts without sharing the original."</p>
            <nav aria-label="Product and legal">
                <a href="/pricing">"Pricing"</a>
                <a href="/security">"Security"</a>
                <a href="/support">"Support"</a>
                <a href="/privacy">"Privacy"</a>
                <a href="/terms">"Terms"</a>
            </nav>
            <code>"https://footon.dev/mcp"</code>
        </footer>
    }
}

pub(crate) async fn privacy_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Privacy Policy · Footon",
            description: Some("How Footon handles published shares and authentication data."),
            body_class: "legal-page",
            main_class: "legal",
            <nav class="legal-nav">
                <a class="brand" href="/">"footon"</a>
                <a href="/">"Home"</a>
            </nav>
            <article class="legal-copy">
                <p class="landing-eyebrow">"LEGAL"</p>
                <h1>"Privacy Policy"</h1>
                <p class="legal-effective">"Effective August 14, 2026"</p>
                <p>"Footon is operated by Douglas Lance as an individual developer. Raw transcripts stay local while you draft; the service receives a thread only when you publish the reviewed draft."</p>
                <h2>"Data Footon processes"</h2>
                <p>"Footon stores the published title and sanitized thread, an account identifier, and creation and revocation timestamps. For sign-in, it stores your normalized email address, approved OAuth scopes and client details, hashed one-time codes and tokens, expiry times, and security state. It never stores raw sign-in codes or raw access and refresh tokens."</p>
                <p>"For paid plans, Footon stores the billing email, Lemon Squeezy customer, order, product, variant, subscription and event identifiers, subscription state and dates, entitlement limit, test-mode flag, and customer-portal URL. Lemon Squeezy is the merchant of record and hosts checkout. Footon does not receive or store payment-card details."</p>
                <h2>"Processors and purposes"</h2>
                <p>"Cloudflare provides network delivery, Worker hosting, D1 storage, operational logs, and sign-in email delivery. Lemon Squeezy provides checkout, tax handling, payment processing, subscription administration, and billing records."</p>
                <p>"Footon uses the data only to provide, secure, support, account for, and improve the service. Footon does not include advertising or third-party behavioral tracking code and does not sell personal information."</p>
                <h2>"Retention and deletion"</h2>
                <p>"Active shares remain available until you revoke them. Revocation removes public access immediately; the stored share content is deleted by the daily cleanup within 30 days."</p>
                <p>"OAuth authorization codes expire after 5 minutes, email codes after 10 minutes, access tokens after 1 hour, refresh tokens after 30 days, and registered clients after 90 days. Expired, used, or revoked credential records are removed by the daily cleanup."</p>
                <p>"Billing event, subscription, and entitlement records may be retained for up to 7 years after the related transaction or subscription ends for accounting, tax, fraud prevention, dispute handling, and legal compliance. Support email is retained only as long as reasonably needed to resolve the request and meet those obligations. Provider backups and security logs may persist for their configured retention windows before aging out."</p>
                <p>"Email support@footon.dev to request access, correction, export, or deletion of personal information. Footon will verify the request and honor it where applicable law requires, subject to security, accounting, dispute, and legal retention duties."</p>
                <h2>"Your responsibility"</h2>
                <p>"Do not publish information you do not have permission to share. Review the exact draft before publishing it. Shares are unlisted, not private: anyone with the URL can read an active share."</p>
                <h2>"Contact"</h2>
                <p>"For private privacy questions or requests, email " <a href="mailto:support@footon.dev">"support@footon.dev"</a> ". Do not put private information in a public GitHub issue."</p>
            </article>
        )
    }
}

pub(crate) async fn terms_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Terms of Use · Footon",
            description: Some("The terms for using Footon."),
            body_class: "legal-page",
            main_class: "legal",
            <nav class="legal-nav">
                <a class="brand" href="/">"footon"</a>
                <a href="/">"Home"</a>
            </nav>
            <article class="legal-copy">
                <p class="landing-eyebrow">"LEGAL"</p>
                <h1>"Terms of Use"</h1>
                <p class="legal-effective">"Effective August 14, 2026"</p>
                <p>"Footon is operated by Douglas Lance as an individual developer. These terms apply when you access or use the Footon website, CLI, MCP server, sharing service, or paid plan. By using Footon, you agree to them."</p>
                <h2>"Using Footon"</h2>
                <p>"Footon provides tools for creating and sharing a reviewed, sanitized copy of an agent thread. You are responsible for the content you publish, confirming that you have permission to share it, and reviewing the final draft before publication. Shares are unlisted, not private: anyone with a working share URL can read that share until it is revoked."</p>
                <h2>"Plans, renewal, and cancellation"</h2>
                <p>"The Free plan includes up to 3 active shares. Footon Pro includes up to 100 active shares and is offered for $12 per month or $120 per year in USD, plus any amount shown at checkout. Lemon Squeezy is the merchant of record and hosts checkout."</p>
                <p>"Paid subscriptions renew automatically for the selected monthly or annual period until canceled. You can cancel through the customer-portal link returned by Footon or by emailing support@footon.dev. Cancel before the renewal date to avoid the next charge. After cancellation, Pro remains active through the end of the paid period unless the subscription is refunded, reversed, or terminated for cause."</p>
                <p>"Except where applicable law requires otherwise, completed subscription payments are non-refundable. Email support@footon.dev for a duplicate charge, billing error, or exceptional refund request. Approved refunds are processed through Lemon Squeezy and may revoke Pro access. Future prices or plan limits may change with notice before a later renewal; a change does not alter a period already paid."</p>
                <h2>"Acceptable use"</h2>
                <p>"Do not use Footon to break the law, harm others, expose private information without permission, interfere with the service, or distribute malicious content."</p>
                <h2>"Your content"</h2>
                <p>"You keep your rights to your content and give Footon only the permission needed to host, process, display, and deliver it. Shares are unlisted, not private: anyone with a working share URL can read that share until it is revoked."</p>
                <h2>"Availability, warranties, and liability"</h2>
                <p>"Footon may change, suspend, or stop all or part of the service. Automated scanning reduces risk but cannot guarantee that a draft is safe, complete, available, or error-free. To the fullest extent permitted by law, Footon is provided as available without express or implied warranties."</p>
                <p>"Footon and its operator are not liable for indirect, incidental, special, consequential, or punitive loss. For any claim relating to paid Footon service, aggregate liability will not exceed the amount you paid for Footon during the 12 months before the event giving rise to the claim. These limits do not apply where law prohibits them."</p>
                <h2>"Termination and law"</h2>
                <p>"You may stop using Footon at any time. Footon may suspend or terminate access for a material breach, security risk, legal requirement, or harm to the service or others. Terms that by their nature should survive termination remain effective. These terms are governed by applicable United States law, without overriding consumer protections that apply to you."</p>
                <h2>"Contact"</h2>
                <p>"For private legal or billing questions, email " <a href="mailto:support@footon.dev">"support@footon.dev"</a> ". Public product bugs may be reported on GitHub without including private information."</p>
            </article>
        )
    }
}

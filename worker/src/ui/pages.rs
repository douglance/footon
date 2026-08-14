use topcoat::{Result, context::Cx, view::view};

use crate::ui::layout::{command_block, page_layout, page_shell, section};

const INSTALL_COMMAND: &str = "cargo install --git https://github.com/douglance/footon footon";
const SHARE_COMMAND: &str = "footon draft thread.jsonl --title \"Public title\" --output footon-draft.json\nFOOTON_TOKEN=... footon publish footon-draft.json";
const FETCH_COMMAND: &str = "footon fetch https://footon.dev/s/...";

pub(crate) async fn home_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Footon",
            description: Some("Footon helps agents share a safe version of a thread or transcript."),
            body_class: "bg-canvas text-ink",
            main_class: "mx-auto max-w-3xl px-4 py-8",
            page_shell(
                eyebrow_text: "FOOTON / SAFE AGENT THREAD SHARING",
                title: "Share an agent thread safely.",
                title_class: "mt-2 max-w-[40ch] text-3xl font-semibold tracking-tight text-balance",
                <p class="mt-3 max-w-[56ch] text-base text-pretty text-muted sm:text-sm">
                    "Footon gives agents a simple way to share a safe version of their current thread or transcript with another person or agent. It removes sensitive data and noisy tool details. The original transcript stays local. Footon publishes only the draft you approve."
                </p>
                <p class="mt-3 font-mono text-sm text-muted">
                    "DRAFT LOCALLY → REVIEW → SHARE AN UNLISTED LINK"
                </p>
                section(
                    title: "Install the CLI",
                    id: Some("install"),
                    class: "mt-8",
                    command_block(
                        command: INSTALL_COMMAND,
                        class: "mt-3 overflow-x-auto bg-paper p-3 text-sm"
                    )
                )
                section(
                    title: "Share a safe thread",
                    class: "mt-8",
                    <p class="mt-2 text-base text-pretty text-muted sm:text-sm">
                        "Create a sanitized draft from an agent transcript, review it, then publish only the approved file."
                    </p>
                    command_block(
                        command: SHARE_COMMAND,
                        class: "mt-3 overflow-x-auto bg-paper p-3 text-sm"
                    )
                )
                section(
                    title: "Open a shared chain",
                    class: "mt-8",
                    <p class="mt-2 text-sm text-muted">
                        "Open the returned link in a browser, or fetch its Markdown for an agent."
                    </p>
                    command_block(
                        command: FETCH_COMMAND,
                        class: "mt-3 overflow-x-auto bg-paper p-3 text-sm"
                    )
                )
                section(
                    title: "Connect an agent",
                    class: "mt-8",
                    <p class="mt-2 text-sm text-muted">
                        "Use "
                        <code>"https://footon.dev/mcp"</code>
                        " as the remote MCP endpoint. Footon authorizes access with OAuth and passwordless email sign-in."
                    </p>
                    <p class="mt-4 text-sm"><a href="/connect">"View agent connection details"</a></p>
                )
            )
        )
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

use topcoat::{
    Result,
    context::Cx,
    view::{attributes, view},
};

use crate::{
    components::{button::button, input::input, label::label},
    ui::layout::{page_layout, page_shell},
};

pub(crate) struct AuthorizationPage {
    pub(crate) client_id: String,
    pub(crate) redirect_uri: String,
    pub(crate) scope: String,
    pub(crate) state: String,
    pub(crate) code_challenge: String,
    pub(crate) resource: String,
}

pub(crate) struct VerificationPage {
    pub(crate) ticket: String,
}

pub(crate) fn authorization_markdown(data: &AuthorizationPage) -> String {
    let scope = data
        .scope
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, ':' | ' '))
        .collect::<String>();
    let resource = if data.resource == "https://footon.dev/mcp" {
        data.resource.as_str()
    } else {
        "unrecognized resource; do not authorize"
    };
    format!(
        "# Authorize Footon\n\nAn agent or CLI is requesting access to your Footon account. Enter your email to receive a one-time six-digit code. The code expires in 10 minutes and works once.\n\n- Requested scopes: `{scope}`\n- Protected resource: `{resource}`\n- `shares:read`: list and read your active shares\n- `shares:write`: publish, blackout, and revoke your shares\n\nFooton uses the email to sign you in and associate your shares and plan. Give the code only to the agent or CLI completing this sign-in. Do not forward OAuth state or PKCE material. See https://footon.dev/privacy.\n"
    )
}

pub(crate) fn verification_markdown() -> &'static str {
    "# Enter your Footon code\n\nFooton sent a six-digit code to your email. It expires in 10 minutes and can be used once. Give the code only to the agent completing this sign-in.\n"
}

pub(crate) async fn authorize_page(data: &AuthorizationPage) -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Authorize Footon",
            body_class: "bg-canvas text-ink",
            main_class: "mx-auto max-w-md px-4 py-8",
            page_shell(
                eyebrow_text: "FOOTON / AUTHORIZE",
                title: "Authorize agent access",
                title_class: "mt-2 text-2xl font-semibold",
                <p class="mt-2 text-sm text-muted">
                    "An agent or CLI is requesting access to your Footon account."
                </p>
                <dl class="mt-4 grid gap-2 border border-line bg-paper p-3 text-sm">
                    <div>
                        <dt class="text-xs font-medium text-muted">"Protected resource"</dt>
                        <dd class="mt-1 font-mono text-xs">(&data.resource)</dd>
                    </div>
                    <div>
                        <dt class="text-xs font-medium text-muted">"Requested access"</dt>
                        <dd class="mt-1 font-mono text-xs">(&data.scope)</dd>
                    </div>
                </dl>
                <ul class="mt-4 grid gap-2 text-sm text-muted">
                    <li><code>"shares:read"</code> " lists and reads your active shares."</li>
                    <li><code>"shares:write"</code> " publishes, blackouts, and revokes your shares."</li>
                </ul>
                <form class="mt-5 grid gap-3" method="post" action="/auth/request">
                    <div class="grid gap-1">
                        label(
                            attrs: attributes! { cx => for="email" class="text-xs font-medium" },
                            "Email"
                        )
                        input(attrs: attributes! {
                            cx =>
                            id="email"
                            class="authorization-input rounded-none bg-paper font-mono shadow-none"
                            name="email"
                            type="email"
                            autocomplete="email"
                            required="required"
                        })
                    </div>
                    <input type="hidden" name="client_id" value=(&data.client_id)>
                    <input type="hidden" name="redirect_uri" value=(&data.redirect_uri)>
                    <input type="hidden" name="scope" value=(&data.scope)>
                    <input type="hidden" name="state" value=(&data.state)>
                    <input type="hidden" name="code_challenge" value=(&data.code_challenge)>
                    <input type="hidden" name="code_challenge_method" value="S256">
                    <input type="hidden" name="resource" value=(&data.resource)>
                    button(
                        attrs: attributes! {
                            cx =>
                            class="rounded-none bg-ink font-mono text-paper shadow-none"
                            type="submit"
                        },
                        "Send code"
                    )
                </form>
                <p class="mt-4 text-xs text-muted">
                    "Footon uses your email to sign you in and associate your shares and plan. The six-digit code expires in 10 minutes and works once. Give it only to the agent or CLI completing this sign-in. "
                    <a class="underline" href="/privacy">"Privacy details"</a>
                </p>
            )
        )
    }
}

pub(crate) async fn verification_page(data: &VerificationPage) -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Enter your Footon code",
            body_class: "bg-canvas text-ink",
            main_class: "mx-auto max-w-md px-4 py-8",
            page_shell(
                eyebrow_text: "FOOTON / VERIFY",
                title: "Enter your email code",
                title_class: "mt-2 text-2xl font-semibold",
                <p class="mt-2 text-sm text-muted">
                    "Use the six-digit code Footon just sent. It expires in 10 minutes."
                </p>
                <form class="mt-5 grid gap-3" method="post" action="/auth/verify">
                    <div class="grid gap-1">
                        label(
                            attrs: attributes! { cx => for="code" class="text-xs font-medium" },
                            "Email code"
                        )
                        input(attrs: attributes! {
                            cx =>
                            id="code"
                            class="rounded-none bg-paper font-mono tracking-[0.2em] shadow-none"
                            name="code"
                            type="text"
                            autocomplete="one-time-code"
                            inputmode="numeric"
                            pattern="[0-9]{6}"
                            minlength="6"
                            maxlength="6"
                            required="required"
                            autofocus="autofocus"
                        })
                    </div>
                    <input type="hidden" name="ticket" value=(&data.ticket)>
                    button(
                        attrs: attributes! {
                            cx =>
                            class="rounded-none bg-ink font-mono text-paper shadow-none"
                            type="submit"
                        },
                        "Verify code"
                    )
                </form>
            )
        )
    }
}

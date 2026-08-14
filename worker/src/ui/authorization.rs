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
    pub(crate) turnstile_site_key: Option<String>,
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
                    "Requested scopes: "
                    <code>(&data.scope)</code>
                </p>
                <form class="mt-5 grid gap-3" method="post" action="/auth/request">
                    <div class="grid gap-1">
                        label(
                            attrs: attributes! { cx => for="email" class="text-xs font-medium" },
                            "Email"
                        )
                        input(attrs: attributes! {
                            cx =>
                            id="email"
                            class="rounded-none bg-paper font-mono shadow-none"
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
                    if let Some(site_key) = &data.turnstile_site_key {
                        <div class="cf-turnstile" data-sitekey=(site_key)></div>
                        <script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async="async" defer="defer"></script>
                    }
                    button(
                        attrs: attributes! {
                            cx =>
                            class="rounded-none bg-ink font-mono text-paper shadow-none"
                            type="submit"
                        },
                        "Send magic link"
                    )
                </form>
            )
        )
    }
}

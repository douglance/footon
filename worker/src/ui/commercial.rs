use topcoat::{
    Result,
    context::Cx,
    view::{component, view},
};

use crate::ui::layout::page_layout;

pub(crate) fn pricing_markdown() -> &'static str {
    r"# Footon pricing

Simple limits for individual developers. Prices are tax-inclusive USD where applicable.

## Free - $0

- Up to 3 active shares
- Local sanitization and review
- Unlisted, revocable links
- HTML for people and Markdown for agents

## Pro monthly - $12 per month

- Up to 100 active shares
- The complete Footon safety and sharing workflow
- Cancel any time; access continues through the paid period

## Pro annual - $120 per year

- Up to 100 active shares
- Two months less than monthly billing over one year
- Cancel any time; access continues through the paid period

Lemon Squeezy is the merchant of record and hosts checkout. Footon does not receive or store payment-card details. Enter the same email you use with Footon so the purchase can be associated with your account.

For billing help or a refund request, contact support@footon.dev. Refund eligibility is reviewed under the Terms and applicable law.
"
}

pub(crate) fn security_markdown() -> &'static str {
    r"# Footon security

Footon keeps the raw transcript local while you draft. The CLI removes known secret, personal-data, path, and injected-instruction patterns before publication. You review the exact draft, explicitly approve it, and the service validates and scans it again before storage.

Shared links are unlisted, not private. Anyone with a working URL can read that share until its owner revokes it. Automated scanning reduces risk but cannot guarantee that every sensitive value will be detected.

Authentication uses short-lived access tokens, rotating refresh tokens, PKCE S256, and one-time email codes. The CLI stores reusable credentials in the operating-system credential store.

Report a suspected vulnerability privately to support@footon.dev. Do not include live credentials, full private transcripts, access tokens, or unnecessary personal data. Include the affected URL or version, impact, safe reproduction steps, and a private way to reply.

Security fixes target the current Footon release. Update to the latest release before reporting a problem that may already be fixed.
"
}

pub(crate) fn support_markdown() -> &'static str {
    r"# Footon support

Email support@footon.dev for private account, billing, privacy, or security help. Public product bugs and feature requests may also be opened at https://github.com/douglance/footon/issues.

Include the Footon version, the command or page involved, the approximate time and timezone, the HTTP status or safe error text, and the smallest reproduction steps.

Never send passwords, one-time codes, access or refresh tokens, payment-card data, complete private transcripts, or local credential-store contents. Replace sensitive values with `[REDACTED]`.

Support is provided on a reasonable-effort basis for this individual-developer launch. No response-time or availability SLA is offered.
"
}

pub(crate) async fn pricing_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Pricing · Footon",
            description: Some("Free and Pro pricing for Footon, the safety-first agent thread sharing service."),
            body_class: "commercial-page",
            main_class: "commercial",
            commercial_nav()
            <header class="commercial-hero">
                <p class="landing-eyebrow">"PRICING"</p>
                <h1>"Share safely. Pay when you need more room."</h1>
                <p>"Start with three active shares. Pro raises the limit to 100 without changing the local-first review workflow."</p>
            </header>
            <section class="plan-grid" aria-label="Footon plans">
                <article class="plan-card">
                    <p class="plan-name">"FREE"</p>
                    <p class="plan-price"><strong>"$0"</strong></p>
                    <p class="plan-period">"No card. No trial."</p>
                    <ul>
                        <li>"3 active shares"</li>
                        <li>"Local sanitization and review"</li>
                        <li>"Unlisted, revocable links"</li>
                        <li>"HTML and Markdown output"</li>
                    </ul>
                    <a class="plan-secondary" href="/#install-agent-prompt">"Install Footon"</a>
                </article>
                <article class="plan-card plan-card-pro">
                    <p class="plan-name">"PRO / MONTHLY"</p>
                    <p class="plan-price"><strong>"$12"</strong> <span>"/ month"</span></p>
                    <p class="plan-period">"Tax-inclusive USD where applicable."</p>
                    <ul>
                        <li>"100 active shares"</li>
                        <li>"Complete safety and sharing workflow"</li>
                        <li>"Cancel any time"</li>
                        <li>"Access through the paid period"</li>
                    </ul>
                    checkout_form(
                        action: "/checkout/monthly",
                        input_id: "monthly-email",
                        button_text: "Continue to monthly checkout"
                    )
                </article>
                <article class="plan-card plan-card-pro">
                    <p class="plan-name">"PRO / ANNUAL"</p>
                    <p class="plan-price"><strong>"$120"</strong> <span>"/ year"</span></p>
                    <p class="plan-period">"Two months less than monthly billing."</p>
                    <ul>
                        <li>"100 active shares"</li>
                        <li>"Complete safety and sharing workflow"</li>
                        <li>"Cancel any time"</li>
                        <li>"Access through the paid period"</li>
                    </ul>
                    checkout_form(
                        action: "/checkout/annual",
                        input_id: "annual-email",
                        button_text: "Continue to annual checkout"
                    )
                </article>
            </section>
            <section class="commercial-note" aria-labelledby="payment-boundary">
                <p class="landing-eyebrow">"PAYMENT BOUNDARY"</p>
                <h2 id="payment-boundary">"Checkout is hosted by Lemon Squeezy."</h2>
                <p>"Lemon Squeezy is the merchant of record. Footon receives the purchase status and account email needed to apply your plan, but it does not receive or store payment-card details."</p>
                <p>"Use the same email you use with Footon. For billing help or a refund request, " <a href="mailto:support@footon.dev">"email support@footon.dev"</a> "."</p>
            </section>
        )
    }
}

pub(crate) async fn security_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Security · Footon",
            description: Some("Footon's local-first sharing boundary, authentication controls, and security reporting channel."),
            body_class: "commercial-page",
            main_class: "commercial commercial-copy",
            commercial_nav()
            <article>
                <p class="landing-eyebrow">"SECURITY"</p>
                <h1>"A safer handoff, with explicit limits."</h1>
                <p class="commercial-lede">"Footon reduces accidental disclosure by keeping raw context local, requiring review, and scanning again at publication. Automated scanning cannot guarantee that every sensitive value will be found."</p>
                <h2>"Local first"</h2>
                <p>"The CLI drafts and sanitizes on your machine. Footon receives a thread only after you review and approve the exact sanitized copy."</p>
                <h2>"Checked twice"</h2>
                <p>"The service validates the document shape and scans the approved copy again before storage. Owners can black out exact text or revoke a link."</p>
                <h2>"Unlisted, not private"</h2>
                <p>"Anyone with a working share URL can read it until revocation. Do not publish data you cannot safely disclose to every recipient of that URL."</p>
                <h2>"Account protection"</h2>
                <p>"Footon uses one-time email codes, PKCE S256, short-lived access tokens, rotating refresh tokens, and operating-system credential storage in the CLI."</p>
                <h2>"Report a vulnerability"</h2>
                <p>"Email " <a href="mailto:support@footon.dev">"support@footon.dev"</a> " privately. Include impact, the affected version or URL, and safe reproduction steps. Never send live credentials or complete private transcripts."</p>
                <h2>"Supported versions"</h2>
                <p>"Security fixes target the current release. Update to the latest Footon release before reporting a problem that may already be fixed."</p>
            </article>
        )
    }
}

pub(crate) async fn support_page() -> Result {
    let cx = &Cx::default();
    view! {
        cx =>
        page_layout(
            title: "Support · Footon",
            description: Some("Private support and credential-safe diagnostic guidance for Footon."),
            body_class: "commercial-page",
            main_class: "commercial commercial-copy",
            commercial_nav()
            <article>
                <p class="landing-eyebrow">"SUPPORT"</p>
                <h1>"Get help without sending secrets."</h1>
                <p class="commercial-lede">"For private account, billing, privacy, or security help, email " <a href="mailto:support@footon.dev">"support@footon.dev"</a> "."</p>
                <h2>"What to include"</h2>
                <ul>
                    <li>"Footon version"</li>
                    <li>"Command or page involved"</li>
                    <li>"Approximate time and timezone"</li>
                    <li>"HTTP status or safe error text"</li>
                    <li>"Smallest reproduction steps"</li>
                </ul>
                <h2>"What never to include"</h2>
                <p>"Do not send passwords, one-time codes, access or refresh tokens, payment-card data, complete private transcripts, or credential-store contents. Replace sensitive values with [REDACTED]."</p>
                <h2>"Public product feedback"</h2>
                <p>"Non-sensitive bugs and feature requests may be opened in the " <a href="https://github.com/douglance/footon/issues">"public GitHub issue tracker"</a> "."</p>
                <h2>"Service level"</h2>
                <p>"Support is provided on a reasonable-effort basis for the individual-developer launch. Footon does not offer a response-time or availability SLA."</p>
            </article>
        )
    }
}

#[component]
async fn commercial_nav() -> Result {
    view! {
        <nav class="commercial-nav" aria-label="Footon">
            <a class="brand" href="/">"footon"</a>
            <div>
                <a href="/pricing">"Pricing"</a>
                <a href="/security">"Security"</a>
                <a href="/support">"Support"</a>
            </div>
        </nav>
    }
}

#[component]
async fn checkout_form(action: &str, input_id: &str, button_text: &str) -> Result {
    view! {
        <form class="checkout-form" method="post" action=(action)>
            <label for=(input_id)>"Footon email"</label>
            <input id=(input_id) name="email" type="email" autocomplete="email" required="required" placeholder="you@example.com">
            <button type="submit">(button_text)</button>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn commercial_pages_state_the_launch_plan_and_safety_boundaries() {
        let cx = Cx::default();
        let pricing = pricing_page().await.expect("pricing page").render(&cx);
        let security = security_page().await.expect("security page").render(&cx);
        let support = support_page().await.expect("support page").render(&cx);

        for expected in ["$0", "$12", "$120", "3 active shares", "100 active shares"] {
            assert!(pricing.contains(expected), "missing {expected}");
        }
        assert!(pricing.contains("Lemon Squeezy"));
        assert!(pricing.contains("type=\"email\""));
        assert!(security.contains("Unlisted, not private"));
        assert!(security.contains("cannot guarantee"));
        assert!(support.contains("support@footon.dev"));
        assert!(support.contains("[REDACTED]"));
    }
}

use topcoat::{
    Result,
    view::{View, component, view},
};

pub(crate) const ASSET_VERSION: &str = "20260814-live-demo-1";

#[component]
pub(crate) async fn page_layout(
    title: &str,
    #[default] description: Option<&str>,
    body_class: &str,
    main_class: &str,
    #[default] child: View,
) -> Result {
    let stylesheet = format!("/style.css?v={ASSET_VERSION}");
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width,initial-scale=1">
                if let Some(description) = description {
                    <meta name="description" content=(description)>
                }
                <title>(title)</title>
                <link rel="icon" href="/favicon.svg" type="image/svg+xml">
                <link rel="stylesheet" href=(stylesheet)>
            </head>
            <body class=(body_class)>
                <main class=(main_class)>(child)</main>
            </body>
        </html>
    }
}

#[component]
pub(crate) async fn page_shell(
    eyebrow_text: &str,
    title: &str,
    title_class: &str,
    #[default] child: View,
) -> Result {
    view! {
        eyebrow(text: eyebrow_text)
        <h1 class=(title_class)>(title)</h1>
        (child)
    }
}

#[component]
pub(crate) async fn eyebrow(text: &str) -> Result {
    view! { <p class="font-mono text-xs text-muted">(text)</p> }
}

#[component]
pub(crate) async fn command_block(command: &str, class: &str) -> Result {
    view! { <pre class=(class)><code>(command)</code></pre> }
}

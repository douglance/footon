use footon_core::accept::{ContentType, negotiate};
use topcoat::{context::Cx, view::View};
use worker::{Response, Result};

use crate::security_headers;

#[derive(Clone, Copy)]
pub(crate) enum HtmlPolicy {
    Standard,
    Authorization,
}

pub(crate) fn html(view: topcoat::Result<View>, policy: HtmlPolicy) -> Result<Response> {
    let view = view.map_err(|error| worker::Error::RustError(error.to_string()))?;
    let mut response = Response::from_html(view.render(&Cx::default()))?;
    security_headers(&mut response)?;
    if matches!(policy, HtmlPolicy::Authorization) {
        response.headers_mut().set(
            "Content-Security-Policy",
            "default-src 'none'; style-src 'self' 'unsafe-inline' https://challenges.cloudflare.com; script-src https://challenges.cloudflare.com; frame-src https://challenges.cloudflare.com; connect-src https://challenges.cloudflare.com; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
        )?;
    }
    Ok(response)
}

pub(crate) fn standard(view: topcoat::Result<View>) -> Result<Response> {
    let mut response = html(view, HtmlPolicy::Standard)?;
    response.headers_mut().set("Vary", "Accept")?;
    Ok(response)
}

pub(crate) fn negotiated_standard(
    accept: Option<&str>,
    view: topcoat::Result<View>,
    markdown_body: &str,
) -> Result<Response> {
    negotiated(accept, view, markdown_body, HtmlPolicy::Standard)
}

pub(crate) fn negotiated_authorization(
    accept: Option<&str>,
    view: topcoat::Result<View>,
    markdown_body: &str,
) -> Result<Response> {
    negotiated(accept, view, markdown_body, HtmlPolicy::Authorization)
}

fn negotiated(
    accept: Option<&str>,
    view: topcoat::Result<View>,
    markdown_body: &str,
    policy: HtmlPolicy,
) -> Result<Response> {
    match negotiate(accept) {
        None => Response::error("not acceptable", 406),
        Some(ContentType::Markdown) => markdown(markdown_body),
        Some(ContentType::Html) => {
            let mut response = html(view, policy)?;
            response.headers_mut().set("Vary", "Accept")?;
            Ok(response)
        }
    }
}

pub(crate) fn markdown(markdown_body: &str) -> Result<Response> {
    let mut response = Response::ok(markdown_body.to_string())?;
    response
        .headers_mut()
        .set("Content-Type", "text/markdown; charset=utf-8")?;
    response.headers_mut().set("Vary", "Accept")?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    security_headers(&mut response)?;
    Ok(response)
}

use topcoat::{context::Cx, view::View};
use worker::{Response, Result};

use crate::security_headers;

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
    html(view, HtmlPolicy::Standard)
}

pub(crate) fn authorization(view: topcoat::Result<View>) -> Result<Response> {
    html(view, HtmlPolicy::Authorization)
}

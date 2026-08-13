use reqwest::header::{ACCEPT, CONTENT_TYPE};
use url::Url;

use crate::error::{Error, Result};

const MAX_MARKDOWN_BYTES: u64 = 1_000_000;
const MAX_REDIRECTS: usize = 3;

/// Fetch a shared Footon thread as Markdown.
///
/// # Errors
///
/// Returns an error when the URL is unsafe, the server does not negotiate
/// Markdown, redirects leave the origin, or the body exceeds the size bound.
pub async fn fetch_markdown(url: &str) -> Result<String> {
    let mut url = validate_share_url(url)?;
    let origin = origin_key(&url);
    let client = client()?;

    for _ in 0..=MAX_REDIRECTS {
        let response = request_markdown(&client, &url).await?;

        if response.status().is_redirection() {
            let next = redirect_target(&url, &response)?;
            if origin_key(&next) != origin {
                return Err(Error::Fetch("redirect changed origin".to_string()));
            }
            url = next;
            continue;
        }

        return read_markdown(response).await;
    }

    Err(Error::Fetch("too many redirects".to_string()))
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| Error::Fetch(error.to_string()))
}

async fn request_markdown(client: &reqwest::Client, url: &Url) -> Result<reqwest::Response> {
    client
        .get(url.clone())
        .header(ACCEPT, "text/markdown")
        .send()
        .await
        .map_err(|error| Error::Fetch(error.to_string()))
}

fn redirect_target(url: &Url, response: &reqwest::Response) -> Result<Url> {
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| Error::Fetch("redirect response missing Location".to_string()))?;
    url.join(location)
        .map_err(|error| Error::Fetch(format!("invalid redirect: {error}")))
}

async fn read_markdown(response: reqwest::Response) -> Result<String> {
    validate_response_headers(&response)?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| Error::Fetch(error.to_string()))?;
    if bytes.len() as u64 > MAX_MARKDOWN_BYTES {
        return Err(Error::Fetch("markdown response exceeds 1 MB".to_string()));
    }
    String::from_utf8(bytes.to_vec()).map_err(|error| Error::Fetch(error.to_string()))
}

fn validate_response_headers(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        return Err(Error::Fetch(format!(
            "server returned {}",
            response.status()
        )));
    }
    if !is_markdown(response) {
        return Err(Error::Fetch(
            "server did not return text/markdown".to_string(),
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MARKDOWN_BYTES)
    {
        return Err(Error::Fetch("markdown response exceeds 1 MB".to_string()));
    }
    Ok(())
}

fn is_markdown(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .split(';')
        .next()
        .is_some_and(|media| media.trim() == "text/markdown")
}

/// Validate that fetching uses HTTPS, except for loopback integration tests.
///
/// # Errors
///
/// Returns an error for malformed URLs, non-HTTPS remote URLs, or non-HTTP URLs.
pub fn validate_share_url(url: &str) -> Result<Url> {
    let url = Url::parse(url).map_err(|error| Error::Fetch(error.to_string()))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if !matches!(url.scheme(), "https" | "http") {
        return Err(Error::Fetch("only HTTP(S) URLs are supported".to_string()));
    }
    if url.scheme() != "https" && !loopback {
        return Err(Error::Fetch(
            "HTTPS is required outside loopback tests".to_string(),
        ));
    }
    Ok(url)
}

fn origin_key(url: &Url) -> String {
    format!(
        "{}://{}:{}",
        url.scheme(),
        url.host_str().unwrap_or_default(),
        url.port_or_known_default().unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::{fetch_markdown, validate_share_url};

    #[test]
    fn validates_https_except_loopback() {
        assert!(validate_share_url("https://footon.dev/s/share").is_ok());
        assert!(validate_share_url("http://127.0.0.1:8787/s/share").is_ok());
        assert!(validate_share_url("http://example.com/s/share").is_err());
        assert!(validate_share_url("file:///tmp/share.md").is_err());
    }

    #[tokio::test]
    async fn requests_markdown_and_returns_body() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/s/share_1", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 4096];
            let size = stream.read(&mut bytes).await.unwrap();
            let request = String::from_utf8(bytes[..size].to_vec()).unwrap();
            let body = "# Thread\n\n## AGENT\n\nDone\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            request
        });

        let markdown = fetch_markdown(&url).await.unwrap();
        let request = server.await.unwrap();

        assert!(request.contains("accept: text/markdown"));
        assert_eq!(markdown, "# Thread\n\n## AGENT\n\nDone\n");
    }
}

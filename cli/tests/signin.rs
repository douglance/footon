use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use footon::error::Result;
use footon::session::{CredentialStore, StoredSession};
use footon::signin::{begin, read_code};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn signin_passes_email_then_exchanges_the_prompted_code() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(serve_oauth_flow(listener));

    let pending = begin(&origin, "dlance@velostics.com").await.unwrap();
    let completed = pending.complete("123456").await.unwrap();
    let store = MemoryStore::default();
    let status = completed.save(&store).unwrap();
    let requests = server.await.unwrap();
    let session = store.session.lock().unwrap().clone().unwrap();

    assert_eq!(session.access_token(), "access-token");
    assert_eq!(session.refresh_token(), "refresh-token");
    assert_eq!(status.email, "dlance@velostics.com");
    assert_eq!(status.scope, "shares:read shares:write");
    assert_public_output_is_safe(&status);

    let registration: Value = request_json(&requests[0]);
    assert_eq!(registration["client_name"], "Footon CLI");
    assert_eq!(
        registration["redirect_uris"][0],
        "http://127.0.0.1/callback"
    );

    let request: Value = request_json(&requests[1]);
    assert_eq!(request["email"], "dlance@velostics.com");
    assert_eq!(request["client_id"], "fc_test");
    assert_eq!(request["code_challenge_method"], "S256");
    assert_eq!(request["resource"], format!("{origin}/mcp"));

    let verification: Value = request_json(&requests[2]);
    assert_eq!(verification["ticket"], "ticket-1");
    assert_eq!(verification["code"], "123456");

    let token = request_form(&requests[3]);
    assert_eq!(
        token.get("grant_type").map(String::as_str),
        Some("authorization_code")
    );
    assert_eq!(
        token.get("code").map(String::as_str),
        Some("authorization-code")
    );
    assert_eq!(token.get("client_id").map(String::as_str), Some("fc_test"));
    assert_eq!(
        token.get("resource").map(String::as_str),
        Some(format!("{origin}/mcp").as_str())
    );
    let verifier = token.get("code_verifier").unwrap();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    assert_eq!(request["code_challenge"], challenge);
}

fn assert_public_output_is_safe(status: &footon::signin::SigninResponse) {
    assert!(status.signed_in);
    let public_output = serde_json::to_string(status).unwrap();
    assert!(!public_output.contains("access-token"));
    assert!(!public_output.contains("refresh-token"));
}

#[derive(Default)]
struct MemoryStore {
    session: std::sync::Mutex<Option<StoredSession>>,
}

impl CredentialStore for MemoryStore {
    fn load(&self, _origin: &str) -> Result<Option<StoredSession>> {
        Ok(self.session.lock().unwrap().clone())
    }

    fn save(&self, session: &StoredSession) -> Result<()> {
        *self.session.lock().unwrap() = Some(session.clone());
        Ok(())
    }

    fn delete(&self, _origin: &str) -> Result<()> {
        *self.session.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn code_prompt_names_the_email_and_returns_only_six_digits() {
    let mut input = Cursor::new(b"123456\n".to_vec());
    let mut output = Vec::new();

    let code = read_code(&mut input, &mut output, "dlance@velostics.com").unwrap();

    assert_eq!(code, "123456");
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Code sent to dlance@velostics.com.\nEnter the six-digit code: "
    );
}

async fn serve_oauth_flow(listener: TcpListener) -> Vec<String> {
    let mut requests = Vec::new();
    let mut state = String::new();
    for step in 0..4 {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        let response = match step {
            0 => json_response(
                201,
                r#"{"client_id":"fc_test","client_name":"Footon CLI","redirect_uris":["http://127.0.0.1/callback"],"grant_types":["authorization_code","refresh_token"],"response_types":["code"],"token_endpoint_auth_method":"none","scope":"shares:read shares:write","client_id_issued_at":1,"client_secret_expires_at":0}"#,
            ),
            1 => {
                state = request_json(&request)["state"]
                    .as_str()
                    .unwrap()
                    .to_string();
                json_response(200, r#"{"ok":true,"ticket":"ticket-1","expiresIn":600}"#)
            }
            2 => redirect_response(&format!(
                "http://127.0.0.1/callback?code=authorization-code&state={state}"
            )),
            3 => json_response(
                200,
                r#"{"access_token":"access-token","token_type":"Bearer","expires_in":3600,"refresh_token":"refresh-token","scope":"shares:read shares:write"}"#,
            ),
            _ => unreachable!(),
        };
        stream.write_all(response.as_bytes()).await.unwrap();
        requests.push(request);
    }
    requests
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut bytes = vec![0; 16_384];
    let mut size = 0;
    loop {
        size += stream.read(&mut bytes[size..]).await.unwrap();
        let request = String::from_utf8_lossy(&bytes[..size]);
        let Some(header_end) = request.find("\r\n\r\n") else {
            continue;
        };
        let content_length = request[..header_end]
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap_or(0);
        if size >= header_end + 4 + content_length {
            return request.into_owned();
        }
    }
}

fn request_json(request: &str) -> Value {
    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

fn request_form(request: &str) -> std::collections::HashMap<String, String> {
    url::form_urlencoded::parse(request.split("\r\n\r\n").nth(1).unwrap().as_bytes())
        .into_owned()
        .collect()
}

fn json_response(status: u16, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

fn redirect_response(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    )
}

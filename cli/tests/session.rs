use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use footon::error::Result;
use footon::session::{CredentialStore, StoredSession, resolve_access_token, sign_out};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct MemoryStore {
    session: Arc<Mutex<Option<StoredSession>>>,
    loads: Arc<AtomicUsize>,
}

impl MemoryStore {
    fn with_session(session: StoredSession) -> Self {
        Self {
            session: Arc::new(Mutex::new(Some(session))),
            loads: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl CredentialStore for MemoryStore {
    fn load(&self, _origin: &str) -> Result<Option<StoredSession>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
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

#[tokio::test]
async fn environment_token_wins_without_reading_the_credential_store() {
    let store = MemoryStore::default();

    let token = resolve_access_token(
        "https://footon.dev/api/shares",
        Some("explicit-token"),
        &store,
    )
    .await
    .unwrap();

    assert_eq!(token, "explicit-token");
    assert_eq!(store.loads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn expired_session_refreshes_and_replaces_the_stored_tokens() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let endpoint = format!("{origin}/api/shares");
    let server = tokio::spawn(serve_refresh(listener));
    let store = MemoryStore::with_session(StoredSession::new(
        origin.clone(),
        "test.user@example.com".to_string(),
        "fc_test".to_string(),
        "expired-access".to_string(),
        "old-refresh".to_string(),
        "shares:read shares:write".to_string(),
        format!("{origin}/mcp"),
        1,
    ));

    let token = resolve_access_token(&endpoint, None, &store).await.unwrap();
    let request = server.await.unwrap();
    let saved = store.session.lock().unwrap().clone().unwrap();

    assert_eq!(token, "fresh-access");
    assert_eq!(saved.access_token(), "fresh-access");
    assert_eq!(saved.refresh_token(), "fresh-refresh");
    assert!(saved.expires_at() > chrono::Utc::now().timestamp());
    assert!(request.contains("grant_type=refresh_token"));
    assert!(request.contains("refresh_token=old-refresh"));
    assert!(request.contains("client_id=fc_test"));
}

#[tokio::test]
async fn missing_session_explains_how_to_sign_in() {
    let error = resolve_access_token(
        "https://footon.dev/api/shares",
        None,
        &MemoryStore::default(),
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("footon signin <email>"));
}

#[tokio::test]
async fn signout_revokes_the_refresh_token_then_deletes_the_session() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(serve_revoke(listener));
    let store = MemoryStore::with_session(StoredSession::new(
        origin.clone(),
        "test.user@example.com".to_string(),
        "fc_test".to_string(),
        "access-token".to_string(),
        "refresh-token".to_string(),
        "shares:read shares:write".to_string(),
        format!("{origin}/mcp"),
        i64::MAX,
    ));

    let status = sign_out(&origin, &store).await.unwrap();
    let request = server.await.unwrap();

    assert!(status.signed_out);
    assert_eq!(status.email, "test.user@example.com");
    assert!(store.session.lock().unwrap().is_none());
    assert!(request.starts_with("POST /oauth/revoke HTTP/1.1"));
    assert!(request.contains("token=refresh-token"));
    assert!(request.contains("token_type_hint=refresh_token"));
}

async fn serve_refresh(listener: TcpListener) -> String {
    let (mut stream, _) = listener.accept().await.unwrap();
    let request = read_request(&mut stream).await;
    let body = r#"{"access_token":"fresh-access","token_type":"Bearer","expires_in":3600,"refresh_token":"fresh-refresh","scope":"shares:read shares:write"}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    request
}

async fn serve_revoke(listener: TcpListener) -> String {
    let (mut stream, _) = listener.accept().await.unwrap();
    let request = read_request(&mut stream).await;
    let response = "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
    stream.write_all(response.as_bytes()).await.unwrap();
    request
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

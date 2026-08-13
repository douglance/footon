use std::fs;

use chrono::{TimeZone, Utc};
use footon::cli::app;
use footon::model::{Draft, Message, Report, Role};
use footon::publish::{build_share, send};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn draft_command_writes_only_sanitized_local_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("raw.jsonl");
    let output = temp.path().join("safe.json");
    let raw = r#"{"type":"user","message":{"role":"user","content":"email raw@example.com token=\"abcdefghijklmnopqrstuvwxyz0123456789\""}}"#;
    fs::write(&input, raw).unwrap();

    let mut stdout = Vec::new();
    let exit = app()
        .serve_to(draft_argv(&input, &output), &mut stdout, false)
        .await
        .unwrap();

    assert_eq!(exit, None);
    let draft = fs::read_to_string(&output).unwrap();
    let report = fs::read_to_string(output.with_file_name("safe.json.report.json")).unwrap();
    assert!(!draft.contains("raw@example.com"));
    assert!(!draft.contains("abcdefghijklmnopqrstuvwxyz0123456789"));
    assert!(!report.contains("raw@example.com"));
}

#[tokio::test]
async fn publish_sends_bearer_and_exact_share_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/api/shares", listener.local_addr().unwrap());
    let server = tokio::spawn(capture_request(listener));
    let approved = Utc.with_ymd_and_hms(2026, 8, 13, 1, 2, 3).unwrap();
    let share = build_share(sample_draft(), approved).unwrap();

    let response = send(&endpoint, "test-session-token", &share).await.unwrap();
    let request = server.await.unwrap();

    assert_eq!(response.id, "share_1");
    assert!(request.contains("authorization: Bearer test-session-token"));
    let body = request.split("\r\n\r\n").nth(1).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(body).unwrap()["schemaVersion"],
        "footon.share.v1"
    );
}

fn draft_argv(input: &std::path::Path, output: &std::path::Path) -> Vec<String> {
    [
        "draft",
        input.to_str().unwrap(),
        "--title",
        "Safe thread",
        "--output",
        output.to_str().unwrap(),
        "--json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn sample_draft() -> Draft {
    Draft {
        schema_version: "footon.share.v1".to_string(),
        title: "Safe".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        report: Report::default(),
    }
}

async fn capture_request(listener: TcpListener) -> String {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut bytes = vec![0; 8192];
    let size = stream.read(&mut bytes).await.unwrap();
    let request = String::from_utf8(bytes[..size].to_vec()).unwrap();
    let body = r#"{"id":"share_1","url":"https://footon.dev/s/share_1","createdAt":"2026-08-13T01:02:03Z"}"#;
    let response = format!(
        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    request
}

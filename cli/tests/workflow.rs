use std::fs;

use chrono::{TimeZone, Utc};
use footon::blackout::remote as blackout_remote;
use footon::cli::app;
use footon::draft;
use footon::fetch::fetch_markdown;
use footon::model::{Draft, Message, Report, Role};
use footon::publish::{build_share, send};
use incurs::tool::{ToolCallOptions, ToolCallOutcome};
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
async fn blackout_command_updates_a_sanitized_draft_in_place() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("safe.json");
    fs::write(&path, serde_json::to_vec_pretty(&sample_draft()).unwrap()).unwrap();

    let mut stdout = Vec::new();
    let exit = app()
        .serve_to(
            ["blackout", path.to_str().unwrap(), "1", "hello", "--json"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            &mut stdout,
            false,
        )
        .await
        .unwrap();

    assert_eq!(exit, None);
    let draft = draft::read(&path).unwrap();
    assert_eq!(draft.messages[0].text, "[BLACKED OUT]");
    assert_eq!(draft.report.redactions, 1);
}

#[tokio::test]
async fn blackout_commands_are_typed_incurs_tools_for_code_mode() {
    let catalog = app().tool_catalog();
    let local = catalog.get("blackout").unwrap();
    let remote = catalog.get("blackout-share").unwrap();

    assert_eq!(
        local.input_schema["properties"]["message"]["type"],
        "number"
    );
    assert_eq!(local.input_schema["properties"]["text"]["type"], "string");
    assert_eq!(remote.input_schema["properties"]["share"]["type"], "string");

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("code-mode-draft.json");
    fs::write(&path, serde_json::to_vec_pretty(&sample_draft()).unwrap()).unwrap();
    let outcome = catalog
        .call(
            "blackout",
            [
                (
                    "draft".to_string(),
                    serde_json::json!(path.to_str().unwrap()),
                ),
                ("message".to_string(), serde_json::json!(1)),
                ("text".to_string(), serde_json::json!("hello")),
            ]
            .into_iter()
            .collect(),
            ToolCallOptions::isolated(),
        )
        .await;

    assert!(matches!(outcome, ToolCallOutcome::Ok { .. }));
    assert_eq!(
        draft::read(&path).unwrap().messages[0].text,
        "[BLACKED OUT]"
    );
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
        "footon.share.v2"
    );
}

#[tokio::test]
async fn blackout_share_sends_one_exact_owner_update() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let endpoint = format!("{origin}/api/shares");
    let share = format!("{origin}/s/abcdefghijklmnopqrst");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 8192];
        let size = stream.read(&mut bytes).await.unwrap();
        let request = String::from_utf8(bytes[..size].to_vec()).unwrap();
        let body = r#"{"id":"abcdefghijklmnopqrst","url":"https://footon.dev/s/abcdefghijklmnopqrst","updatedAt":"2026-08-14T01:02:03Z","message":2,"replacement":"[BLACKED OUT]","redactions":4}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        request
    });

    let response = blackout_remote(&endpoint, "owner-token", &share, 2, "private text")
        .await
        .unwrap();
    let request = server.await.unwrap();

    assert_eq!(response.id, "abcdefghijklmnopqrst");
    assert!(request.starts_with("POST /api/shares/abcdefghijklmnopqrst/blackouts HTTP/1.1"));
    assert!(request.contains("authorization: Bearer owner-token"));
    let body = request.split("\r\n\r\n").nth(1).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(body).unwrap(),
        serde_json::json!({ "message": 2, "text": "private text" })
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
        schema_version: "footon.share.v2".to_string(),
        title: "Safe".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        report: Report {
            redactions: 0,
            detectors: vec!["footon-secret-patterns@1".to_string()],
        },
    }
}

#[tokio::test]
async fn fetch_requests_markdown_and_returns_only_markdown() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/s/share_1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 8192];
        let size = stream.read(&mut bytes).await.unwrap();
        let request = String::from_utf8(bytes[..size].to_vec()).unwrap();
        let body = "# Safe\n\n## AGENT\n\nDone\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        request
    });

    let markdown = fetch_markdown(&endpoint).await.unwrap();
    let request = server.await.unwrap();

    assert!(request.contains("accept: text/markdown"));
    assert_eq!(markdown, "# Safe\n\n## AGENT\n\nDone\n");
}

#[tokio::test]
async fn fetch_rejects_unsafe_remote_http() {
    assert!(
        fetch_markdown("http://example.com/s/share_1")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn fetch_rejects_wrong_content_type_and_oversized_body() {
    let wrong_type = serve_once(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: 4\r\n\r\nhtml",
    )
    .await;
    assert!(fetch_markdown(&wrong_type).await.is_err());

    let oversized = serve_once(
        "HTTP/1.1 200 OK\r\nContent-Type: text/markdown\r\nConnection: close\r\nContent-Length: 1000001\r\n\r\n",
    )
    .await;
    assert!(fetch_markdown(&oversized).await.is_err());
}

#[tokio::test]
async fn fetch_rejects_cross_origin_redirects() {
    let redirect = serve_once(
        "HTTP/1.1 302 Found\r\nLocation: https://example.com/s/other\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
    )
    .await;

    assert!(fetch_markdown(&redirect).await.is_err());
}

async fn serve_once(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/s/share_1", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 4096];
        let _size = stream.read(&mut bytes).await.unwrap();
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    endpoint
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

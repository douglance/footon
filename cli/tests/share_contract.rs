use chrono::{TimeZone, Utc};
use footon::model::{Draft, Message, Report, Role};
use footon::publish::{build_share, validate_endpoint};

#[test]
fn publish_adds_approval_and_emits_exact_wire_keys() {
    let draft = Draft {
        schema_version: "footon.share.v2".to_string(),
        title: "Safe thread".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        report: Report {
            redactions: 2,
            detectors: vec!["redact-core@0.9.1".to_string()],
        },
    };
    let approved = Utc.with_ymd_and_hms(2026, 8, 13, 1, 2, 3).unwrap();

    let value = serde_json::to_value(build_share(draft, approved).unwrap()).unwrap();
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        ["approvedAt", "messages", "report", "schemaVersion", "title"]
    );
    assert_eq!(value["approvedAt"], "2026-08-13T01:02:03Z");
    assert_eq!(value["messages"][0]["text"], "hello");
}

#[test]
fn endpoint_requires_https_except_loopback_tests() {
    assert!(validate_endpoint("https://footon.dev/api/shares").is_ok());
    assert!(validate_endpoint("http://127.0.0.1:8787/api/shares").is_ok());
    assert!(validate_endpoint("http://localhost:8787/api/shares").is_ok());
    assert!(validate_endpoint("http://example.com/api/shares").is_err());
}

#[test]
fn rejects_unsanitized_or_oversized_drafts() {
    let unsafe_draft = Draft {
        schema_version: "other".to_string(),
        title: "bad".to_string(),
        messages: vec![Message::new(Role::User, "hello")],
        report: Report::default(),
    };
    assert!(build_share(unsafe_draft, Utc::now()).is_err());

    let empty = Draft {
        schema_version: "footon.share.v2".to_string(),
        title: "empty".to_string(),
        messages: vec![],
        report: Report::default(),
    };
    assert!(build_share(empty, Utc::now()).is_err());
}

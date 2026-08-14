use chrono::{TimeZone, Utc};
use footon_core::blackout::{BLACKOUT_TEXT, blackout};
use footon_core::markdown::serialize_share;
use footon_core::model::{
    Draft, Message, Report, Role, SCHEMA_VERSION_V1, SCHEMA_VERSION_V2, Share,
};
use footon_core::parse::{Source, parse_jsonl};
use footon_core::safety::sanitize_messages;
use footon_core::validate::{build_share, validate_share};

#[test]
fn validates_v1_reads_but_rejects_v1_write_upgrade() {
    let approved_at = Utc.with_ymd_and_hms(2026, 8, 13, 1, 2, 3).unwrap();
    let share = Share {
        schema_version: SCHEMA_VERSION_V1.to_string(),
        title: "Safe".to_string(),
        approved_at,
        messages: vec![Message::new(Role::Assistant, "done")],
        report: report(),
    };

    validate_share(&share).unwrap();

    let draft = Draft {
        schema_version: SCHEMA_VERSION_V1.to_string(),
        title: "Safe".to_string(),
        messages: share.messages,
        report: report(),
    };
    assert!(build_share(draft, approved_at).is_err());
}

#[test]
fn validates_v2_tool_and_file_activity_shape() {
    let share = Share {
        schema_version: SCHEMA_VERSION_V2.to_string(),
        title: "Safe".to_string(),
        approved_at: Utc::now(),
        messages: vec![
            Message::new(Role::Tool, "functions.exec cargo 2 arguments"),
            Message::new(Role::File, "update lib.rs"),
        ],
        report: report(),
    };

    validate_share(&share).unwrap();

    let mut unsafe_share = share;
    unsafe_share.messages[0].text = "functions.exec token=abc123456789".to_string();
    assert!(validate_share(&unsafe_share).is_err());
}

#[test]
fn safety_scanner_matches_edge_secret_and_pii_families() {
    let text = concat!(
        "email doug@example.com ",
        "password=supersecret ",
        "postgres://user:pass@example.com/db ",
        "github_pat_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789________"
    );
    let result = sanitize_messages(&[Message::new(Role::User, text)]).unwrap();
    let clean = &result.messages[0].text;

    assert!(!clean.contains("doug@example.com"));
    assert!(!clean.contains("supersecret"));
    assert!(!clean.contains("postgres://"));
    assert!(!clean.contains("github_pat_"));
    assert!(result.report.redactions >= 4);
}

#[test]
fn filters_injected_blocks_and_compacts_adjacent_assistant_messages() {
    let input = concat!(
        r##"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>\nsecret\n</recommended_plugins>\n\nReal request"}]}}"##,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"first"}]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"second"}]}}"#,
    );
    let parsed = parse_jsonl(input, Source::Codex).unwrap();
    let sanitized = sanitize_messages(&parsed).unwrap();

    assert_eq!(
        sanitized.messages,
        vec![
            Message::new(Role::User, "Real request"),
            Message::new(Role::Assistant, "first\n\nsecond"),
        ]
    );
}

#[test]
fn serializes_markdown_with_stable_role_headings() {
    let share = build_share(
        Draft {
            schema_version: SCHEMA_VERSION_V2.to_string(),
            title: "Safe Thread".to_string(),
            messages: vec![
                Message::new(Role::User, "hello"),
                Message::new(Role::Assistant, "done"),
                Message::new(Role::Tool, "functions.exec cargo 1 argument"),
                Message::new(Role::File, "update lib.rs"),
            ],
            report: report(),
        },
        Utc.with_ymd_and_hms(2026, 8, 13, 1, 2, 3).unwrap(),
    )
    .unwrap();

    assert_eq!(
        serialize_share(&share),
        "# Safe Thread\n\n## USER\n\nhello\n\n## AGENT\n\ndone\n\n### TOOL\n\nfunctions.exec cargo 1 argument\n\n### FILE\n\nupdate lib.rs"
    );
}

fn report() -> Report {
    Report {
        redactions: 0,
        detectors: vec!["footon-secret-patterns@1".to_string()],
    }
}

#[test]
fn manually_blacks_out_one_exact_prose_match() {
    let mut messages = vec![
        Message::new(Role::User, "keep private-code-123 private"),
        Message::new(Role::Assistant, "done"),
    ];
    let mut report = report();

    let outcome = blackout(&mut messages, &mut report, 1, "private-code-123").unwrap();

    assert_eq!(messages[0].text, format!("keep {BLACKOUT_TEXT} private"));
    assert_eq!(outcome.message, 1);
    assert_eq!(outcome.replacement, BLACKOUT_TEXT);
    assert_eq!(report.redactions, 1);
    assert!(
        report
            .detectors
            .iter()
            .any(|detector| detector == "footon-manual-blackout@1")
    );
}

#[test]
fn manual_blackout_rejects_ambiguous_or_non_prose_targets() {
    let mut repeated = vec![Message::new(Role::User, "same same")];
    let mut repeated_report = report();
    assert!(blackout(&mut repeated, &mut repeated_report, 1, "same").is_err());

    let mut activity = vec![Message::new(Role::Tool, "exec cargo 1 argument")];
    let mut activity_report = report();
    assert!(blackout(&mut activity, &mut activity_report, 1, "cargo").is_err());

    let mut prose = vec![Message::new(Role::Assistant, "safe")];
    let mut prose_report = report();
    assert!(blackout(&mut prose, &mut prose_report, 0, "safe").is_err());
    assert!(blackout(&mut prose, &mut prose_report, 1, "missing").is_err());
}

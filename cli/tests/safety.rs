use footon::model::{Message, Role};
use footon::sanitize::sanitize_messages;

#[test]
fn redacts_pii_secrets_paths_and_bearer_values() {
    let raw = concat!(
        "Email me at doug@example.com. Read /Users/doug/private/key.pem. ",
        "Authorization: Bearer abcdEFGHijklMNOPqrstUVWXyz0123456789. ",
        "token = 'cf_live_abcdefghijklmnopqrstuvwxyz012345'."
    );
    let messages = vec![Message::new(Role::User, raw)];

    let result = sanitize_messages(&messages).expect("sanitize");
    let text = &result.messages[0].text;

    for forbidden in [
        "doug@example.com",
        "/Users/doug/private/key.pem",
        "abcdEFGHijklMNOPqrstUVWXyz0123456789",
        "cf_live_abcdefghijklmnopqrstuvwxyz012345",
    ] {
        assert!(!text.contains(forbidden), "leaked {forbidden}");
    }
    assert!(result.report.redactions >= 4);
    assert!(
        result
            .report
            .detectors
            .contains(&"redact-core@0.9.1".to_string())
    );
    assert!(
        result
            .report
            .detectors
            .contains(&"footon-secret-patterns@1".to_string())
    );
}

#[test]
fn placeholders_are_deterministic_without_disclosing_values() {
    let messages = vec![Message::new(
        Role::Assistant,
        "key=\"sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN0123456789\"",
    )];

    let first = sanitize_messages(&messages).unwrap();
    let second = sanitize_messages(&messages).unwrap();
    assert_eq!(first.messages, second.messages);
    assert_eq!(first.report, second.report);
    assert!(first.messages[0].text.contains("[REDACTED:"));
}

#[test]
fn removes_embedded_system_reminders_but_preserves_code_fences() {
    let messages = vec![Message::new(
        Role::User,
        "before <system-reminder>private policy</system-reminder>\n```sh\necho safe\n``` after",
    )];

    let result = sanitize_messages(&messages).unwrap();
    let text = &result.messages[0].text;
    assert!(!text.contains("private policy"));
    assert!(text.contains("```sh\necho safe\n```"));
}

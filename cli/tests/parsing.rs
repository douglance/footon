use footon::model::{Message, Role};
use footon::parse::{Source, parse_jsonl};

#[test]
fn claude_keeps_only_user_and_assistant_text() {
    let input = concat!(
        r#"{"type":"system","message":{"content":"hidden"}}"#,
        "\n",
        r#"{"type":"user","cwd":"/secret","message":{"role":"user","content":[{"type":"text","text":"hello"},{"type":"tool_result","content":"secret output"}]}}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"```rust\nfn ok() {}\n```"},{"type":"tool_use","input":{"token":"raw"}}]}}"#,
    );

    let messages = parse_jsonl(input, Source::Claude).expect("parse Claude JSONL");
    assert_eq!(
        messages,
        vec![
            Message::new(Role::User, "hello"),
            Message::new(Role::Assistant, "```rust\nfn ok() {}\n```"),
        ]
    );
}

#[test]
fn codex_keeps_response_messages_and_drops_privileged_roles() {
    let input = concat!(
        r#"{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"hidden"}]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"reasoning","summary":"private"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"question"}]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"function_call","arguments":"{\"cmd\":\"cat secret\"}"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"answer"}]}}"#,
    );

    let messages = parse_jsonl(input, Source::Codex).expect("parse Codex JSONL");
    assert_eq!(
        messages,
        vec![
            Message::new(Role::User, "question"),
            Message::new(Role::Assistant, "answer"),
        ]
    );
}

#[test]
fn auto_detects_both_sources_and_ignores_invalid_lines() {
    let claude = "not-json\n{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}";
    let codex = "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":\"yo\"}}";

    assert_eq!(parse_jsonl(claude, Source::Auto).unwrap().len(), 1);
    assert_eq!(parse_jsonl(codex, Source::Auto).unwrap().len(), 1);
}

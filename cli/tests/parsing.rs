use footon::model::{Message, Role};
use footon::parse::{Source, parse_jsonl};

#[test]
fn claude_keeps_prose_and_neutered_activity() {
    let input = concat!(
        r#"{"type":"system","message":{"content":"hidden"}}"#,
        "\n",
        r#"{"type":"user","cwd":"/secret","message":{"role":"user","content":[{"type":"text","text":"hello"},{"type":"tool_result","content":"secret output"}]}}"#,
        "\n",
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"```rust\nfn ok() {}\n```"},{"type":"tool_use","name":"Bash","input":{"token":"raw"}}]}}"#,
    );

    let messages = parse_jsonl(input, Source::Claude).expect("parse Claude JSONL");
    assert_eq!(
        messages,
        vec![
            Message::new(Role::User, "hello"),
            Message::new(Role::Assistant, "```rust\nfn ok() {}\n```"),
            Message::new(Role::Tool, "Bash"),
        ]
    );
}

#[test]
fn codex_keeps_messages_and_safe_tool_names() {
    let input = concat!(
        r#"{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"hidden"}]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"reasoning","summary":"private"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"question"}]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"function_call","name":"functions.exec","arguments":"{\"cmd\":\"npm run check --token secret\",\"patch\":\"*** Update File: /Users/private/project/src/viewer.ts\"}"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"const patch = \"*** Begin Patch\\n*** Add File: /Users/private/project/src/history.ts\\n*** End Patch\";"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"answer"}]}}"#,
    );

    let messages = parse_jsonl(input, Source::Codex).expect("parse Codex JSONL");
    assert_eq!(
        messages,
        vec![
            Message::new(Role::User, "question"),
            Message::new(Role::Tool, "functions.exec npm 4 arguments"),
            Message::new(Role::File, "update viewer.ts"),
            Message::new(Role::Tool, "exec"),
            Message::new(Role::File, "add history.ts"),
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

#[test]
fn tool_summaries_never_keep_arbitrary_arguments() {
    let input = concat!(
        r#"{"type":"response_item","payload":{"type":"function_call","name":"functions.exec","arguments":"{\"cmd\":\"echo private-value\"}"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"function_call","name":"functions.exec","arguments":"{\"cmd\":\"git status --short\"}"}}"#,
    );

    assert_eq!(
        parse_jsonl(input, Source::Codex).unwrap(),
        vec![
            Message::new(Role::Tool, "functions.exec echo 1 argument"),
            Message::new(Role::Tool, "functions.exec git 2 arguments"),
        ]
    );
}

#[test]
fn extracts_safe_intent_from_typed_execution_wrapper() {
    let input = r#"{"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"const r = await tools.exec_command({cmd:\"apoc execution command --purpose 'Check it.' -- npm run check --token private\",workdir:\"/Users/private/project\"});"}}"#;

    assert_eq!(
        parse_jsonl(input, Source::Codex).unwrap(),
        vec![Message::new(Role::Tool, "exec npm 4 arguments")]
    );
}

#[test]
fn summarizes_every_program_with_the_same_rule() {
    let input = concat!(
        r#"{"type":"response_item","payload":{"type":"function_call","name":"exec","arguments":"{\"cmd\":\"apoc code run --code private\"}"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"function_call","name":"exec","arguments":"{\"cmd\":\"for item in private\"}"}}"#,
    );

    assert_eq!(
        parse_jsonl(input, Source::Codex).unwrap(),
        vec![
            Message::new(Role::Tool, "exec apoc 4 arguments"),
            Message::new(Role::Tool, "exec for 3 arguments"),
        ]
    );
}

use std::str::FromStr;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::model::{Message, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Auto,
    Claude,
    Codex,
}

impl FromStr for Source {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            _ => Err(Error::Source(value.to_string())),
        }
    }
}

/// Extract only user and assistant prose from Claude Code or Codex JSONL.
///
/// # Errors
///
/// Returns an error when the source is unsupported or no conversation prose exists.
pub fn parse_jsonl(input: &str, source: Source) -> Result<Vec<Message>> {
    let values = input
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let resolved = resolve_source(&values, source);
    let messages = values
        .iter()
        .filter_map(|value| parse_record(value, resolved))
        .collect::<Vec<_>>();
    if messages.is_empty() {
        return Err(Error::NoMessages);
    }
    Ok(messages)
}

fn resolve_source(values: &[Value], source: Source) -> Source {
    if source != Source::Auto {
        return source;
    }
    if values.iter().any(|value| value.get("payload").is_some()) {
        Source::Codex
    } else {
        Source::Claude
    }
}

fn parse_record(value: &Value, source: Source) -> Option<Message> {
    match source {
        Source::Claude => parse_claude(value),
        Source::Codex => parse_codex(value),
        Source::Auto => None,
    }
}

fn parse_claude(value: &Value) -> Option<Message> {
    let role = parse_role(value.get("type")?.as_str()?)?;
    let message = value.get("message")?;
    let declared = message.get("role").and_then(Value::as_str);
    if declared
        .and_then(parse_role)
        .is_some_and(|item| item != role)
    {
        return None;
    }
    text_message(role, message.get("content")?)
}

fn parse_codex(value: &Value) -> Option<Message> {
    if value.get("type")?.as_str()? != "response_item" {
        return None;
    }
    let payload = value.get("payload")?;
    if payload.get("type")?.as_str()? != "message" {
        return None;
    }
    let role = parse_role(payload.get("role")?.as_str()?)?;
    text_message(role, payload.get("content")?)
}

fn text_message(role: Role, content: &Value) -> Option<Message> {
    let text = extract_text(content);
    (!text.trim().is_empty()).then(|| Message::new(role, text))
}

fn extract_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(text_block)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn text_block(block: &Value) -> Option<String> {
    match block.get("type").and_then(Value::as_str) {
        Some("text" | "input_text" | "output_text") => {
            block.get("text").and_then(Value::as_str).map(str::to_owned)
        }
        _ => None,
    }
}

fn parse_role(role: &str) -> Option<Role> {
    match role {
        "user" => Some(Role::User),
        "assistant" => Some(Role::Assistant),
        _ => None,
    }
}

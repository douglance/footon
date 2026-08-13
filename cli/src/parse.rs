use std::str::FromStr;

use serde_json::Value;

use crate::activity::tool_activity;
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

/// Extract conversation prose and neutered activity summaries from Claude or Codex JSONL.
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
        .flat_map(|value| parse_record(value, resolved))
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

fn parse_record(value: &Value, source: Source) -> Vec<Message> {
    match source {
        Source::Claude => parse_claude(value),
        Source::Codex => parse_codex(value),
        Source::Auto => Vec::new(),
    }
}

fn parse_claude(value: &Value) -> Vec<Message> {
    let Some(role) = value
        .get("type")
        .and_then(Value::as_str)
        .and_then(parse_role)
    else {
        return Vec::new();
    };
    let Some(message) = value.get("message") else {
        return Vec::new();
    };
    let declared = message.get("role").and_then(Value::as_str);
    if declared
        .and_then(parse_role)
        .is_some_and(|item| item != role)
    {
        return Vec::new();
    }
    claude_content(role, message.get("content"))
}

fn parse_codex(value: &Value) -> Vec<Message> {
    if value.get("type").and_then(Value::as_str) != Some("response_item") {
        return Vec::new();
    }
    let Some(payload) = value.get("payload") else {
        return Vec::new();
    };
    if payload.get("type").and_then(Value::as_str) == Some("message") {
        return codex_message(payload).into_iter().collect();
    }
    tool_activity(payload)
}

fn codex_message(payload: &Value) -> Option<Message> {
    let role = parse_role(payload.get("role")?.as_str()?)?;
    text_message(role, payload.get("content")?)
}

fn claude_content(role: Role, content: Option<&Value>) -> Vec<Message> {
    let Some(content) = content else {
        return Vec::new();
    };
    let Value::Array(blocks) = content else {
        return text_message(role, content).into_iter().collect();
    };
    blocks
        .iter()
        .flat_map(|block| match block.get("type").and_then(Value::as_str) {
            Some("tool_use") => tool_activity(block),
            _ => text_block(block)
                .map(|text| Message::new(role, text))
                .into_iter()
                .collect(),
        })
        .collect()
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
    if let Value::String(text) = block {
        return Some(text.clone());
    }
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

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

use crate::model::{Message, Role};

static CMD_LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^|[,{])\s*cmd\s*:\s*(\"(?:\\.|[^\"\\])*\")"#)
        .expect("static command literal regex")
});

pub fn tool_activity(value: &Value) -> Vec<Message> {
    let Some(name) = value
        .get("name")
        .and_then(Value::as_str)
        .and_then(safe_tool_name)
    else {
        return Vec::new();
    };
    let summary = activity_summary(name, value).unwrap_or_else(|| name.to_owned());
    let mut items = vec![Message::new(Role::Tool, summary)];
    for input in [value.get("arguments"), value.get("input")]
        .into_iter()
        .flatten()
    {
        collect_file_changes(input, &mut items);
    }
    items
}

fn activity_summary(name: &str, value: &Value) -> Option<String> {
    let input = value.get("arguments").or_else(|| value.get("input"))?;
    let parsed = input
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok());
    let input = parsed.as_ref().unwrap_or(input);
    let command = find_command(input)?;
    let command = command
        .rsplit_once(" -- ")
        .map_or(command.as_str(), |(_, inner)| inner);
    summarize_command(command).map(|summary| format!("{name} {summary}"))
}

fn summarize_command(command: &str) -> Option<String> {
    let mut words = command.split_whitespace();
    let program = words.next()?.trim_matches(['\'', '"']);
    let program = program.rsplit(['/', '\\']).next()?;
    if !safe_word(program) {
        return None;
    }
    let arguments = words.count();
    match arguments {
        0 => Some(program.to_owned()),
        1 => Some(format!("{program} 1 argument")),
        count => Some(format!("{program} {count} arguments")),
    }
}

fn find_command(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => embedded_command(text),
        Value::Object(values) => ["cmd", "command"]
            .into_iter()
            .find_map(|key| values.get(key).and_then(Value::as_str).map(str::to_owned))
            .or_else(|| values.values().find_map(find_command)),
        Value::Array(values) => values.iter().find_map(find_command),
        _ => None,
    }
}

fn embedded_command(value: &str) -> Option<String> {
    let quoted = CMD_LITERAL.captures(value)?.get(1)?.as_str();
    serde_json::from_str(quoted).ok()
}

fn safe_word(value: &str) -> bool {
    value.len() <= 48
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        && !value.contains(['=', '\\', '/'])
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._:@-".contains(character))
}

fn collect_file_changes(value: &Value, output: &mut Vec<Message>) {
    match value {
        Value::String(text) => collect_string_changes(text, output),
        Value::Array(values) => values
            .iter()
            .for_each(|item| collect_file_changes(item, output)),
        Value::Object(values) => values
            .values()
            .for_each(|item| collect_file_changes(item, output)),
        _ => {}
    }
}

fn collect_string_changes(text: &str, output: &mut Vec<Message>) {
    if let Ok(nested) = serde_json::from_str::<Value>(text) {
        collect_file_changes(&nested, output);
    }
    let normalized = text.replace("\\r\\n", "\n").replace("\\n", "\n");
    for line in normalized.lines() {
        if let Some(change) = safe_file_change(line)
            && !output.contains(&change)
        {
            output.push(change);
        }
    }
}

fn safe_file_change(line: &str) -> Option<Message> {
    let clean = line.trim().trim_start_matches('*').trim();
    let (operation, path) = ["Add File:", "Update File:", "Delete File:"]
        .into_iter()
        .find_map(|prefix| {
            clean
                .strip_prefix(prefix)
                .map(|path| (&prefix[..prefix.len() - 6], path))
        })?;
    let file = path.trim().rsplit(['/', '\\']).next()?;
    valid_filename(file)
        .then(|| Message::new(Role::File, format!("{} {}", operation.to_lowercase(), file)))
}

fn valid_filename(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && value.len() <= 120
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
}

fn safe_tool_name(value: &str) -> Option<&str> {
    (value.len() <= 80
        && !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_.:-".contains(character)))
    .then_some(value)
}

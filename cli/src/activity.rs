use serde_json::Value;

use crate::model::{Message, Role};

pub fn tool_activity(value: &Value) -> Vec<Message> {
    let Some(name) = value
        .get("name")
        .and_then(Value::as_str)
        .and_then(safe_tool_name)
    else {
        return Vec::new();
    };
    let mut items = vec![Message::new(Role::Tool, name)];
    for input in [value.get("arguments"), value.get("input")]
        .into_iter()
        .flatten()
    {
        collect_file_changes(input, &mut items);
    }
    items
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

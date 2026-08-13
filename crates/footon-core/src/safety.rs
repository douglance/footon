use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Message, Report, Role};
use crate::scanner::redact;

static PRIVILEGED_TAGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(?:system-reminder|system|developer)[^>]*>.*?</(?:system-reminder|system|developer)>")
        .expect("static privileged tag regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedMessages {
    pub messages: Vec<Message>,
    pub report: Report,
}

/// Redact sensitive text, drop injected prompt blocks, and compact assistant chunks.
///
/// # Errors
///
/// Returns an error if a scanner span cannot be applied safely.
pub fn sanitize_messages(messages: &[Message]) -> crate::error::Result<SanitizedMessages> {
    let mut redactions = 0_usize;
    let mut sanitized = Vec::with_capacity(messages.len());
    for message in messages {
        let filtered = filter_message_text(message);
        let (filtered, tags) = strip_privileged_tags(&filtered);
        let (text, count) = redact(&filtered)?;
        redactions += tags + count;
        sanitized.push(Message::new(message.role, text));
    }
    Ok(SanitizedMessages {
        messages: compact_messages(&sanitized),
        report: Report {
            redactions,
            detectors: vec![
                "footon-secret-patterns@1".to_string(),
                "redact-core@0.9.1".to_string(),
            ],
        },
    })
}

fn strip_privileged_tags(text: &str) -> (String, usize) {
    let count = PRIVILEGED_TAGS.find_iter(text).count();
    (
        PRIVILEGED_TAGS
            .replace_all(text, "[REMOVED:PRIVILEGED]")
            .into_owned(),
        count,
    )
}

#[must_use]
pub fn compact_messages(messages: &[Message]) -> Vec<Message> {
    let mut compacted: Vec<Message> = Vec::new();
    let mut seen_assistant = Vec::<String>::new();
    for message in messages {
        let text = filter_message_text(message);
        if text.trim().is_empty() {
            continue;
        }
        if message.role == Role::Assistant
            && compacted
                .last()
                .is_some_and(|previous| previous.role == Role::Assistant)
        {
            if !seen_assistant.iter().any(|seen| seen == &text)
                && let Some(previous) = compacted.last_mut()
            {
                previous.text.push_str("\n\n");
                previous.text.push_str(&text);
            }
            seen_assistant.push(text);
            continue;
        }
        compacted.push(Message::new(message.role, text.clone()));
        seen_assistant = if message.role == Role::Assistant {
            vec![text]
        } else {
            Vec::new()
        };
    }
    compacted
}

#[must_use]
pub fn filter_message_text(message: &Message) -> String {
    if message.role == Role::User {
        filter_injected_blocks(&message.text)
    } else {
        message.text.clone()
    }
}

#[must_use]
pub fn filter_injected_blocks(text: &str) -> String {
    let mut filtered = text.to_string();
    for heading in [
        "# AGENTS.md instructions",
        "# [DOMAIN_NAME] instructions",
        "# Domain instructions",
        "# domain instructions",
    ] {
        filtered = strip_instruction_blocks(&filtered, heading);
    }
    for tag in [
        "recommended_plugins",
        "environment_context",
        "codex_internal_context",
    ] {
        filtered = strip_tagged_blocks(&filtered, tag);
    }
    if filtered == text {
        text.to_string()
    } else {
        collapse_blank_lines(filtered.trim())
    }
}

fn strip_instruction_blocks(text: &str, heading: &str) -> String {
    let mut output = text.to_string();
    loop {
        let Some(start) = output.find(heading) else {
            return output;
        };
        let after_heading = start + heading.len();
        let rest = &output[after_heading..];
        let Some(open_start) = rest.find("<INSTRUCTIONS>") else {
            return output;
        };
        if rest[..open_start].trim().is_empty()
            && let Some(close_end) = rest.find("</INSTRUCTIONS>")
        {
            let end = after_heading + close_end + "</INSTRUCTIONS>".len();
            output.replace_range(start..end, "");
        } else {
            return output;
        }
    }
}

fn strip_tagged_blocks(text: &str, tag: &str) -> String {
    let mut output = text.to_string();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    loop {
        let Some(start) = output.find(&open) else {
            return output;
        };
        let Some(open_end) = output[start..].find('>') else {
            return output;
        };
        let content_start = start + open_end + 1;
        let Some(close_start_rel) = output[content_start..].find(&close) else {
            return output;
        };
        let end = content_start + close_start_rel + close.len();
        output.replace_range(start..end, "");
    }
}

fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::new();
    let mut blank_count = 0_usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::model::Role;

    use super::*;

    #[test]
    fn strips_agent_instructions_and_plugins_from_user_text() {
        let message = Message {
            role: Role::User,
            text: "hello\n# AGENTS.md instructions\n<INSTRUCTIONS>\nsecret\n</INSTRUCTIONS>\n<recommended_plugins>x</recommended_plugins>\nworld".to_string(),
        };
        let filtered = filter_message_text(&message);
        assert!(!filtered.contains("secret"));
        assert!(!filtered.contains("recommended_plugins"));
        assert!(filtered.contains("hello"));
        assert!(filtered.contains("world"));
    }
}

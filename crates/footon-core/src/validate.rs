use crate::error::{Error, Result};
use crate::model::{
    Draft, MAX_MESSAGE_TEXT_CHARS, MAX_MESSAGES, MAX_TEXT_BYTES, MAX_TITLE_CHARS, Message, Report,
    Role, SCHEMA_VERSION_V1, SCHEMA_VERSION_V2, Share,
};
use crate::scanner::contains_sensitive_text;

/// Validate a sanitized draft before it becomes an approved share.
///
/// # Errors
///
/// Returns an error when the schema, title, message count, activity shape, or text safety is invalid.
pub fn validate_draft(draft: &Draft) -> Result<()> {
    validate_common(
        &draft.schema_version,
        &draft.title,
        &draft.messages,
        &draft.report,
        None,
    )
}

/// Validate a read-side share document.
///
/// # Errors
///
/// Returns an error when the share violates the v1 or v2 contract.
pub fn validate_share(share: &Share) -> Result<()> {
    validate_common(
        &share.schema_version,
        &share.title,
        &share.messages,
        &share.report,
        Some(&share.approved_at),
    )
}

/// Validate and stamp the explicit approval time.
///
/// # Errors
///
/// Returns an error when the draft is not a valid v2 write document.
pub fn build_share(draft: Draft, approved_at: chrono::DateTime<chrono::Utc>) -> Result<Share> {
    validate_draft(&draft)?;
    if draft.schema_version != SCHEMA_VERSION_V2 {
        return Err(Error::Share("draft is not footon.share.v2".to_string()));
    }
    Ok(Share {
        schema_version: draft.schema_version,
        title: draft.title,
        approved_at,
        messages: draft.messages,
        report: draft.report,
    })
}

fn validate_common(
    schema_version: &str,
    title: &str,
    messages: &[Message],
    report: &Report,
    approved_at: Option<&chrono::DateTime<chrono::Utc>>,
) -> Result<()> {
    validate_schema(schema_version)?;
    validate_title(title)?;
    validate_messages(schema_version, messages)?;
    validate_report(report)?;
    if approved_at.is_some_and(|value| value.timestamp() <= 0) {
        return Err(Error::Share("approvedAt is invalid".to_string()));
    }
    if text_bytes(title)
        + messages
            .iter()
            .map(|message| text_bytes(&message.text))
            .sum::<usize>()
        > MAX_TEXT_BYTES
    {
        return Err(Error::Share("share text exceeds 1 MB".to_string()));
    }
    if contains_sensitive_text(title) {
        return Err(Error::Share("possible sensitive title remains".to_string()));
    }
    Ok(())
}

fn validate_schema(value: &str) -> Result<()> {
    if matches!(value, SCHEMA_VERSION_V1 | SCHEMA_VERSION_V2) {
        return Ok(());
    }
    Err(Error::Share("unsupported schema".to_string()))
}

fn validate_title(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > MAX_TITLE_CHARS {
        return Err(Error::Share("title is invalid".to_string()));
    }
    Ok(())
}

fn validate_messages(schema_version: &str, messages: &[Message]) -> Result<()> {
    if !(1..=MAX_MESSAGES).contains(&messages.len()) {
        return Err(Error::Share(
            "share must contain 1 to 2000 transcript items".to_string(),
        ));
    }
    for message in messages {
        validate_message(schema_version, message)?;
    }
    Ok(())
}

fn validate_message(schema_version: &str, message: &Message) -> Result<()> {
    if schema_version == SCHEMA_VERSION_V1 && matches!(message.role, Role::Tool | Role::File) {
        return Err(Error::Share(
            "footon.share.v1 supports only user and assistant roles".to_string(),
        ));
    }
    if message.text.is_empty() || message.text.chars().count() > MAX_MESSAGE_TEXT_CHARS {
        return Err(Error::Share("message text is invalid".to_string()));
    }
    validate_activity(message)?;
    if contains_sensitive_text(&message.text) {
        return Err(Error::Share("possible sensitive text remains".to_string()));
    }
    Ok(())
}

fn validate_activity(message: &Message) -> Result<()> {
    match message.role {
        Role::Tool if valid_tool_activity(&message.text) => Ok(()),
        Role::Tool => Err(Error::Share("invalid tool activity summary".to_string())),
        Role::File if valid_file_activity(&message.text) => Ok(()),
        Role::File => Err(Error::Share("invalid file activity summary".to_string())),
        Role::User | Role::Assistant => Ok(()),
    }
}

fn valid_tool_activity(text: &str) -> bool {
    let words = text.split(' ').collect::<Vec<_>>();
    (1..=8).contains(&words.len()) && words.iter().all(|word| valid_activity_word(word))
}

fn valid_file_activity(text: &str) -> bool {
    let Some((operation, filename)) = text.split_once(' ') else {
        return false;
    };
    matches!(operation, "add" | "update" | "delete")
        && !filename.is_empty()
        && filename.len() <= 120
        && filename
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        && filename
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
}

fn valid_activity_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
        && word
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_.:@-".contains(character))
}

fn validate_report(report: &Report) -> Result<()> {
    if report.detectors.is_empty() || report.detectors.len() > 16 {
        return Err(Error::Share("invalid sanitization report".to_string()));
    }
    if report.detectors.iter().any(|item| item.len() > 80) {
        return Err(Error::Share("invalid detector name".to_string()));
    }
    Ok(())
}

fn text_bytes(value: &str) -> usize {
    value.len()
}

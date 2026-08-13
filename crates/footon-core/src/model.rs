use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCHEMA_VERSION: &str = "footon.share.v2";
pub const LEGACY_SCHEMA_VERSION: &str = "footon.share.v1";
pub const SCHEMA_VERSION_V1: &str = LEGACY_SCHEMA_VERSION;
pub const SCHEMA_VERSION_V2: &str = SCHEMA_VERSION;
pub const MAX_MESSAGES: usize = 2_000;
pub const MAX_TEXT_BYTES: usize = 1_000_000;
pub const MAX_MESSAGE_TEXT_CHARS: usize = 100_000;
pub const MAX_TITLE_CHARS: usize = 160;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    Tool,
    File,
}

impl Role {
    #[must_use]
    pub const fn markdown_heading(self) -> &'static str {
        match self {
            Self::User => "## USER",
            Self::Assistant => "## AGENT",
            Self::Tool => "### TOOL",
            Self::File => "### FILE",
        }
    }

    #[must_use]
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::File => "file",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::User => "USER",
            Self::Assistant => "AGENT",
            Self::Tool => "TOOL",
            Self::File => "FILE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

impl Message {
    #[must_use]
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub redactions: usize,
    pub detectors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub schema_version: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub report: Report,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Share {
    pub schema_version: String,
    pub title: String,
    pub approved_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub report: Report,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareDocument {
    pub schema_version: String,
    pub title: String,
    pub approved_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub report: Report,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRecord {
    pub id: String,
    pub owner_id: String,
    pub title: String,
    pub document: ShareDocument,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("share is not footon.share.v2")]
    Schema,
    #[error("share must contain 1 to 2000 transcript items")]
    MessageCount,
    #[error("message text exceeds 1 MB")]
    TextBytes,
    #[error("title is required")]
    Title,
}

/// Validate newly written shares. Legacy documents remain readable but cannot
/// be newly written by the Rust path.
///
/// # Errors
///
/// Returns a typed validation error when the share is not a bounded v2 document.
pub fn validate_share(share: &Share) -> Result<(), ValidationError> {
    if share.schema_version != SCHEMA_VERSION {
        return Err(ValidationError::Schema);
    }
    validate_title_messages(&share.title, &share.messages)
}

fn validate_title_messages(title: &str, messages: &[Message]) -> Result<(), ValidationError> {
    if title.trim().is_empty() {
        return Err(ValidationError::Title);
    }
    if !(1..=MAX_MESSAGES).contains(&messages.len()) {
        return Err(ValidationError::MessageCount);
    }
    let bytes = messages
        .iter()
        .map(|message| message.text.len())
        .sum::<usize>();
    if bytes > MAX_TEXT_BYTES {
        return Err(ValidationError::TextBytes);
    }
    Ok(())
}

impl TryFrom<Share> for ShareDocument {
    type Error = ValidationError;

    fn try_from(value: Share) -> Result<Self, Self::Error> {
        validate_share(&value)?;
        Ok(Self {
            schema_version: value.schema_version,
            title: value.title,
            approved_at: value.approved_at,
            messages: value.messages,
            report: value.report,
        })
    }
}

impl ShareDocument {
    /// Parse stored v1 or v2 share JSON from D1.
    ///
    /// # Errors
    ///
    /// Returns a JSON error for malformed documents and a validation error for
    /// unsupported shapes.
    pub fn from_json(value: &str) -> Result<Self, DocumentError> {
        let raw = serde_json::from_str::<Value>(value)?;
        let schema_version = raw
            .get("schemaVersion")
            .or_else(|| raw.get("schema_version"))
            .and_then(Value::as_str)
            .unwrap_or(LEGACY_SCHEMA_VERSION)
            .to_string();
        let title = raw
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled share")
            .to_string();
        let messages = raw
            .get("messages")
            .and_then(Value::as_array)
            .ok_or(DocumentError::Validation(ValidationError::MessageCount))?
            .iter()
            .map(parse_message)
            .collect::<Result<Vec<_>, _>>()?;
        let report = raw
            .get("report")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let approved_at = raw
            .get("approvedAt")
            .or_else(|| raw.get("approved_at"))
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<DateTime<Utc>>().ok())
            .unwrap_or_else(Utc::now);
        validate_title_messages(&title, &messages)?;
        Ok(Self {
            schema_version,
            title,
            approved_at,
            messages,
            report,
        })
    }
}

fn parse_message(value: &Value) -> Result<Message, DocumentError> {
    let role = value
        .get("role")
        .and_then(Value::as_str)
        .ok_or(DocumentError::Validation(ValidationError::MessageCount))?;
    let text = value
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let role = match role {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        "file" => Role::File,
        _ => return Err(DocumentError::Validation(ValidationError::MessageCount)),
    };
    Ok(Message::new(role, text))
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_document_without_approval_field() {
        let document = ShareDocument::from_json(
            r#"{"schemaVersion":"footon.share.v1","title":"T","messages":[{"role":"assistant","text":"hi"}],"report":{"redactions":1,"detectors":["x"]}}"#,
        )
        .unwrap();
        assert_eq!(document.messages[0].role, Role::Assistant);
        assert_eq!(document.report.redactions, 1);
    }
}

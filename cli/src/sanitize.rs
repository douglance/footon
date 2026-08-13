use std::sync::LazyLock;

use redact_core::{AnalyzerEngine, AnonymizationStrategy, AnonymizerConfig};
use regex::Regex;

use crate::error::{Error, Result};
use crate::model::{Message, Report};
use crate::scanner;

const DETECTORS: [&str; 2] = ["footon-secret-patterns@1", "redact-core@0.9.1"];

static PRIVILEGED_TAGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(?:system-reminder|system|developer)[^>]*>.*?</(?:system-reminder|system|developer)>")
        .expect("static privileged tag regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    pub messages: Vec<Message>,
    pub report: Report,
}

/// Apply the independent secret scanner and redact-core to every message.
///
/// # Errors
///
/// Returns an error if a detector produces an invalid span or cannot analyze text.
pub fn sanitize_messages(messages: &[Message]) -> Result<Sanitized> {
    let engine = AnalyzerEngine::new();
    let mut redactions = 0;
    let sanitized = messages
        .iter()
        .map(|message| sanitize_message(message, &engine, &mut redactions))
        .collect::<Result<Vec<_>>>()?;
    Ok(Sanitized {
        messages: sanitized,
        report: Report {
            redactions,
            detectors: DETECTORS.into_iter().map(str::to_owned).collect(),
        },
    })
}

fn sanitize_message(
    message: &Message,
    engine: &AnalyzerEngine,
    total: &mut usize,
) -> Result<Message> {
    let (without_tags, tags) = strip_privileged_tags(&message.text);
    let (secrets_removed, secrets) = scanner::redact(&without_tags)?;
    if matches!(
        message.role,
        crate::model::Role::Tool | crate::model::Role::File
    ) {
        *total += tags + secrets;
        return Ok(Message::new(message.role, secrets_removed));
    }
    let pii = engine
        .anonymize(
            &secrets_removed,
            None,
            &AnonymizerConfig {
                strategy: AnonymizationStrategy::Replace,
                ..AnonymizerConfig::default()
            },
        )
        .map_err(|error| Error::Safety(error.to_string()))?;
    *total += tags + secrets + pii.entities.len();
    Ok(Message::new(message.role, pii.text))
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

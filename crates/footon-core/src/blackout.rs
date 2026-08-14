use schemars::JsonSchema;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{Message, Report, Role};

pub const BLACKOUT_TEXT: &str = "[BLACKED OUT]";
const BLACKOUT_DETECTOR: &str = "footon-manual-blackout@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlackoutOutcome {
    pub message: usize,
    pub replacement: &'static str,
    pub redactions: usize,
}

/// Replace one exact substring in one user or agent message.
///
/// Message numbers are one-based to match the public transcript. The match
/// must occur exactly once so an agent cannot silently redact the wrong text.
///
/// # Errors
///
/// Returns an error for invalid message numbers, empty or ambiguous matches,
/// and tool or file activity rows whose shape must remain machine-readable.
pub fn blackout(
    messages: &mut [Message],
    report: &mut Report,
    message: usize,
    text: &str,
) -> Result<BlackoutOutcome> {
    if message == 0 {
        return Err(Error::Share("message numbers start at 1".to_string()));
    }
    if text.is_empty() || text == BLACKOUT_TEXT {
        return Err(Error::Share(
            "blackout text must be a non-empty original substring".to_string(),
        ));
    }
    let target = messages
        .get_mut(message - 1)
        .ok_or_else(|| Error::Share(format!("message {message} does not exist")))?;
    if !matches!(target.role, Role::User | Role::Assistant) {
        return Err(Error::Share(
            "only USER and AGENT message text can be blacked out".to_string(),
        ));
    }
    if target.text.matches(text).count() != 1 {
        return Err(Error::Share(
            "blackout text must occur exactly once in the selected message".to_string(),
        ));
    }

    target.text = target.text.replacen(text, BLACKOUT_TEXT, 1);
    report.redactions = report.redactions.saturating_add(1);
    if !report
        .detectors
        .iter()
        .any(|item| item == BLACKOUT_DETECTOR)
    {
        report.detectors.push(BLACKOUT_DETECTOR.to_string());
    }
    Ok(BlackoutOutcome {
        message,
        replacement: BLACKOUT_TEXT,
        redactions: report.redactions,
    })
}

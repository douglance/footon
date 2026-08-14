use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{Draft, SCHEMA_VERSION};
use crate::parse::{Source, parse_jsonl};
use crate::sanitize::sanitize_messages;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftOutput {
    pub draft_path: PathBuf,
    pub report_path: PathBuf,
    pub message_count: usize,
    pub redactions: usize,
}

/// Parse and sanitize a raw thread into separate local draft and report files.
///
/// # Errors
///
/// Returns an error when the input cannot be read, parsed, sanitized, or written.
pub fn create(input: &Path, output: &Path, title: String, source: Source) -> Result<DraftOutput> {
    let raw = fs::read_to_string(input).map_err(|source| Error::Read {
        path: input.to_path_buf(),
        source,
    })?;
    let parsed = parse_jsonl(&raw, source)?;
    let sanitized = sanitize_messages(&parsed)?;
    let draft = Draft {
        schema_version: SCHEMA_VERSION.to_string(),
        title,
        messages: sanitized.messages,
        report: sanitized.report,
    };
    write(output, &draft)?;
    let report_path = report_path(output);
    write_json(&report_path, &draft.report)?;
    Ok(DraftOutput {
        draft_path: output.to_path_buf(),
        report_path,
        message_count: draft.messages.len(),
        redactions: draft.report.redactions,
    })
}

/// Read a strict sanitized draft from disk.
///
/// # Errors
///
/// Returns an error when the file cannot be read or violates the draft JSON shape.
pub fn read(path: &Path) -> Result<Draft> {
    let bytes = fs::read(path).map_err(|source| Error::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub(crate) fn write(path: &Path, draft: &Draft) -> Result<()> {
    write_json(path, draft)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|source| Error::Write {
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(&bytes).map_err(|source| Error::Write {
        path: path.to_path_buf(),
        source,
    })?;
    file.as_file().sync_all().map_err(|source| Error::Write {
        path: path.to_path_buf(),
        source,
    })?;
    file.persist(path).map_err(|error| Error::Write {
        path: path.to_path_buf(),
        source: error.error,
    })?;
    Ok(())
}

fn report_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("draft.json");
    path.with_file_name(format!("{name}.report.json"))
}

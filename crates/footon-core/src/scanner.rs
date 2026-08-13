use std::sync::LazyLock;

use regex::Regex;

use crate::error::{Error, Result};

struct Rule {
    kind: &'static str,
    regex: Regex,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    [
        ("PRIVATE_KEY", r"-----BEGIN (?:[A-Z]+ )*PRIVATE KEY-----[\s\S]*?-----END (?:[A-Z]+ )*PRIVATE KEY-----"),
        ("BEARER", r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{16,}"),
        ("AWS", r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
        ("GITHUB", r"\b(?:gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{70,}|glpat-[A-Za-z0-9_-]{20,})\b"),
        ("SLACK", r"\bxox[baprs]-[A-Za-z0-9-]{10,72}\b"),
        ("JWT", r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\b"),
        ("API_KEY", r"\b(?:sk-(?:proj-|ant-|live-)?|AIza|SG\.|npm_|pypi-)[A-Za-z0-9._-]{20,}\b"),
        ("EMAIL", r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"),
        ("CONNECTION", r"\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis|amqp)://[^\s]+"),
        ("ASSIGNMENT", r#"(?i)\b(?:api[_-]?key|client[_-]?secret|secret|token|password|passwd|auth)[A-Za-z0-9_-]*\s*[:=]\s*['\"]?[A-Za-z0-9._~+/=-]{8,}['\"]?"#),
        ("EMAIL", r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b"),
        ("SSN", r"\b\d{3}-\d{2}-\d{4}\b"),
        ("CARD", r"\b(?:\d[ -]*?){13,19}\b"),
        ("PATH", r#"(?x)(?:/Users|/home|/private|/var|/tmp)/[^\s`'"<>]+|[A-Za-z]:\\(?:[^\\\s]+\\)*[^\\\s]+"#),
    ]
    .into_iter()
    .map(|(kind, pattern)| Rule {
        kind,
        regex: Regex::new(pattern).expect("static secret regex"),
    })
    .collect()
});

#[derive(Debug)]
struct Match {
    start: usize,
    end: usize,
    kind: &'static str,
}

pub fn redact(text: &str) -> Result<(String, usize)> {
    let mut matches = RULES
        .iter()
        .flat_map(|rule| {
            rule.regex.find_iter(text).map(|found| Match {
                start: found.start(),
                end: found.end(),
                kind: rule.kind,
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|item| (item.start, std::cmp::Reverse(item.end)));
    replace_non_overlapping(text, &matches)
}

pub fn contains_sensitive_text(text: &str) -> bool {
    RULES.iter().any(|rule| rule.regex.is_match(text)) || contains_control(text)
}

fn replace_non_overlapping(text: &str, matches: &[Match]) -> Result<(String, usize)> {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    let mut count = 0;
    for item in matches {
        if item.start < cursor {
            continue;
        }
        let original = text
            .get(item.start..item.end)
            .ok_or_else(|| Error::Safety("invalid scanner span".to_string()))?;
        output.push_str(&text[cursor..item.start]);
        output.push_str(&placeholder(item.kind, original));
        cursor = item.end;
        count += 1;
    }
    output.push_str(&text[cursor..]);
    Ok((output, count))
}

fn placeholder(kind: &str, original: &str) -> String {
    let digest = blake3::hash(original.as_bytes()).to_hex();
    format!("[REDACTED:{kind}:{}]", &digest[..10])
}

fn contains_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character < ' ' && character != '\n' && character != '\t')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Html,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Candidate {
    content_type: ContentType,
    q: u16,
    specificity: u8,
    index: usize,
}

/// Negotiate `/s/{id}` response content.
///
/// Missing `Accept` and `*/*` default to HTML for browsers. If explicit HTML
/// and Markdown tie, Markdown wins for agents.
#[must_use]
pub fn negotiate(accept: Option<&str>) -> Option<ContentType> {
    let Some(header) = accept else {
        return Some(ContentType::Html);
    };
    let trimmed = header.trim();
    if trimmed.is_empty() {
        return Some(ContentType::Html);
    }

    let mut best: Option<Candidate> = None;
    for (index, part) in trimmed.split(',').enumerate() {
        let Some(candidate) = parse_range(part.trim(), index) else {
            continue;
        };
        if candidate.q == 0 {
            continue;
        }
        best = match best {
            Some(previous) if compare(previous, candidate) => Some(previous),
            _ => Some(candidate),
        };
    }
    best.map(|candidate| candidate.content_type)
}

fn compare(previous: Candidate, next: Candidate) -> bool {
    if previous.q != next.q {
        return previous.q > next.q;
    }
    if previous.specificity != next.specificity {
        return previous.specificity > next.specificity;
    }
    if previous.content_type != next.content_type {
        return previous.content_type == ContentType::Markdown;
    }
    previous.index < next.index
}

fn parse_range(value: &str, index: usize) -> Option<Candidate> {
    let mut parts = value.split(';').map(str::trim);
    let media = parts.next()?.to_ascii_lowercase();
    let q = parts
        .find_map(|part| part.strip_prefix("q="))
        .and_then(parse_q)
        .unwrap_or(1000);
    let (content_type, specificity) = match media.as_str() {
        "text/markdown" | "text/x-markdown" => (ContentType::Markdown, 2),
        "text/html" | "application/xhtml+xml" => (ContentType::Html, 2),
        "text/*" => (ContentType::Html, 1),
        "*/*" => (ContentType::Html, 0),
        _ => return None,
    };
    Some(Candidate {
        content_type,
        q,
        specificity,
        index,
    })
}

fn parse_q(raw: &str) -> Option<u16> {
    let (whole, frac) = raw.split_once('.').unwrap_or((raw, ""));
    match whole {
        "1" => Some(1000),
        "0" => {
            let mut value = 0_u16;
            for (index, digit) in frac.chars().take(3).enumerate() {
                value += u16::try_from(digit.to_digit(10)?).ok()?
                    * 10_u16.pow(2_u32.saturating_sub(u32::try_from(index).ok()?));
            }
            Some(value)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_browserish_requests_to_html() {
        assert_eq!(negotiate(None), Some(ContentType::Html));
        assert_eq!(negotiate(Some("*/*")), Some(ContentType::Html));
    }

    #[test]
    fn returns_markdown_when_requested_or_tied() {
        assert_eq!(
            negotiate(Some("text/html;q=1, text/markdown;q=1")),
            Some(ContentType::Markdown)
        );
        assert_eq!(
            negotiate(Some("text/html;q=0.5, text/markdown;q=1")),
            Some(ContentType::Markdown)
        );
    }

    #[test]
    fn rejects_unacceptable_ranges() {
        assert_eq!(negotiate(Some("application/json")), None);
        assert_eq!(negotiate(Some("text/markdown;q=0")), None);
    }
}

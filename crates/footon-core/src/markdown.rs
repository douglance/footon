use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};

use crate::model::{Message, ShareDocument};
use crate::safety::compact_messages;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedMarkdownHtml(String);

impl RenderedMarkdownHtml {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[must_use]
pub fn messages_to_markdown(document: &ShareDocument) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(&document.title);
    out.push_str("\n\n");
    for message in compact_messages(&document.messages) {
        out.push_str(message.role.markdown_heading());
        out.push_str("\n\n");
        out.push_str(message.text.trim());
        out.push_str("\n\n");
    }
    out.trim_end().to_string()
}

#[must_use]
pub fn serialize_share(share: &crate::model::Share) -> String {
    messages_to_markdown(&ShareDocument {
        schema_version: share.schema_version.clone(),
        title: share.title.clone(),
        approved_at: share.approved_at,
        messages: share.messages.clone(),
        report: share.report.clone(),
    })
}

#[must_use]
pub fn render_markdown_html(message: &Message) -> RenderedMarkdownHtml {
    let mut output = String::new();
    let mut link_stack = Vec::new();
    let parser = Parser::new_ext(
        &message.text,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    )
    .filter_map(|event| safe_event(event, &mut link_stack))
    .map(shift_heading);
    html::push_html(&mut output, parser);
    RenderedMarkdownHtml(output)
}

fn safe_event<'a>(event: Event<'a>, link_stack: &mut Vec<bool>) -> Option<Event<'a>> {
    match event {
        Event::Start(
            Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }
            | Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            },
        ) => {
            let safe = safe_link(&dest_url);
            link_stack.push(safe);
            safe.then_some({
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            })
        }
        Event::End(TagEnd::Image | TagEnd::Link) => link_stack
            .pop()
            .unwrap_or(false)
            .then_some(Event::End(TagEnd::Link)),
        Event::Html(text) | Event::InlineHtml(text) => Some(Event::Text(text)),
        Event::End(TagEnd::Table) => Some(Event::End(TagEnd::Table)),
        other => Some(other),
    }
}

fn shift_heading(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Start(Tag::Heading {
            level,
            id,
            classes,
            attrs,
        }) => Event::Start(Tag::Heading {
            level: shift_level(level),
            id,
            classes,
            attrs,
        }),
        Event::End(TagEnd::Heading(level)) => Event::End(TagEnd::Heading(shift_level(level))),
        other => other,
    }
}

fn shift_level(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H2,
        HeadingLevel::H2 => HeadingLevel::H3,
        HeadingLevel::H3 => HeadingLevel::H4,
        HeadingLevel::H4 => HeadingLevel::H5,
        HeadingLevel::H5 | HeadingLevel::H6 => HeadingLevel::H6,
    }
}

fn safe_link(dest: &CowStr<'_>) -> bool {
    let lower = dest.trim_start().to_ascii_lowercase();
    !(lower.starts_with("javascript:") || lower.starts_with("vbscript:"))
}

#[cfg(test)]
mod tests {
    use crate::model::{Report, Role, ShareDocument};

    use super::*;

    #[test]
    fn markdown_labels_assistant_as_agent() {
        let document = ShareDocument {
            schema_version: "footon.share.v2".to_string(),
            title: "Dense".to_string(),
            approved_at: chrono::Utc::now(),
            messages: vec![Message {
                role: Role::Assistant,
                text: "done".to_string(),
            }],
            report: Report::default(),
        };
        assert!(messages_to_markdown(&document).contains("## AGENT"));
    }

    #[test]
    fn html_renderer_drops_html_and_script_links_but_links_images() {
        let html = render_markdown_html(&Message {
            role: Role::Assistant,
            text: "<b>x</b>\n![alt](https://x)\n[good](https://good)\n[bad](javascript:alert(1))"
                .to_string(),
        });
        assert!(!html.as_str().contains("<b>"));
        assert!(!html.as_str().contains("<img"));
        assert!(html.as_str().contains("href=\"https://x\""));
        assert_eq!(html.as_str().matches("<a ").count(), 2);
        assert_eq!(html.as_str().matches("</a>").count(), 2);
        assert!(!html.as_str().contains("javascript:"));
    }

    #[test]
    fn html_renderer_displays_custom_tags_as_literal_transcript_text() {
        let html = render_markdown_html(&Message {
            role: Role::Assistant,
            text: "before\n\n<oai-mem-citation>\n<citation_entries>\nsource:1-2\n</citation_entries>\n</oai-mem-citation>\n\nafter".to_string(),
        });

        assert!(html.as_str().contains("&lt;oai-mem-citation&gt;"));
        assert!(html.as_str().contains("source:1-2"));
        assert!(html.as_str().contains("after"));
    }
}

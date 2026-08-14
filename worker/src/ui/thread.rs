use chrono::Datelike;
use footon_core::{
    markdown::render_markdown_html,
    model::{Message, Role, ShareRecord},
    safety::compact_messages,
};
use topcoat::{
    Result,
    context::Cx,
    view::{Unescaped, component, view},
};

use crate::ui::layout::ASSET_VERSION;

pub(crate) const VIEWER_JS: &str = include_str!("../viewer.js");

#[component]
pub(crate) async fn thread_demo(messages: &[Message]) -> Result {
    let summary = format!("{} messages · 4 redactions", messages.len());
    let script = format!("/viewer.js?v={ASSET_VERSION}");
    view! {
        thread_view(
            messages: messages,
            title: "Deployment review",
            summary: &summary,
            text_mode: false,
            class: "viewer thread-demo",
            label: "Actual Footon output example",
            scroll_container: true,
        )
        <script src=(script) defer="defer"></script>
    }
}

pub(crate) async fn viewer_page(record: &ShareRecord, text_mode: bool) -> Result {
    let cx = &Cx::default();
    let messages = compact_messages(&record.document.messages);
    let stylesheet = format!("/style.css?v={ASSET_VERSION}");
    let script = format!("/viewer.js?v={ASSET_VERSION}");
    let page_title = format!("{} · footon", record.title);
    let shared = format!(
        "Shared {}. {} redactions.",
        format_date(record.created_at.date_naive()),
        record.document.report.redactions,
    );

    view! {
        cx =>
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width,initial-scale=1">
                <title>(page_title)</title>
                <link rel="icon" href="/favicon.svg" type="image/svg+xml">
                <link rel="stylesheet" href=(stylesheet)>
            </head>
            <body class="viewer-page">
                <header><a class="brand" href="/">"footon"</a></header>
                <main>
                    thread_view(
                        messages: &messages,
                        title: &record.title,
                        summary: &shared,
                        text_mode: text_mode,
                        class: "viewer",
                        label: "Shared Footon thread",
                        scroll_container: false,
                    )
                </main>
                <script src=(script) defer="defer"></script>
            </body>
        </html>
    }
}

#[component]
async fn thread_view(
    messages: &[Message],
    title: &str,
    summary: &str,
    text_mode: bool,
    class: &str,
    label: &str,
    scroll_container: bool,
) -> Result {
    view! {
        <article
            class=(class)
            aria-label=(label)
            data-thread-scroll=(scroll_container.to_string())
        >
            <input
                class="thread-view-toggle"
                id="thread-view"
                type="checkbox"
                aria-label="Show source text for all messages"
                checked=(text_mode)
            >
            <div class="meta">
                <div class="document-heading">
                    <h1>(title)</h1>
                    <p>(summary)</p>
                </div>
                <div class="toolbar" role="toolbar" aria-label="Thread display controls">
                    <div class="role-filters" aria-label="Message filters">
                        <input class="filter-input" id="filter-user" type="checkbox" data-filter-role="user" checked="checked">
                        <label class="filter-toggle user" for="filter-user">"USER"</label>
                        <input class="filter-input" id="filter-agent" type="checkbox" data-filter-role="assistant" checked="checked">
                        <label class="filter-toggle assistant" for="filter-agent">"AGENT"</label>
                        <input class="filter-input" id="filter-tools" type="checkbox" data-filter-role="tool" checked="checked">
                        <label class="filter-toggle tool" for="filter-tools">"TOOL"</label>
                    </div>
                    <label class="view-control" for="thread-view" title="Toggle rendered or source text">
                        <span class="view-icon rendered-icon" aria-hidden="true">
                            <svg viewBox="0 0 16 16" focusable="false">
                                <path d="M1.5 8s2.4-4 6.5-4 6.5 4 6.5 4-2.4 4-6.5 4S1.5 8 1.5 8Z"></path>
                                <circle cx="8" cy="8" r="1.75"></circle>
                            </svg>
                        </span>
                        <span class="view-icon text-icon" aria-hidden="true">
                            <svg viewBox="0 0 16 16" focusable="false">
                                <path d="m5.5 4-4 4 4 4M10.5 4l4 4-4 4"></path>
                            </svg>
                        </span>
                    </label>
                </div>
            </div>
            thread_minimap(messages: messages)
            thread_rows(messages: messages)
        </article>
    }
}

enum ThreadGroup<'a> {
    Call {
        message: &'a Message,
        activity: &'a [Message],
        index: usize,
    },
    Activity {
        messages: &'a [Message],
        index: usize,
    },
    Message {
        message: &'a Message,
        index: usize,
    },
}

fn group_messages(messages: &[Message]) -> Vec<ThreadGroup<'_>> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < messages.len() {
        let message = &messages[index];
        if message.role == Role::Assistant {
            let end = activity_end(messages, index + 1);
            groups.push(ThreadGroup::Call {
                message,
                activity: &messages[index + 1..end],
                index,
            });
            index = end;
            continue;
        }
        let end = activity_end(messages, index);
        if end > index {
            groups.push(ThreadGroup::Activity {
                messages: &messages[index..end],
                index,
            });
            index = end;
            continue;
        }
        groups.push(ThreadGroup::Message { message, index });
        index += 1;
    }
    groups
}

fn activity_end(messages: &[Message], start: usize) -> usize {
    let mut end = start;
    while messages
        .get(end)
        .is_some_and(|message| matches!(message.role, Role::Tool | Role::File))
    {
        end += 1;
    }
    end
}

#[component]
async fn thread_rows(messages: &[Message]) -> Result {
    let groups = group_messages(messages);
    view! {
        <div class="thread">
            for group in groups {
                match group {
                    ThreadGroup::Call { message, activity, index } => {
                        <section class="call-block">
                            message_row(message: message, index: index)
                            activity_run(messages: activity, start: index + 1)
                        </section>
                    }
                    ThreadGroup::Activity { messages, index } => {
                        activity_run(messages: messages, start: index)
                    }
                    ThreadGroup::Message { message, index } => {
                        message_row(message: message, index: index)
                    }
                }
            }
        </div>
    }
}

#[component]
async fn activity_run(messages: &[Message], start: usize) -> Result {
    view! {
        if !messages.is_empty() {
            <div class="activity-run" role="group" aria-label="Tool and file activity">
                for (offset, message) in messages.iter().enumerate() {
                    message_row(message: message, index: start + offset)
                }
            </div>
        }
    }
}

#[component]
async fn message_row(message: &Message, index: usize) -> Result {
    let ordinal = index + 1;
    let ordinal_text = format!("{ordinal:03}");
    let role = message.role.css_class();
    let label = message.role.label();
    let id = format!("message-{ordinal}");
    let href = format!("#{id}");
    let link_label = format!("{ordinal_text}, link to message {ordinal}");
    let region_label = format!(
        "{} {ordinal}",
        if message.role == Role::Assistant {
            "agent"
        } else {
            role
        }
    );

    view! {
        <section class=(format!("message {role}")) id=(id) aria-label=(region_label)>
            <a class="ordinal" href=(href) aria-label=(link_label)>(ordinal_text)</a>
            <span class="role">(label)</span>
            if matches!(message.role, Role::Tool | Role::File) {
                <p>(&message.text)</p>
            } else {
                let rendered = render_markdown_html(message);
                // `RenderedMarkdownHtml` strips raw HTML and unsafe URL schemes in footon-core.
                let rendered = Unescaped::new_unchecked(rendered.as_str().to_string());
                <div class="message-body">
                    <div class="rendered">(rendered)</div>
                    <pre class="message-text">(&message.text)</pre>
                </div>
            }
        </section>
    }
}

#[component]
async fn thread_minimap(messages: &[Message]) -> Result {
    view! {
        <div class="minimap-frame">
            <nav class="minimap" aria-label="Thread minimap">
                <div class="map-viewport" aria-hidden="true"></div>
                <ol>
                    for (index, message) in messages.iter().enumerate() {
                        minimap_marker(message: message, index: index)
                    }
                </ol>
            </nav>
        </div>
    }
}

#[component]
async fn minimap_marker(message: &Message, index: usize) -> Result {
    let ordinal = index + 1;
    let role = message.role.css_class();
    let message_id = format!("message-{ordinal}");
    view! {
        <li>
            if message.role == Role::User {
                <a
                    class="map-marker user"
                    href=(format!("#{message_id}"))
                    aria-label=(format!("Jump to user message {ordinal}"))
                ></a>
            } else {
                <span class=(format!("map-marker {role}")) data-message-id=(message_id)></span>
            }
        </li>
    }
}

fn format_date(date: chrono::NaiveDate) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    format!(
        "{} {}, {}",
        MONTHS[date.month0() as usize],
        date.day(),
        date.year()
    )
}

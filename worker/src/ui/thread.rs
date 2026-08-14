use chrono::Datelike;
use footon_core::{model::ShareRecord, safety::compact_messages};
use topcoat::{
    Result,
    context::Cx,
    view::{Unescaped, view},
};

use crate::{ui::layout::ASSET_VERSION, viewer::render_transcript};

pub(crate) async fn viewer_page(record: &ShareRecord, text_mode: bool) -> Result {
    let cx = &Cx::default();
    let messages = compact_messages(&record.document.messages);
    let transcript = render_transcript(&messages);
    let stylesheet = format!("/style.css?v={ASSET_VERSION}");
    let script = format!("/viewer.js?v={ASSET_VERSION}");
    let page_title = format!("{} · footon", record.title);
    let shared = format!(
        "Shared {}. {} redactions.",
        format_date(record.created_at.date_naive()),
        record.document.report.redactions,
    );

    // The legacy transcript renderer escapes plain text and emits only sanitized Markdown HTML.
    // These two wrappers disappear when transcript rows move to typed views in the next slice.
    let minimap = Unescaped::new_unchecked(transcript.map);
    let rows = Unescaped::new_unchecked(transcript.messages);

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
                    <article class="viewer">
                        <input
                            class="thread-view-toggle"
                            id="thread-view"
                            type="checkbox"
                            aria-label="Show source text for all messages"
                            checked=(text_mode)
                        >
                        <div class="meta">
                            <div class="document-heading">
                                <h1>(&record.title)</h1>
                                <p>(shared)</p>
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
                        (minimap)
                        <div class="thread">(rows)</div>
                    </article>
                </main>
                <script src=(script) defer="defer"></script>
            </body>
        </html>
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

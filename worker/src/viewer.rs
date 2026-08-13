use footon_core::markdown::render_markdown_html;
use footon_core::model::{Message, Role, ShareRecord};
use footon_core::safety::compact_messages;

use crate::escape_html;

pub(crate) fn viewer_page(record: &ShareRecord, text_mode: bool) -> String {
    let messages = compact_messages(&record.document.messages);
    let title = escape_html(&record.title);
    let checked = if text_mode { " checked" } else { "" };
    let header = format!(
        "<input class=\"thread-view-toggle\" id=\"thread-view\" type=\"checkbox\" aria-label=\"Show source text for all messages\"{checked}><div class=\"meta\"><div class=\"document-heading\"><h1>{title}</h1><p>Shared {}. {} redactions.</p></div><label class=\"view-control\" for=\"thread-view\"><span>Rendered</span><span>Text</span></label></div>",
        format_date(record.created_at.date_naive()),
        record.document.report.redactions,
    );
    let transcript = render_transcript(&messages);
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title} · footon</title><link rel=\"stylesheet\" href=\"/style.css\"></head><body><header><a class=\"brand\" href=\"/\">footon</a></header><main><article class=\"viewer\">{header}{}<div class=\"thread\">{}</div></article></main><script src=\"/viewer.js\" defer></script></body></html>",
        transcript.map, transcript.messages,
    )
}

struct Transcript {
    map: String,
    messages: String,
}

fn render_transcript(messages: &[Message]) -> Transcript {
    let mut rendered = String::new();
    let mut markers = String::new();
    let mut index = 0;
    while index < messages.len() {
        let message = &messages[index];
        if message.role == Role::Assistant {
            let end = activity_end(messages, index + 1);
            rendered.push_str(&render_call(message, &messages[index + 1..end], index));
            index = end;
            continue;
        }
        let end = activity_end(messages, index);
        if end > index {
            rendered.push_str(&render_activity_run(&messages[index..end], index));
            index = end;
            continue;
        }
        rendered.push_str(&render_message(message, index));
        index += 1;
    }
    for (index, message) in messages.iter().enumerate() {
        markers.push_str(&render_marker(message, index));
    }
    Transcript {
        map: format!(
            "<nav class=\"minimap\" aria-label=\"Thread minimap\"><div class=\"map-viewport\" aria-hidden=\"true\"></div><ol>{markers}</ol></nav>"
        ),
        messages: rendered,
    }
}

fn render_call(message: &Message, activity: &[Message], index: usize) -> String {
    format!(
        "<section class=\"call-block\">{}{}</section>",
        render_message(message, index),
        render_activity_run(activity, index + 1),
    )
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

fn render_activity_run(messages: &[Message], start: usize) -> String {
    if messages.is_empty() {
        return String::new();
    }
    let rows = messages
        .iter()
        .enumerate()
        .map(|(offset, message)| render_message(message, start + offset))
        .collect::<String>();
    format!("<ol class=\"activity-run\" aria-label=\"Tool and file activity\">{rows}</ol>")
}

fn render_message(message: &Message, index: usize) -> String {
    let ordinal = index + 1;
    let role = message.role.css_class();
    let label = message.role.label();
    if matches!(message.role, Role::Tool | Role::File) {
        return format!(
            "<section class=\"message {role}\" id=\"message-{ordinal}\" aria-label=\"{role} {ordinal}\"><span class=\"ordinal\">{ordinal:03}</span><span class=\"role\">{label}</span><p>{}</p></section>",
            escape_html(&message.text),
        );
    }
    let rendered = render_markdown_html(message);
    format!(
        "<section class=\"message {role}\" id=\"message-{ordinal}\" aria-label=\"{} {ordinal}\"><span class=\"ordinal\">{ordinal:03}</span><span class=\"role\">{label}</span><div class=\"message-body\"><div class=\"rendered\">{}</div><pre class=\"message-text\">{}</pre></div></section>",
        if message.role == Role::Assistant {
            "agent"
        } else {
            role
        },
        rendered.as_str(),
        escape_html(&message.text),
    )
}

fn render_marker(message: &Message, index: usize) -> String {
    let ordinal = index + 1;
    let role = message.role.css_class();
    if message.role == Role::User {
        format!(
            "<li><a class=\"map-marker user\" href=\"#message-{ordinal}\" aria-label=\"Jump to user message {ordinal}\"></a></li>"
        )
    } else {
        format!(
            "<li><span class=\"map-marker {role}\" data-message-id=\"message-{ordinal}\"></span></li>"
        )
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

use chrono::Datelike;

pub(crate) const VIEWER_JS: &str = r"
const rail=document.querySelector('.minimap');const map=document.querySelector('.minimap ol');const markers=[...document.querySelectorAll('.map-marker')];const targets=markers.map(marker=>({marker,message:document.getElementById(marker.getAttribute('href')?.slice(1)||marker.dataset.messageId)}));const canvas=document.createElement('canvas');const context=canvas.getContext('2d',{alpha:true});const texture=document.createElement('canvas');const textureContext=texture.getContext('2d',{alpha:true});const reducedMotion=matchMedia('(prefers-reduced-motion: reduce)').matches;let scale=1,frame=0;
function render(){const height=map.clientHeight,viewportHeight=Math.min(height,innerHeight*scale),viewportTop=Math.min(height-viewportHeight,Math.max(0,scrollY*scale));context.clearRect(0,0,map.clientWidth,height);context.drawImage(texture,0,0,map.clientWidth,height);context.fillStyle='rgba(255,255,255,.2)';context.fillRect(0,viewportTop,map.clientWidth,viewportHeight)}
function schedule(){if(frame)return;frame=requestAnimationFrame(()=>{frame=0;render()})}
function layout(){if(!rail||!map||!context||!textureContext)return;scale=map.clientHeight/document.documentElement.scrollHeight;const ratio=devicePixelRatio||1,width=map.clientWidth,height=map.clientHeight;canvas.width=Math.round(width*ratio);canvas.height=Math.round(height*ratio);texture.width=canvas.width;texture.height=canvas.height;canvas.style.width=width+'px';canvas.style.height=height+'px';context.setTransform(ratio,0,0,ratio,0,0);textureContext.setTransform(ratio,0,0,ratio,0,0);textureContext.clearRect(0,0,width,height);for(const{marker,message}of targets){if(!message)continue;const top=message.getBoundingClientRect().top+scrollY;textureContext.fillStyle=getComputedStyle(marker).backgroundColor;textureContext.fillRect(1,top*scale,Math.max(1,width-2),Math.max(1,message.offsetHeight*scale))}render();rail.classList.add('enhanced')}
canvas.tabIndex=0;canvas.setAttribute('role','navigation');canvas.setAttribute('aria-label','Thread minimap');canvas.addEventListener('pointerdown',event=>scrollTo({top:event.clientY/scale-innerHeight/2,behavior:reducedMotion?'auto':'smooth'}));rail?.prepend(canvas);addEventListener('scroll',schedule,{passive:true});addEventListener('resize',layout);layout();
";

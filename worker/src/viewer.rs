use crate::escape_html;
use footon_core::markdown::render_markdown_html;
use footon_core::model::{Message, Role};

pub(crate) struct Transcript {
    pub(crate) map: String,
    pub(crate) messages: String,
}

pub(crate) fn render_transcript(messages: &[Message]) -> Transcript {
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
            "<div class=\"minimap-frame\"><nav class=\"minimap\" aria-label=\"Thread minimap\"><div class=\"map-viewport\" aria-hidden=\"true\"></div><ol>{markers}</ol></nav></div>"
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
            "<section class=\"message {role}\" id=\"message-{ordinal}\" aria-label=\"{role} {ordinal}\"><a class=\"ordinal\" href=\"#message-{ordinal}\" aria-label=\"Link to message {ordinal}\">{ordinal:03}</a><span class=\"role\">{label}</span><p>{}</p></section>",
            escape_html(&message.text),
        );
    }
    let rendered = render_markdown_html(message);
    format!(
        "<section class=\"message {role}\" id=\"message-{ordinal}\" aria-label=\"{} {ordinal}\"><a class=\"ordinal\" href=\"#message-{ordinal}\" aria-label=\"Link to message {ordinal}\">{ordinal:03}</a><span class=\"role\">{label}</span><div class=\"message-body\"><div class=\"rendered\">{}</div><pre class=\"message-text\">{}</pre></div></section>",
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

pub(crate) const VIEWER_JS: &str = r"
const rail=document.querySelector('.minimap');const map=document.querySelector('.minimap ol');const markers=[...document.querySelectorAll('.map-marker')];const targets=markers.map(marker=>({marker,message:document.getElementById(marker.getAttribute('href')?.slice(1)||marker.dataset.messageId)}));const filters=[...document.querySelectorAll('[data-filter-role]')];const canvas=document.createElement('canvas');const context=canvas.getContext('2d',{alpha:true});const texture=document.createElement('canvas');const textureContext=texture.getContext('2d',{alpha:true});let scale=1,frame=0,dragging=false;
function render(){const height=map.clientHeight,viewportHeight=Math.min(height,innerHeight*scale),viewportTop=Math.min(height-viewportHeight,Math.max(0,scrollY*scale));context.clearRect(0,0,map.clientWidth,height);context.drawImage(texture,0,0,map.clientWidth,height);context.fillStyle='rgba(255,255,255,.2)';context.fillRect(0,viewportTop,map.clientWidth,viewportHeight)}
function schedule(){if(frame)return;frame=requestAnimationFrame(()=>{frame=0;render()})}
function layout(){if(!rail||!map||!context||!textureContext)return;scale=map.clientHeight/document.documentElement.scrollHeight;const ratio=devicePixelRatio||1,width=map.clientWidth,height=map.clientHeight;canvas.width=Math.round(width*ratio);canvas.height=Math.round(height*ratio);texture.width=canvas.width;texture.height=canvas.height;canvas.style.width=width+'px';canvas.style.height=height+'px';context.setTransform(ratio,0,0,ratio,0,0);textureContext.setTransform(ratio,0,0,ratio,0,0);textureContext.clearRect(0,0,width,height);for(const{marker,message}of targets){if(!message||message.hidden)continue;const top=message.getBoundingClientRect().top+scrollY;textureContext.fillStyle=getComputedStyle(marker).backgroundColor;textureContext.fillRect(1,top*scale,Math.max(1,width-2),Math.max(1,message.offsetHeight*scale))}render();rail.classList.add('enhanced')}
function filterRole(message){if(message.classList.contains('user'))return'user';if(message.classList.contains('assistant'))return'assistant';return'tool'}
function applyFilters(){const enabled=new Set(filters.filter(filter=>filter.checked).map(filter=>filter.dataset.filterRole));for(const{marker,message}of targets){if(!message)continue;const hidden=!enabled.has(filterRole(message));message.hidden=hidden;marker.closest('li')?.toggleAttribute('hidden',hidden)}for(const run of document.querySelectorAll('.activity-run'))run.hidden=![...run.querySelectorAll('.message')].some(message=>!message.hidden);for(const block of document.querySelectorAll('.call-block'))block.hidden=![...block.querySelectorAll('.message')].some(message=>!message.hidden);requestAnimationFrame(layout)}
function seek(event,behavior){const bounds=canvas.getBoundingClientRect(),y=Math.min(bounds.height,Math.max(0,event.clientY-bounds.top));scrollTo({top:y/scale-innerHeight/2,behavior})}
function stopDragging(event){if(!dragging)return;dragging=false;if(canvas.hasPointerCapture(event.pointerId))canvas.releasePointerCapture(event.pointerId)}
canvas.tabIndex=0;canvas.setAttribute('role','navigation');canvas.setAttribute('aria-label','Thread minimap');canvas.addEventListener('pointerdown',event=>{event.preventDefault();dragging=true;canvas.setPointerCapture(event.pointerId);seek(event,'auto')});canvas.addEventListener('pointermove',event=>{if(dragging)seek(event,'auto')});canvas.addEventListener('pointerup',stopDragging);canvas.addEventListener('pointercancel',stopDragging);rail?.prepend(canvas);for(const filter of filters)filter.addEventListener('change',applyFilters);addEventListener('scroll',schedule,{passive:true});addEventListener('resize',layout);addEventListener('load',layout,{once:true});applyFilters();
";

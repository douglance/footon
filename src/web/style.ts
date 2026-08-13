export const CSS = `
:root {
  color-scheme: dark;
  --bg: #090c0a;
  --panel: #0e1310;
  --ink: #d7dfd8;
  --muted: #7d8c81;
  --line: #29332c;
  --green: #72e39f;
  --amber: #e6b566;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; scroll-padding-top: 52px; }
body {
  margin: 0;
  padding: 0 18px;
  overflow-x: hidden;
  background: var(--bg);
  color: var(--ink);
  font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}
header, main { width: 100%; max-width: 1040px; margin-inline: auto; }
header {
  position: sticky; z-index: 6; top: 0;
  height: 30px;
  display: flex;
  align-items: center;
  border-bottom: 1px solid var(--line);
  background: var(--bg); color: var(--muted);
  font-size: 10px;
}
.brand { color: var(--green); font-weight: 800; text-decoration: none; }
main { padding: 12px 0 40px; overflow-wrap: anywhere; }
h1 { max-width: 820px; margin: 5px 0 10px; font-size: clamp(20px, 3vw, 30px); line-height: 1.18; letter-spacing: -.025em; }
p { margin: 8px 0; }
.lede { max-width: 700px; color: #b3beb5; font-size: 14px; }
.role { color: var(--green); font-weight: 750; }
.muted { color: #91a096; }
.panel { max-width: 800px; padding: 18px 20px; border: 1px solid var(--line); background: var(--panel); }
.actions { display: flex; gap: 8px; flex-wrap: wrap; margin: 18px 0; }
.button, button {
  display: inline-block;
  border: 1px solid var(--green);
  border-radius: 0;
  background: var(--green);
  color: var(--bg);
  padding: 7px 10px;
  font: inherit;
  font-weight: 750;
  text-decoration: none;
  cursor: pointer;
}
.button.secondary { background: transparent; color: var(--green); }
.facts { display: grid; grid-template-columns: repeat(3, 1fr); margin: 18px 0 0; border-top: 1px solid var(--line); }
.facts div { padding: 9px 12px 0 0; }
.facts dt { color: var(--muted); font-size: 10px; text-transform: uppercase; }
.facts dd { margin: 2px 0 0; color: var(--ink); }
form { max-width: 620px; padding: 16px; border: 1px solid var(--line); background: var(--panel); }
label { display: block; margin-bottom: 5px; color: var(--green); font-weight: 700; }
input { width: 100%; margin-bottom: 12px; padding: 8px; border: 1px solid #4b5b50; border-radius: 0; background: var(--bg); color: var(--ink); font: inherit; }
code, pre { font-family: inherit; }
pre { max-width: 900px; overflow: auto; margin: 14px 0; padding: 12px 14px; border: 1px solid var(--line); background: var(--panel); color: #c8d2ca; }
.viewer { position: relative; padding-right: 64px; }
.meta {
  position: sticky; z-index: 5; top: 30px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: end;
  gap: 16px;
  margin-bottom: 6px;
  padding: 7px 0 8px; border-bottom: 1px solid var(--line); background: var(--bg);
}
.meta h1 { margin: 0 0 2px; font-size: 18px; line-height: 1.2; letter-spacing: -.015em; }
.meta p { margin: 0; color: var(--muted); font-size: 10.5px; line-height: 1.35; }
.view-control {
  display: grid;
  grid-template-columns: 1fr 1fr;
  width: max-content;
  margin: 0; padding: 2px;
  border: 1px solid var(--line);
  background: var(--panel);
  color: var(--muted);
  font-size: 9px;
  font-weight: 650;
  line-height: 1;
  cursor: pointer;
}
.view-control span { padding: 4px 6px; }
.view-control span:first-child { background: var(--line); color: var(--ink); }
.thread-view-toggle {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: 0;
  padding: 0;
  clip-path: inset(50%);
  overflow: hidden;
  white-space: nowrap;
}
.thread-view-toggle:focus-visible + .meta .view-control { outline: 2px solid var(--amber); outline-offset: 2px; }
.thread-view-toggle:checked + .meta .view-control span:first-child { background: transparent; color: var(--muted); }
.thread-view-toggle:checked + .meta .view-control span:last-child { background: var(--line); color: var(--ink); }
.thread-view-toggle:checked ~ .thread .rendered { display: none; }
.thread-view-toggle:checked ~ .thread .message-text { display: block; }
.thread { border-top: 1px solid var(--line); }
.call-block { margin: 0; border-bottom: 1px solid var(--line); }
.message {
  display: grid;
  grid-template-columns: 94px minmax(0, 1fr);
  align-items: baseline;
  margin: 0;
  padding: 7px 12px 8px 0;
  border-bottom: 1px solid var(--line);
  scroll-margin-top: 52px;
}
.message.user { margin-top: 7px; border-left: 2px solid var(--green); background: #0c1510; }
.call-block > .message { border-bottom: 0; }
.message:target { box-shadow: inset 2px 0 var(--green); }
.message-body {
  min-width: 0;
  max-width: 78ch;
}
.message-body p {
  margin: 0;
  color: #d3dbd5;
  font-size: 13.5px;
  line-height: 1.42;
  letter-spacing: -.006em;
}
.message-body p + p, .message-body pre, .message-body ul, .message-body ol, .message-body blockquote { margin-top: 8px; }
.message-body ul, .message-body ol { padding-left: 20px; }
.message-body h2, .message-body h3, .message-body h4, .message-body h5, .message-body h6 {
  margin: 0 0 8px;
  color: var(--ink);
  font-size: 15px;
  line-height: 1.25;
  letter-spacing: 0;
}
.message-body a { color: var(--green); }
.message-body code { color: #f1c98a; }
.message-body pre {
  max-width: 100%;
  overflow-x: auto;
  white-space: pre;
}
.message-text {
  display: none;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: #d3dbd5;
  font-size: 13.5px;
  line-height: 1.42;
  letter-spacing: -.006em;
  white-space: pre-wrap;
}
.message-body .message-text { overflow-wrap: anywhere; white-space: pre-wrap; }
.message-text.fallback { display: block; }
.rendered > :last-child { margin-bottom: 0; }
.message > p {
  min-width: 0;
  max-width: 78ch;
  margin: 0;
  color: #d3dbd5;
  font-size: 13.5px;
  line-height: 1.42;
  letter-spacing: -.006em;
  white-space: pre-wrap;
}
.role { display: flex; gap: 10px; padding-left: 10px; color: var(--green); font-size: 9px; font-weight: 750; letter-spacing: .06em; text-transform: uppercase; }
.role span { flex: 0 0 26px; color: var(--muted); font-weight: 500; }
.message.assistant .role { color: var(--amber); }
.activity-run { margin: 0 0 5px; padding: 1px 0 2px; list-style: none; }
.message.tool, .message.file { min-height: 19px; grid-template-columns: 94px minmax(0, 1fr); padding: 1px 8px 1px 0; border: 0; background: transparent; }
.message.tool .role { color: #68cce8; }
.message.file .role { color: #c6a4ef; }
.message.tool p, .message.file p { color: #a8b5ac; font-size: 11.5px; line-height: 1.38; letter-spacing: 0; }
.minimap {
  position: fixed;
  inset-block: 0;
  width: 48px;
  margin-left: min(992px, calc(100vw - 84px));
  border-left: 1px solid var(--line);
  font-size: 9px;
}
.minimap ol { position: relative; z-index: 1; height: 100vh; margin: 0; padding: 0; list-style: none; }
.minimap li { position: absolute; inset-inline: 1px; min-height: 1px; }
.map-viewport { position: absolute; z-index: 2; inset-inline: 0; top: 0; background: rgba(255, 255, 255, .24); pointer-events: none; }
.map-marker { display: block; width: 100%; height: 100%; min-height: 1px; background: rgba(66, 80, 71, .55); text-decoration: none; }
.map-marker.user { background: rgba(114, 227, 159, .55); }
.map-marker.assistant { background: rgba(230, 181, 102, .55); }
.map-marker.tool { background: rgba(104, 204, 232, .55); }
.map-marker.file { background: rgba(198, 164, 239, .55); }
.map-marker:hover, .map-marker:focus-visible { filter: brightness(1.25); }
a:hover { color: #a3f6c1; }
.minimap canvas { position: absolute; z-index: 3; inset: 0; display: block; width: 100%; height: 100%; cursor: pointer; }
.minimap.enhanced ol, .minimap.enhanced .map-viewport { visibility: hidden; pointer-events: none; }
:focus-visible { outline: 2px solid var(--amber); outline-offset: 2px; }
@media (max-width: 720px) {
  body { padding-inline: 12px; }
  main { padding-top: 8px; }
  .facts { grid-template-columns: 1fr; }
  .viewer { padding-right: 24px; }
  .meta { gap: 8px; padding-top: 6px; }
  .meta h1 { font-size: 16px; }
  .meta p { font-size: 9.5px; }
  .view-control span { padding-inline: 4px; }
  .message { grid-template-columns: 72px minmax(0, 1fr); padding-right: 4px; }
  .message-body p, .message > p, .message-text { font-size: 12.5px; line-height: 1.42; }
  .role { gap: 5px; padding-left: 4px; }
  .activity-run { margin-left: 0; padding-left: 0; }
  .message.tool, .message.file { grid-template-columns: 72px minmax(0, 1fr); }
  .message.tool p, .message.file p { font-size: 10.5px; }
  .minimap { right: 5px; width: 10px; margin-left: 0; }
}
@media (prefers-reduced-motion: reduce) { html { scroll-behavior: auto; } }
`

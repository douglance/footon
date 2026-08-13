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
  height: 42px;
  display: flex;
  align-items: center;
  gap: 12px;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
}
.brand { color: var(--green); font-weight: 800; text-decoration: none; }
.status { margin-left: auto; color: var(--green); }
main { padding: 24px 0 40px; overflow-wrap: anywhere; }
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
.viewer { position: relative; padding-right: 112px; }
.meta { margin-bottom: 7px; padding-bottom: 8px; border-bottom: 1px solid var(--line); }
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
.message p {
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
  width: 96px;
  margin-left: min(944px, calc(100vw - 132px));
  padding-left: 10px;
  border-left: 1px solid var(--line);
  font-size: 9px;
}
.minimap ol { display: flex; flex-direction: column; height: 100vh; margin: 0; padding: 0 3px 0 0; list-style: none; }
.minimap li { flex: 1 1 1px; min-height: 1px; max-height: 9px; }
.map-marker { display: block; width: 28px; height: 100%; min-height: 1px; border-top: 1px solid #425047; text-decoration: none; }
.map-marker.user { width: 76px; min-height: 2px; border-color: var(--green); }
.map-marker.assistant { border-color: var(--amber); opacity: .72; }
.map-marker.tool { border-color: #68cce8; }
.map-marker.file { border-color: #c6a4ef; }
.map-marker.active { width: 76px; border-color: var(--green); background: rgba(114, 227, 159, .32); opacity: 1; }
.map-marker:hover, .map-marker:focus-visible { width: 76px; opacity: 1; }
a:hover { color: #a3f6c1; }
:focus-visible { outline: 2px solid var(--amber); outline-offset: 2px; }
@media (max-width: 720px) {
  body { padding-inline: 12px; }
  header { gap: 8px; }
  header span:not(.status) { display: none; }
  main { padding-top: 16px; }
  .facts { grid-template-columns: 1fr; }
  .viewer { padding-right: 32px; }
  h1 { font-size: 21px; line-height: 1.22; }
  .message { grid-template-columns: 72px minmax(0, 1fr); padding-right: 4px; }
  .message p { font-size: 12.5px; line-height: 1.42; }
  .role { padding-left: 4px; }
  .role { gap: 5px; }
  .activity-run { margin-left: 0; padding-left: 0; }
  .message.tool, .message.file { grid-template-columns: 72px minmax(0, 1fr); }
  .message.tool p, .message.file p { font-size: 10.5px; }
  .minimap { right: 5px; width: 23px; margin-left: 0; padding-left: 5px; }
  .map-marker { width: 8px; }
  .map-marker.user, .map-marker.active, .map-marker:hover, .map-marker:focus-visible { width: 17px; }
}
@media (prefers-reduced-motion: reduce) { html { scroll-behavior: auto; } }
`

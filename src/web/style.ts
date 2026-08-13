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
  font: 13px/1.48 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
header, main, footer { width: 100%; max-width: 1040px; margin-inline: auto; }
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
.brand::before { content: "> "; color: var(--muted); }
.status { margin-left: auto; color: var(--green); }
main { padding: 24px 0 40px; overflow-wrap: anywhere; }
footer {
  display: flex;
  justify-content: space-between;
  padding: 12px 0 24px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 10px;
}
h1 { max-width: 820px; margin: 5px 0 10px; font-size: clamp(20px, 3vw, 30px); line-height: 1.18; letter-spacing: -.025em; }
p { margin: 8px 0; }
.lede { max-width: 700px; color: #b3beb5; font-size: 14px; }
.prompt, .role, .map-head { color: var(--green); font-weight: 750; }
.muted { color: var(--muted); }
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
.meta { margin-bottom: 12px; padding-bottom: 13px; border-bottom: 1px solid var(--line); }
.thread { border-top: 1px solid var(--line); }
.call-break { display: flex; align-items: center; gap: 8px; height: 24px; color: var(--muted); font-size: 9px; text-transform: uppercase; }
.call-break::before, .call-break::after { height: 1px; background: var(--line); content: ""; }
.call-break::before { width: 10px; }
.call-break::after { flex: 1; }
.call-break span { color: var(--amber); }
.message {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr);
  margin: 0;
  padding: 11px 12px 12px 0;
  border-bottom: 1px solid var(--line);
  scroll-margin-top: 52px;
}
.message.user { background: #0c1510; }
.message:target { box-shadow: inset 2px 0 var(--green); }
.message p { min-width: 0; margin: 0; white-space: pre-wrap; }
.role { padding-left: 10px; font-size: 10px; letter-spacing: .04em; text-transform: uppercase; }
.role span { display: inline-block; width: 26px; color: var(--muted); }
.message.assistant .role { color: var(--amber); }
.message.tool, .message.file { min-height: 28px; padding-block: 5px; color: var(--muted); background: #0a0f0c; }
.message.tool .role { color: #68cce8; }
.message.file .role { color: #c6a4ef; }
.message.tool p::before { content: "$ tool "; color: #68cce8; }
.message.file p::before { content: "~ "; color: #c6a4ef; }
.minimap {
  position: fixed;
  top: 58px;
  width: 96px;
  margin-left: min(944px, calc(100vw - 132px));
  padding-left: 10px;
  border-left: 1px solid var(--line);
  font-size: 9px;
}
.map-head { display: flex; justify-content: space-between; margin-bottom: 6px; text-transform: uppercase; }
.map-head span { color: var(--muted); }
.minimap ol { display: flex; flex-direction: column; height: calc(100vh - 112px); margin: 0; padding: 0 3px 0 0; list-style: none; }
.minimap li { flex: 1 1 1px; min-height: 1px; max-height: 9px; }
.map-marker { display: block; width: 28px; height: 100%; min-height: 1px; border-top: 1px solid #425047; text-decoration: none; }
.map-marker.user { width: 76px; min-height: 2px; border-color: var(--green); }
.map-marker.assistant { border-color: var(--amber); opacity: .72; }
.map-marker.tool { border-color: #68cce8; }
.map-marker.file { border-color: #c6a4ef; }
.map-marker:hover, .map-marker:focus-visible { width: 76px; opacity: 1; }
.map-key { display: flex; gap: 5px; align-items: center; margin-top: 7px; color: var(--muted); }
.map-key i { display: inline-block; width: 9px; height: 2px; margin-left: 3px; background: var(--green); }
.map-key .tool-key { background: #68cce8; }
.map-key .file-key { background: #c6a4ef; }
a:hover { color: #a3f6c1; }
:focus-visible { outline: 2px solid var(--amber); outline-offset: 2px; }
@media (max-width: 720px) {
  body { padding-inline: 12px; }
  header { gap: 8px; }
  header span:not(.status) { display: none; }
  main { padding-top: 16px; }
  .facts { grid-template-columns: 1fr; }
  .viewer { padding-right: 32px; }
  .message { grid-template-columns: 66px minmax(0, 1fr); padding-right: 4px; }
  .role { padding-left: 4px; }
  .minimap { right: 5px; width: 23px; margin-left: 0; padding-left: 5px; }
  .map-head, .map-key { display: none; }
  .minimap ol { height: calc(100vh - 76px); }
  .map-marker { width: 8px; }
  .map-marker.user, .map-marker:hover, .map-marker:focus-visible { width: 17px; }
}
@media (prefers-reduced-motion: reduce) { html { scroll-behavior: auto; } }
`

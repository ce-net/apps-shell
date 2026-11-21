// The Conversations tab (repo:ce/docs/AI-AS-CELLS-2026-11-18.md section 6, slice 2: read only). Two UI cells side by
// side: conv.list (left: every conversation of the viewer's realm, from ai.session List) and conv.thread (centre: one
// conversation's turns, ai.session Get + Read). On a phone one at a time, Back returns to the list. Every call goes
// through the shell's relay to ai.session as the viewer; ai.session's gate and tenant rule decide what comes back.
import { mountList } from './conv-list.js';
import { mountThread } from './conv-thread.js';

// `also`: the other cell this layout talks to through the shell's relay (the usage strip, conv.usage)
export const view = { cell: 'ai.session', also: ['conv.usage'], name: 'Conversations', speaks: [1], kit: 1 };

const CSS = `
.cv-usage{background:var(--bg)}
@media (min-width: 761px){.cv-usage{position:sticky;top:0;z-index:2}}
.cv-usage:empty{display:none}
.cv{display:grid;grid-template-columns:minmax(300px,380px) 1fr;gap:0;min-height:calc(100vh - 120px);border:1px solid var(--line);border-radius:var(--r);background:var(--surface);overflow:hidden}
.cv-l{border-right:1px solid var(--line);display:flex;flex-direction:column;min-width:0}
.cv-t{display:flex;flex-direction:column;min-width:0}
.cv-head{padding:var(--s-3) var(--s-4);border-bottom:1px solid var(--line);display:flex;flex-direction:column;gap:var(--s-2)}
.cv-title{display:flex;align-items:baseline;gap:var(--s-2);justify-content:space-between}
.cv-title h2{margin:0;font-size:var(--t-lg)}
.cv-n{font-family:var(--mono);color:var(--ink-3);font-size:var(--t-sm)}
.cv-q{width:100%;box-sizing:border-box;padding:var(--s-2) var(--s-3);border:1px solid var(--line);border-radius:var(--r-sm);background:var(--well);color:var(--ink);font:inherit}
.cv-chips{display:flex;flex-wrap:wrap;gap:var(--s-1)}
.cv-chip{border:1px solid var(--line);background:transparent;color:var(--ink-2);border-radius:var(--r-pill);padding:2px var(--s-2);font:inherit;font-size:var(--t-sm);cursor:pointer;min-height:28px}
.cv-chip[aria-pressed=true]{background:var(--acc-soft);color:var(--acc-ink);border-color:var(--acc)}
.cv-count{font-family:var(--mono);font-size:var(--t-xs);color:var(--ink-3);display:flex;flex-wrap:wrap;gap:var(--s-1) var(--s-3)}
.cv-rows{overflow:auto;flex:1}
.cv-group{padding:var(--s-2) var(--s-4) var(--s-1);font-size:var(--t-xs);letter-spacing:var(--track-label);text-transform:uppercase;color:var(--ink-3)}
.cv-row{display:block;width:100%;text-align:left;border:0;border-top:1px solid var(--line);background:transparent;color:var(--ink);padding:var(--s-2) var(--s-4);font:inherit;cursor:pointer}
.cv-row:hover{background:var(--hover)}
.cv-row[aria-current=true]{background:var(--acc-soft)}
.cv-row b{display:block;font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.cv-meta{display:flex;gap:var(--s-2);font-size:var(--t-xs);color:var(--ink-3);overflow:hidden;white-space:nowrap}
.cv-dot{display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--faint);margin-right:6px;vertical-align:middle}
.cv-dot.live{background:var(--live)}
.cv-more{margin:var(--s-3) var(--s-4);color:var(--ink-3);font-size:var(--t-sm)}
.cv-th{padding:var(--s-3) var(--s-4);border-bottom:1px solid var(--line)}
.cv-th h2{margin:0 0 var(--s-1);font-size:var(--t-lg);overflow-wrap:anywhere}
.cv-turns{overflow:auto;flex:1;padding:var(--s-2) var(--s-4) var(--s-5)}
.cv-turn{padding:var(--s-2) 0;border-bottom:1px solid var(--line)}
.cv-who{font-size:var(--t-xs);color:var(--ink-3);display:flex;gap:var(--s-2)}
.cv-who b{color:var(--ink-2);font-weight:600}
.cv-text{white-space:pre-wrap;overflow-wrap:anywhere;margin:var(--s-1) 0 0;font-size:var(--t-sm);line-height:1.5}
.cv-tool{font-family:var(--mono);font-size:var(--t-xs);color:var(--ink-2);margin-top:var(--s-1);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.cv-tool.bad{color:var(--bad)}
.cv-empty{padding:var(--s-6) var(--s-4);color:var(--ink-3)}
.cv-back{display:none}
.cv-btn{border:1px solid var(--line);background:transparent;color:var(--ink);border-radius:var(--r-sm);padding:var(--s-1) var(--s-3);font:inherit;cursor:pointer;min-height:36px}
@media (max-width: 760px){
 .cv{grid-template-columns:1fr;border-radius:0;border-left:0;border-right:0}
 .cv-l{border-right:0}
 .cv.open .cv-l{display:none}
 .cv:not(.open) .cv-t{display:none}
 .cv-back{display:inline-block;margin-bottom:var(--s-2)}
}`;

let sheet = null;
function style() {
  if (sheet) return;
  try {
    sheet = new CSSStyleSheet();
    sheet.replaceSync(CSS);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  } catch (_) { sheet = {}; }
}

export function mount(root, host) {
  style();
  const { h } = host;
  const left = h('section', { class: 'cv-l', 'aria-label': 'Conversations' });
  const centre = h('section', { class: 'cv-t', 'aria-label': 'Conversation' });
  const box = h('div', { class: 'cv' }, left, centre);
  // the usage strip along the top, always visible: its module is served only to a viewer holding ai.usage:read
  const strip = h('div', { class: 'cv-usage' });
  root.replaceChildren(strip, box);
  let stopStrip = () => {};
  import('./conv-usage.js').then(m => { if (!gone) stopStrip = m.mountStrip(strip, host); }).catch(() => { strip.remove(); });
  let gone = false;
  let stopThread = () => {};
  const list = mountList(left, host, {
    onOpen(c) {
      box.classList.add('open');
      stopThread();
      stopThread = mountThread(centre, host, c, { onBack() { box.classList.remove('open'); list.focus(c.id); } });
    },
  });
  stopThread = mountThread(centre, host, null, {});
  return () => { gone = true; stopStrip(); list.stop(); stopThread(); };
}

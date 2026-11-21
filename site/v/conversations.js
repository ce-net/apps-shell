// The Conversations tab (repo:ce/docs/AI-AS-CELLS-2026-11-18.md section 6, slice 2 reading, section 12 slice 3 talking). Two UI cells side by
// side: conv.list (left: every conversation of the viewer's realm, from ai.session List) and conv.thread (centre: one
// conversation's turns, ai.session Get + Read). On a phone one at a time, Back returns to the list. Every call goes
// through the shell's relay to ai.session as the viewer; ai.session's gate and tenant rule decide what comes back.
import { mountList } from './conv-list.js';
import { mountThread } from './conv-thread.js';

// `also`: the other cells this layout talks to through the shell's relay (the usage strip conv.usage; term.session for
// a tmux row opened live)
export const view = { cell: 'ai.session', also: ['conv.usage', 'term.session'], name: 'Conversations', speaks: [1], kit: 1 };

const CSS = `
.cv-usage{background:var(--bg)}
@media (min-width: 761px){.cv-usage{position:sticky;top:0;z-index:2}}
.cv-usage:empty{display:none}
.cv{display:grid;grid-template-columns:minmax(300px,380px) 1fr;gap:0;min-height:calc(100vh - 120px);border:1px solid var(--line);border-radius:var(--r);background:var(--surface);overflow:hidden}
.cv-l{border-right:1px solid var(--line);display:flex;flex-direction:column;min-width:0;min-height:0}
.cv-t{display:flex;flex-direction:column;min-width:0;min-height:0}
@media (min-width: 761px){.cv{height:calc(100vh - 120px)}}
.cv-head{padding:var(--s-3) var(--s-4);border-bottom:1px solid var(--line);display:flex;flex-direction:column;gap:var(--s-2)}
.cv-title{display:flex;align-items:baseline;gap:var(--s-2);justify-content:space-between}
.cv-title h2{margin:0;font-size:var(--t-lg)}
.cv-n{font-family:var(--mono);color:var(--ink-3);font-size:var(--t-sm)}
.cv-q{width:100%;box-sizing:border-box;padding:var(--s-2) var(--s-3);border:1px solid var(--line);border-radius:var(--r-sm);background:var(--well);color:var(--ink);font:inherit}
.cv-chips{display:flex;flex-wrap:wrap;gap:var(--s-1)}
.cv-chip{border:1px solid var(--line);background:transparent;color:var(--ink-2);border-radius:var(--r-pill);padding:2px var(--s-2);font:inherit;font-size:var(--t-sm);cursor:pointer;min-height:28px}
.cv-chip[aria-pressed=true]{background:var(--acc-soft);color:var(--acc-ink);border-color:var(--acc)}
.cv-count{font-family:var(--mono);font-size:var(--t-xs);color:var(--ink-3);display:flex;flex-wrap:wrap;gap:var(--s-1) var(--s-3)}
.cv-nodes{font-family:var(--mono);font-size:var(--t-xs);color:var(--ink-3);width:100%}
.cv-nodes summary{cursor:pointer;display:flex;justify-content:space-between;gap:var(--s-2);min-height:28px;align-items:center}
.cv-nline{border-top:1px solid var(--line)}
.cv-node{display:grid;grid-template-columns:minmax(0,1fr) auto auto;gap:var(--s-2);align-items:baseline;width:100%;border:0;background:transparent;color:var(--ink-2);font:inherit;text-align:left;padding:var(--s-1) 0;cursor:pointer;min-height:28px}
.cv-node>span:first-child{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.cv-node[aria-pressed=true]{color:var(--acc-ink)}
.cv-node[data-mark=gap] .cv-nst,.cv-node[data-mark=stale] .cv-nst{color:var(--warn)}
.cv-node[data-mark=off] .cv-nst{color:var(--bad)}
.cv-node[data-mark=none]{color:var(--ink-3)}
.cv-src{display:flex;flex-wrap:wrap;gap:var(--s-1);padding:0 0 var(--s-1) var(--s-3)}
.cv-src .cv-chip{font-size:var(--t-xs);min-height:24px}
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
.cv-btn.primary{background:var(--acc);border-color:var(--acc);color:var(--on-acc, #fff)}
.cv-btn:disabled{opacity:.5;cursor:default}
.cv-compose{border-top:1px solid var(--line);padding:var(--s-2) var(--s-4) var(--s-3);display:flex;flex-direction:column;gap:var(--s-2);background:var(--surface)}
.cv-bar{display:flex;flex-direction:column;gap:var(--s-2)}
.cv-acts{display:flex;flex-wrap:wrap;gap:var(--s-2)}
.cv-in{width:100%;box-sizing:border-box;resize:vertical;min-height:44px;padding:var(--s-2) var(--s-3);border:1px solid var(--line);border-radius:var(--r-sm);background:var(--well);color:var(--ink);font:inherit}
.cv-note{margin:0;font-size:var(--t-xs);color:var(--ink-3)}
.cv-note:empty{display:none}
.cv-draft .cv-text{color:var(--ink-2)}
@media (max-width: 760px){
 .cv{grid-template-columns:1fr;border-radius:0;border-left:0;border-right:0}
 .cv-l{border-right:0}
 .cv.open .cv-l{display:none}
 .cv:not(.open) .cv-t{display:none}
 .cv-back{display:inline-block;margin-bottom:var(--s-2)}
 .cv-compose{position:sticky;bottom:0}
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
  // a new chat, a fork or a continued row opens in the centre; the list refreshes to show it
  const openId = async id => {
    let c;
    try { c = await host.call('Get', { conversation: id }); } catch (_) { return; }
    if (gone) return;
    list.refresh();
    show(c);
  };
  const show = c => {
    box.classList.add('open');
    stopThread();
    stopThread = mountThread(centre, host, c, { onBack() { box.classList.remove('open'); list.focus(c.id); }, onOpened: openId });
  };
  const list = mountList(left, host, { onOpen: show, onOpened: openId });
  stopThread = mountThread(centre, host, null, {});
  // /conversations/?c=<id> (a Search row) opens that conversation, read as the viewer like any row of the list
  const c0 = new URLSearchParams(location.search).get('c');
  if (c0 && /^[A-Za-z0-9._-]{1,128}$/.test(c0)) openId(c0);
  return () => { gone = true; stopStrip(); list.stop(); stopThread(); };
}

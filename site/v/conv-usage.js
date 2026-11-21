// conv.usage (UI cell, the strip along the top of the Conversations tab): what every subscription, API key and this
// device has left, from ai.usage (repo:ce docs/AI-USAGE-AND-DEVICES-2026-11-18.md section 3).
//
// V2 shape: the Rust types in crates/apps-shell/src/strip.rs (generated TypeScript: crates/apps-shell/gen/ConvUsage.d.ts)
// are the contract, and the shell builds the typed Strip from ai.usage's Report as the viewer. This module only paints
// what it is given: it computes no percent, no colour state and no time, and it never invents a number (a window with
// no reading says "unreadable: <why>"). Data: StripOp once, then WaitOp (ai.usage Events, held open up to 25 s) tells
// it to read again; it also re-reads every 60 s because a percentage moves without an event.
//
// @ts-check
/** @typedef {import('../../crates/apps-shell/gen/ConvUsage').Strip} Strip */
/** @typedef {import('../../crates/apps-shell/gen/ConvUsage').Gauge} Gauge */
/** @typedef {import('../../crates/apps-shell/gen/ConvUsage').ConvCost} ConvCost */
export const view = { cell: 'conv.usage', name: 'conv.usage', speaks: [1], kit: 1 };

const CSS = `
.us{border:1px solid var(--line);border-radius:var(--r);background:var(--surface);padding:var(--s-2) var(--s-4);margin-bottom:var(--s-3);display:flex;flex-wrap:wrap;gap:var(--s-2) var(--s-5);align-items:flex-start;min-width:0;position:relative}
.us-acct,.us-keys,.us-dev{display:flex;flex-direction:column;gap:2px;min-width:0;max-width:100%}
.us-acct{min-width:200px}
.us-h{display:flex;align-items:center;gap:var(--s-2);flex-wrap:wrap;min-width:0}
.us-label{font-weight:600;font-size:var(--t-sm);color:var(--ink);overflow-wrap:anywhere}
.us-plan{font-size:var(--t-xs);color:var(--ink-3)}
.us-g{display:grid;grid-template-columns:2.2em 72px 3.6em minmax(0,1fr);align-items:center;gap:var(--s-2);font-size:var(--t-xs);color:var(--ink-2)}
.us-n{font-family:var(--mono);color:var(--ink-3)}
.us-v{font-family:var(--mono);text-align:right;font-variant-numeric:tabular-nums;color:var(--ink)}
.us-note{color:var(--ink-3);overflow-wrap:anywhere;min-width:0}
.us-bar{appearance:none;-webkit-appearance:none;border:0;display:block;width:100%;height:4px;border-radius:2px;background:var(--well);overflow:hidden}
.us-bar::-webkit-progress-bar{background:var(--well);border-radius:2px}
.us-bar::-webkit-progress-value{background:var(--ok);border-radius:2px}
.us-bar::-moz-progress-bar{background:var(--ok);border-radius:2px}
.us-g[data-state=warn] .us-bar::-webkit-progress-value{background:var(--warn)}
.us-g[data-state=warn] .us-bar::-moz-progress-bar{background:var(--warn)}
.us-g[data-state=bad] .us-bar::-webkit-progress-value,.us-g[data-state=capped] .us-bar::-webkit-progress-value{background:var(--bad)}
.us-g[data-state=bad] .us-bar::-moz-progress-bar,.us-g[data-state=capped] .us-bar::-moz-progress-bar{background:var(--bad)}
.us-g[data-state=warn] .us-v{color:var(--warn)}
.us-g[data-state=bad] .us-v,.us-g[data-state=capped] .us-v,.us-g[data-state=capped] .us-note,.us-g[data-state=bad] .us-note{color:var(--bad)}
.us-g[data-state=off] .us-bar,.us-g[data-state=unreadable] .us-bar{opacity:.35}
.us-g[data-state=off] .us-note,.us-g[data-state=unreadable] .us-note{color:var(--ink-3);font-style:italic}
.us-g[data-stale=true] .us-v,.us-g[data-stale=true] .us-note{opacity:.7}
.us-chips{display:flex;flex-wrap:wrap;gap:var(--s-1)}
.us-chip{display:inline-flex;gap:var(--s-1);align-items:baseline;border:1px solid var(--line);border-radius:var(--r-pill);padding:0 var(--s-2);font-size:var(--t-xs);color:var(--ink-2);line-height:18px;white-space:nowrap}
.us-chip b{font-weight:500}.us-chip span{font-family:var(--mono);font-variant-numeric:tabular-nums}
.us-chip[data-state=warn] span{color:var(--warn)}.us-chip[data-state=bad] span,.us-chip[data-state=capped] span{color:var(--bad)}
.us-lock{display:inline-flex;align-items:center;gap:4px;color:var(--bad);font-size:var(--t-xs);font-weight:500}
.us-lock svg{width:11px;height:11px}
.us-stale{border:1px solid var(--warn);color:var(--warn);border-radius:var(--r-pill);padding:0 var(--s-2);font-size:var(--t-2xs);line-height:16px}
.us-acct[data-state=lapsed] .us-label,.us-acct[data-state=unreadable] .us-label{color:var(--ink-3)}
.us-key{display:flex;gap:var(--s-2);align-items:baseline;font-size:var(--t-xs);color:var(--ink-2);min-width:0}
.us-key b{font-weight:500;overflow-wrap:anywhere}
.us-key span{font-family:var(--mono);margin-left:auto;text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.us-dev .us-h{font-size:var(--t-xs)}
.us-dot{display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--faint);flex:none}
.us-dot[data-on=true]{background:var(--live)}
.us-sub{font-size:var(--t-xs);color:var(--ink-3);overflow-wrap:anywhere}
.us-conn{position:absolute;top:var(--s-2);right:var(--s-3);display:flex;gap:6px;align-items:center;font-size:var(--t-2xs);color:var(--ink-3)}
.us-conn i{display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--faint)}
.us-conn[data-s=live] i{background:var(--live)}
.us-conn[data-s=finding] i,.us-conn[data-s=reconnecting] i{background:var(--warn)}
.us-conn[data-s=stale] i{background:var(--warn);opacity:.6}
.us-conn[data-s=refused] i,.us-conn[data-s=failed] i{background:var(--bad)}
.us-msg{font-size:var(--t-xs);color:var(--ink-3)}
.cv-cost{font-family:var(--mono);font-variant-numeric:tabular-nums;white-space:nowrap}
@media (max-width:760px){
 .us{padding:var(--s-2) 16px;gap:var(--s-3);flex-direction:column;align-items:stretch}
 .us-acct{min-width:0}
 .us-g{grid-template-columns:2.2em 56px 3.4em minmax(0,1fr)}
 .us-conn{position:static;margin-left:auto}
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

const SVG = 'http://www.w3.org/2000/svg';
/** a small padlock (drawn, not an emoji) */
function lockIcon() {
  const s = document.createElementNS(SVG, 'svg');
  s.setAttribute('viewBox', '0 0 12 12'); s.setAttribute('fill', 'none'); s.setAttribute('stroke', 'currentColor'); s.setAttribute('stroke-width', '1.4'); s.setAttribute('aria-hidden', 'true');
  const r = document.createElementNS(SVG, 'rect'); r.setAttribute('x', '2'); r.setAttribute('y', '5.5'); r.setAttribute('width', '8'); r.setAttribute('height', '5.5');
  const p = document.createElementNS(SVG, 'path'); p.setAttribute('d', 'M4 5.5V4a2 2 0 0 1 4 0v1.5');
  s.append(r, p);
  return s;
}

/** one bar row: name, 4 px bar, right-aligned percent, the note */
function gaugeRow(h, g) {
  const bar = h('progress', { class: 'us-bar', max: '100', value: String(g.bar), 'aria-label': g.name + ' ' + (g.value || g.note) });
  return h('div', { class: 'us-g', 'data-state': g.state, 'data-stale': g.stale ? 'true' : 'false', 'data-gauge': g.name },
    h('span', { class: 'us-n' }, g.name), bar, h('span', { class: 'us-v' }, g.value), h('span', { class: 'us-note' }, g.note));
}

/** the strip as DOM, from a typed Strip (the same function paints the golden test) */
export function paint(h, strip) {
  const out = [];
  for (const a of strip.accounts) {
    const head = h('div', { class: 'us-h' }, h('span', { class: 'us-label' }, a.label), a.plan ? h('span', { class: 'us-plan' }, a.plan) : null,
      a.lock ? h('span', { class: 'us-lock', 'data-lock': a.state }, lockIcon(), a.lock) : null,
      a.badge ? h('span', { class: 'us-stale', 'data-stale-badge': 'true' }, a.badge) : null);
    const rows = [a.five, a.seven, a.day].filter(Boolean).map(g => gaugeRow(h, g));
    if (a.unreadable) rows.push(h('div', { class: 'us-sub', 'data-unreadable': 'true' }, a.unreadable));
    const chips = a.chips.length ? h('div', { class: 'us-chips' }, ...a.chips.map(c =>
      h('span', { class: 'us-chip', 'data-state': c.state, title: c.tip }, h('b', {}, c.name), h('span', {}, c.value || '?')))) : null;
    out.push(h('div', { class: 'us-acct', 'data-state': a.state, 'data-account': a.label, role: 'group', 'aria-label': 'Account ' + a.label }, head, ...rows, chips));
  }
  if (strip.keys.length) {
    out.push(h('div', { class: 'us-keys', 'aria-label': 'API keys' },
      h('div', { class: 'us-h' }, h('span', { class: 'us-label' }, 'API keys'),
        strip.keys_today ? h('span', { class: 'us-plan', title: 'every key, today' }, 'today ' + strip.keys_today) : null),
      ...strip.keys.map(k => h('div', { class: 'us-key', 'data-key': k.key_ref, title: k.tip },
        h('b', {}, k.key_ref), h('span', {}, (k.today ? k.today + ' / ' : '') + k.month + ' month'), k.stale ? h('i', { class: 'us-stale' }, 'stale') : null))));
  }
  if (strip.device) {
    const d = strip.device;
    out.push(h('div', { class: 'us-dev', 'data-device': d.name, title: d.tip },
      h('div', { class: 'us-h' }, h('span', { class: 'us-dot', 'data-on': d.online ? 'true' : 'false' }), h('span', { class: 'us-label' }, d.name),
        h('span', { class: 'us-plan' }, d.class + ', ' + (d.online ? 'online' : 'offline'))),
      h('div', { class: 'us-sub' }, d.logins.length ? 'logged in: ' + d.logins.join(', ') : 'no account logged in')));
  }
  if (!out.length) out.push(h('p', { class: 'us-msg' }, 'No subscription, key or device is reported yet.'));
  return out;
}

const CONN = {
  idle: 'idle', finding: 'finding ai.usage', live: 'live', stale: 'stale: no answer for 3 min',
  reconnecting: 'reconnecting', refused: 'not open to you', failed: 'failed',
};

/** `tokens . $cost . time` cache for conv.list rows and the conv.thread header: ids -> ConvCost */
const costs = new Map();
const wanted = new Set();
let flushT = 0;
const listeners = new Set();
export function costOf(id) { return costs.get(id) || null; }
export function onCosts(f) { listeners.add(f); return () => listeners.delete(f); }
let costHost = null;
export function useCostHost(host) { costHost = host; }
/** ask for these conversation ids' cost lines (batched, at most 100 a call, each id once per 5 min) */
export function wantCosts(ids) {
  const now = Date.now();
  for (const id of ids) { const c = costs.get(id); if (!c || now - c.at > 300000) wanted.add(id); }
  if (!wanted.size || flushT || !costHost) return;
  flushT = setTimeout(async () => {
    flushT = 0;
    const ids = [...wanted].slice(0, 100);
    ids.forEach(i => wanted.delete(i));
    try {
      const r = await costHost.call('CostsOp', { ids });
      for (const row of r.rows || []) costs.set(row.id, { ...row, at: Date.now() });
      listeners.forEach(f => { try { f(); } catch (_) {} });
    } catch (_) { /* the strip says why; a row just shows no cost line */ }
    if (wanted.size) wantCosts([]);
  }, 120);
}

/** mount the strip: `host.other['conv.usage']` (a Conversations host that declared it) or `host` itself (mounted alone) */
export function mountStrip(root, host) {
  style();
  const { h } = host;
  const cell = (host.other && host.other['conv.usage']) || host;
  useCostHost(cell);
  const dot = h('span', { class: 'us-conn', 'data-s': 'idle', role: 'status', 'aria-live': 'polite' }, h('i', {}), h('span', {}, CONN.idle));
  const box = h('section', { class: 'us', 'aria-label': 'Usage' });
  root.replaceChildren(box);
  let stopped = false, since = null, lastRead = 0, lastOk = 0, fails = 0, shown = false;
  const conn = s => { dot.dataset.s = s; dot.lastChild.textContent = CONN[s]; dot.title = CONN[s]; };
  const draw = strip => {
    box.replaceChildren(...paint(h, strip), dot);
    shown = true;
  };
  conn('idle');
  const sleep = ms => new Promise(r => setTimeout(r, ms));

  async function read() {
    const strip = await cell.call('StripOp', { device: null });
    lastRead = lastOk = Date.now();
    since = strip.at;
    draw(strip);
    conn('live');
  }
  async function loop() {
    conn('finding');
    while (!stopped) {
      try {
        if (!shown || Date.now() - lastRead > 60000) { await read(); fails = 0; }
        const w = await cell.call('WaitOp', { since, wait_ms: 25000 });
        lastOk = Date.now(); fails = 0;
        if (w.changed) { if (w.at) since = w.at; await read(); }
        else conn('live');
      } catch (e) {
        if (stopped) return;
        if (e && e.refused) {
          conn('refused');
          if (!shown) box.replaceChildren(h('p', { class: 'us-msg' }, 'Usage is not open to you.'), dot);
          return;
        }
        fails++;
        const why = (e && e.message) || 'no answer';
        if (!shown) {
          box.replaceChildren(h('p', { class: 'us-msg', 'data-unreadable': 'true' }, 'unreadable: ' + why), dot);
          conn(fails >= 3 ? 'failed' : 'reconnecting');
        } else conn(Date.now() - lastOk > 180000 ? 'stale' : 'reconnecting');
        await sleep(Math.min(1000 * 2 ** Math.min(fails, 5), 30000));
      }
    }
  }
  box.replaceChildren(dot);
  loop();
  return () => { stopped = true; };
}

export function mount(root, host) { return mountStrip(root, host); }

// apps.shell's page: the tab bar is whatever tabs.json says this viewer has now (allowed by their chain, and a
// replica of the tab's shell answering). Polled every 3 s, so a grant, an end or a last replica going down changes
// the bar on an open page without a reload. This file names no tab and no system.
//
// It is also the ce-net browser (repo:ce/docs/CE-REF-OPENERS-AND-HOME-2026-11-18.md section 4): every link is a
// ce-ref, opened by host.open through open.with in the Pane (side on a desk, full screen on a phone). The Bar takes a
// ce-ref (short or ce-ref://), a cell path or words, and shows the short ref of what the pane shows; Copy gives the
// full ce-ref://. The page address /<tab>/o/<ref> is this host's transport only. Views get the V2 5.1 surface (h,
// call, watch, slot, emit, open) and never a URL, a fetch or a token. The one path to the outside web is
// handoff(), reached only from the Outside view after it said "outside ce-net".
import { CeRef } from './ref.js';

const h = (t, a = {}, ...kids) => {
  const e = document.createElement(t);
  for (const [k, v] of Object.entries(a)) {
    if (k === 'class') e.className = v; else if (k.startsWith('on')) e.addEventListener(k.slice(2), v); else if (v !== false && v != null) e.setAttribute(k, v === true ? '' : v);
  }
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k.nodeType ? k : document.createTextNode(String(k)));
  return e;
};
window.ceh = h;

const root = document.documentElement;
const themeBtn = document.getElementById('theme');
function setTheme(t) {
  if (t === 'light') root.setAttribute('data-theme', 'light'); else root.removeAttribute('data-theme');
  themeBtn.textContent = t === 'light' ? 'Dark' : 'Light';
  try { localStorage.setItem('ce-theme', t); } catch (_) {}
}
let saved = 'dark';
try { saved = localStorage.getItem('ce-theme') || 'dark'; } catch (_) {}
setTheme(saved);
themeBtn.addEventListener('click', () => setTheme(root.getAttribute('data-theme') === 'light' ? 'dark' : 'light'));

const rail = document.getElementById('tabs');
const main = document.getElementById('main');
const frame = document.querySelector('.frame');
let mounted = null; // {view, stop}
let last = '';
let here = '/';          // this face's tab route, from tabs.json
let tabsNow = [];

const device = () => innerWidth < 700 ? 'Phone' : innerWidth > 1800 ? 'Wall' : 'Desk';

// ---- transport: one GET per op of the view's own cell, under the viewer's session. Views never see this. ----
const TIMEOUT_MS = 25000;
async function get(op, req = {}) {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(req)) if (v != null && v !== '') q.set(k, String(v));
  const ctl = new AbortController();
  const t = setTimeout(() => ctl.abort(), TIMEOUT_MS);
  try {
    const r = await fetch('api/' + op + (q.toString() ? '?' + q : ''), { cache: 'no-store', credentials: 'same-origin', signal: ctl.signal });
    const data = await r.json().catch(() => ({}));
    return { status: r.status, ok: r.ok, data };
  } catch (e) {
    return { status: 0, ok: false, data: { error: ctl.signal.aborted ? 'no answer in 25 s' : 'not reachable' }, timeout: ctl.signal.aborted };
  } finally { clearTimeout(t); }
}

// the same GET as JSON lines as the answer grows (stream=1): onPart(data) per line; resolves like get() with the last
async function getParts(op, req, onPart) {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(req || {})) if (v != null && v !== '') q.set(k, String(v));
  q.set('stream', '1');
  const ctl = new AbortController();
  const t = setTimeout(() => ctl.abort(), TIMEOUT_MS);
  try {
    const r = await fetch('api/' + op + '?' + q, { cache: 'no-store', credentials: 'same-origin', signal: ctl.signal });
    if (!r.ok || !r.body) return { status: r.status, ok: false, data: await r.json().catch(() => ({})) };
    const rd = r.body.getReader(), dec = new TextDecoder();
    let buf = '', last = null;
    for (;;) {
      const { value, done } = await rd.read();
      if (value) buf += dec.decode(value, { stream: true });
      let i;
      while ((i = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, i); buf = buf.slice(i + 1);
        if (!line.trim()) continue;
        try { last = JSON.parse(line); } catch (_) { continue; }
        onPart(last);
      }
      if (done) break;
    }
    return last ? { status: 200, ok: true, data: last } : { status: 502, ok: false, data: { error: 'no answer' } };
  } catch (e) {
    return { status: 0, ok: false, data: { error: ctl.signal.aborted ? 'no answer in 25 s' : 'not reachable' }, timeout: ctl.signal.aborted };
  } finally { clearTimeout(t); }
}

// ---- the views the shell can mount: view name -> module and the ops of its own cell it may call ----
const VIEWS = {
  'ref.view::Text': ['v/ref-text.js', ['ref']],
  'ref.view::Raw': ['v/ref-raw.js', ['ref']],
  'ref.view::Record': ['v/ref-record.js', ['rec']],
  'site.cellmap::Cell': ['v/cell.js', ['cellmap']],
  'site.cellmap::Node': ['v/cell.js', ['cellmap']],
  'site.cellmap::Summary': ['v/cells-summary.js', ['cellmap']],
  'apps.shell::Port': ['v/port.js', []],
  'apps.shell::Outside': ['v/outside.js', []],
  'apps.shell::Mount': ['v/mount.js', ['inbox', 'card']],
  'home::CallsToday': ['v/calls-today.js', ['calls']],
  'home::PromisesDue': ['v/promises-due.js', ['promises']],
  'approve.inbox::Pending': ['v/pending.js', ['inbox']],
  'home::Links': ['v/links.js', ['links']],
};
// a tab view's own ops (the tab's routes on this face)
const TAB_OPS = { 'v/home.js': ['layout'], 'v/search.js': ['search'], 'v/core.js': ['cellmap', 'inbox'], 'v/approvals.js': ['decisions', 'inbox', 'card'] };

const STATES = { live: '', asof: 'as of', pinned: 'pinned', refused: 'not open to you', unknown: 'unknown', outside: 'outside ce-net' };
const KIND_MARK = { repo: 'file', doc: 'doc', person: 'person', promise: 'promise', call: 'call', meet: 'recording', cell: 'cell', node: 'node',
  assembly: '3d', map: 'map', web: 'web', view: 'view', op: 'op', contract: 'contract', part: 'part', media: 'media', blob: 'blob' };

// ---- ui-kit: RefChip, SlotFrame ----
function refChip(ref, opts = {}) {
  const r = typeof ref === 'string' ? CeRef.parse(ref) : ref;
  if (!r) return h('span', { class: 'chip', 'data-state': 'unknown', title: 'not a ce-ref: ' + ref }, String(ref));
  const state = opts.state || (r.kind === 'web' ? 'outside' : r.pin ? 'pinned' : 'live');
  const title = opts.title || CeRef.title(r);
  const el = h('button', { type: 'button', class: 'chip' + (opts.block ? ' block' : ''), 'data-state': state, 'data-ref': CeRef.short(r),
      title: CeRef.short(r) + (STATES[state] ? ' · ' + STATES[state] : ''),
      onclick: e => { e.preventDefault(); open(CeRef.short(r)); } },
    h('i', { class: 'k' }, KIND_MARK[r.kind] || r.kind),
    h('span', { class: 't' }, title),
    opts.sub ? h('span', { class: 's' }, opts.sub) : null,
    STATES[state] && state !== 'live' ? h('span', { class: 'st' }, state === 'asof' && opts.asOf ? 'as of ' + opts.asOf : STATES[state]) : null);
  return el;
}

function slotFrame(title) {
  const count = h('span', { class: 'cnt' });
  const line = h('div', { class: 'stline', 'aria-live': 'polite' }, 'Loading.');
  const body = h('div', { class: 'body' });
  const fold = h('button', { type: 'button', class: 'btn quiet fold', 'aria-expanded': 'true' }, 'Fold');
  const el = h('section', { class: 'glass slotframe', 'data-state': 'loading' }, h('header', {}, h('h2', {}, h('b', {}, title), count), fold), line, body);
  fold.addEventListener('click', () => { const f = el.classList.toggle('folded'); fold.setAttribute('aria-expanded', String(!f)); fold.textContent = f ? 'Show' : 'Fold'; });
  return {
    el, body,
    state(s, text) { el.dataset.state = s; line.textContent = text || ''; line.hidden = !text; },
    count(n) { count.textContent = n == null ? '' : String(n); },
  };
}

// the standard states of a view (CELL-CONTRACT): loading, empty, refusal with ask, timeout at 25 s
function answerState(frameOrRoot, res, what) {
  if (res.ok) return true;
  const say = (s, t, extra) => {
    if (frameOrRoot.state) { frameOrRoot.state(s, t); if (extra) frameOrRoot.body.replaceChildren(extra); else frameOrRoot.body.replaceChildren(); }
    else frameOrRoot.replaceChildren(h('p', { class: 'note' }, t), extra || '');
  };
  if (res.status === 403) {
    const need = res.data && res.data.need;
    say('refused', (what || 'This') + ' is not open to you' + (need ? ': it needs ' + need + '.' : '.'),
      need ? refChip('view:approve.inbox@1::Request?need=' + need, { title: 'Ask for ' + need, state: 'refused' }) : null);
  } else if (res.timeout) say('timeout', (what || 'It') + ' did not answer in 25 s; trying again.');
  else say('down', (res.data && res.data.error) || ((what || 'It') + ' is not answering (' + res.status + '); trying again.'));
  return false;
}

// ---- host: the V2 5.1 surface, per mounted view ----
function makeHost(ops, extra = {}) {
  return {
    h, kit: { RefChip: refChip, SlotFrame: slotFrame, state: answerState }, device: device(),
    call(op, req) { return ops.includes(op) ? get(op, req) : Promise.resolve({ status: 403, ok: false, data: { error: op + ' is not this view\'s op' } }); },
    // parts(op, req, onPart): the same op, its answer painted as it grows (an op that streams: search)
    parts(op, req, onPart) { return ops.includes(op) ? getParts(op, req, onPart) : Promise.resolve({ status: 403, ok: false, data: { error: op + ' is not this view\'s op' } }); },
    watch(op, req, onDelta, every = 15000) {
      let stop = false, t = 0;
      const run = async () => { if (stop) return; const r = await this.call(op, req); if (!stop) { onDelta(r); t = setTimeout(run, every); } };
      run();
      return () => { stop = true; clearTimeout(t); };
    },
    slot: slot,
    emit(event, value) { if (event === 'opened' && value) open(value); },
    open: (ref, opts) => open(ref, opts),
    ref: { parse: s => CeRef.parse(s), find: t => CeRef.find(t), show: (r, o) => refChip(r, o), short: r => CeRef.short(typeof r === 'string' ? CeRef.parse(r) : r) },
    ...extra,
  };
}

async function mountView(viewName, el, props, extra = {}) {
  const v = VIEWS[viewName];
  if (!v) { el.replaceChildren(h('p', { class: 'note' }, 'This shell does not carry ' + viewName + ' yet.')); return () => {}; }
  let mod;
  try { mod = await import('../' + v[0]); } catch (_) { el.replaceChildren(h('p', { class: 'note' }, viewName + ' is not open to you here.')); return () => {}; }
  const stop = mod.mount(el, makeHost(v[1], { props: props || {}, ...extra }));
  return typeof stop === 'function' ? stop : () => {};
}

// host.slot(slot): a SlotFrame with the slot's view mounted in it; the view gets the frame for its states.
function slot(s) {
  const f = slotFrame(s.title || s.name);
  f.el.dataset.slot = s.name;
  f.el.dataset.area = (s.area || 'Main').toLowerCase();
  const stop = mountView(s.view, f.body, s.props || {}, { frame: f, offer: s.offer || null });
  f.el.unmount = () => stop.then(fn => fn());
  return f.el;
}

// ---- the Bar ----
const bar = h('form', { class: 'bar glass', role: 'search', onsubmit: e => { e.preventDefault(); go(barIn.value); } });
const barIn = h('input', { class: 'q', name: 'r', autocomplete: 'off', spellcheck: 'false', 'aria-label': 'Open a ce-ref, a cell or search',
  placeholder: 'ce-ref, cell, or search' });
const copyBtn = h('button', { type: 'button', class: 'btn quiet', title: 'Copy the ce-ref:// of what is open', hidden: true }, 'Copy');
bar.append(h('span', { class: 'mark' }, 'ce'), barIn, copyBtn, h('button', { class: 'btn primary', type: 'submit' }, 'Open'));
document.querySelector('.top').insertBefore(bar, document.querySelector('.top .btns'));
copyBtn.addEventListener('click', async () => {
  const r = CeRef.parse(barIn.value);
  if (!r) return;
  try { await navigator.clipboard.writeText(CeRef.uri(r)); copyBtn.textContent = 'Copied'; } catch (_) { copyBtn.textContent = 'Copy failed'; }
  setTimeout(() => { copyBtn.textContent = 'Copy'; }, 1500);
});

function go(text) {
  const s = text.trim();
  if (!s) return;
  const r = CeRef.fromBar(s);
  if (r) return open(CeRef.short(r));
  // words: the Search tab, if the viewer has it (its own page under this site)
  const search = tabsNow.find(t => t.route === '/search/');
  if (search) location.assign('/search/?q=' + encodeURIComponent(s));
  else showPane(null, paneNote('Search is not open to you. Type a ce-ref (person:..., call:..., repo:...) or a cell name.'));
}

// ---- the Pane and history ----
const pane = h('aside', { class: 'pane glass', id: 'pane', hidden: true, 'aria-label': 'Opened' });
const paneHead = h('header', { class: 'phead' });
const paneBody = h('div', { class: 'pbody', 'aria-live': 'polite' });
pane.append(paneHead, paneBody);
frame.append(pane);
let paneStop = null, openSeq = 0, openNow = null;
const cache = new Map(); // short ref -> last Target answer (for when open.with does not answer)

const paneNote = (...t) => h('div', { class: 'pnote' }, ...t);

function showPane(ref, ...content) {
  if (paneStop) { paneStop(); paneStop = null; }
  pane.hidden = false;
  pane.className = 'pane glass ' + (device() === 'Phone' ? 'full' : 'side');
  frame.classList.add('with-pane');
  const back = h('button', { type: 'button', class: 'btn quiet', onclick: () => history.back() }, 'Back');
  const close = h('button', { type: 'button', class: 'btn quiet', onclick: () => closePane(true) }, 'Close');
  paneHead.replaceChildren(back, h('div', { class: 'pref' }, ref ? refChip(ref, { block: true }) : ''), close);
  paneBody.replaceChildren(...content.filter(Boolean));
}

function closePane(push) {
  if (paneStop) { paneStop(); paneStop = null; }
  pane.hidden = true; frame.classList.remove('with-pane'); openNow = null; barIn.value = ''; copyBtn.hidden = true;
  if (push) history.pushState({ o: null }, '', here);
}

function addressOf(short) { return here + 'o/' + encodeURIComponent(short); }

// host.open(ref, {mode, where}): open.with Open for this viewer, then mount the Target's view in the pane.
async function open(ref, opts = {}) {
  const r = typeof ref === 'string' ? CeRef.fromBar(ref) : ref;
  if (!r) return showPane(null, paneNote('Not a ce-ref: ', String(ref)));
  const short = CeRef.short(r);
  if (!opts.fromHistory && openNow !== short) history.pushState({ o: short }, '', addressOf(short));
  openNow = short;
  barIn.value = short; copyBtn.hidden = false;
  const my = ++openSeq;
  showPane(short, paneNote('Opening ', h('b', {}, short), ' through open.with.'));
  const res = await get('open', { r: short, mode: opts.mode || 'view', device: device() });
  if (my !== openSeq) return;
  if (!res.ok) {
    if (res.status === 403) return showPane(short, paneNote('Opening refs is not open to you here.'));
    // open.with did not answer: say so, offer the floor from what this page last knew, and keep asking
    const known = cache.get(short);
    const raw = h('button', { type: 'button', class: 'btn', onclick: async () => { paneBody.replaceChildren(); paneStop = await mountView('ref.view::Raw', paneBody, { ref: short, cached: true }); } }, 'Show raw from cache');
    showPane(short, paneNote('open.with is not answering (', (res.data && res.data.error) || res.status, '). Trying again every 3 s.'),
      known ? paneNote('Last opened in ', h('b', {}, known.view), '.') : null, raw);
    const retry = setInterval(async () => {
      if (my !== openSeq) return clearInterval(retry);
      const again = await get('open', { r: short, mode: opts.mode || 'view', device: device() });
      if (my !== openSeq) return clearInterval(retry);
      if (again.ok || again.status === 403) { clearInterval(retry); show(short, again, my); }
    }, 3000);
    paneStop = () => clearInterval(retry);
    return;
  }
  show(short, res, my);
}

async function show(short, res, my) {
  const o = res.data;
  if (o.Target) {
    const t = o.Target;
    cache.set(short, t);
    const resolved = CeRef.short(CeRef.fromWire(t.r));
    barIn.value = resolved;
    const body = h('div', { class: 'pview' });
    showPane(resolved, body, t.others ? paneNote(t.others + ' more opener' + (t.others === 1 ? '' : 's') + ' can open ' + t.ty + '.') : null);
    let props = {};
    try { props = JSON.parse(t.props || '{}'); } catch (_) {}
    const stop = await mountView(t.view, body, props, { opened: { ref: resolved, ty: t.ty, cell: t.cell, view: t.view }, handoff });
    if (my !== openSeq) { stop(); return; }
    paneStop = stop;
  } else if (o.Refused) {
    showPane(short, paneNote('This exists, and it is not open to you: it needs ', h('b', {}, o.Refused.need), '.'),
      refChip(CeRef.fromWire(o.Refused.ask), { title: 'Ask for ' + o.Refused.need, state: 'refused', block: true }));
  } else if (o.NoOpener) {
    showPane(short, paneNote('Nothing opens ', h('b', {}, o.NoOpener.ty), ' yet. A cell that declares an opener for it plugs in by one line.'));
  } else if (o.Unknown) {
    showPane(short, h('div', { class: 'chipline' }, refChip(short, { state: 'unknown' })), paneNote(o.Unknown.reason));
  } else {
    showPane(short, paneNote('open.with gave an answer this shell does not read.'));
  }
}

// the one path to the outside web: only the Outside view, after it named the site and said "outside ce-net"
function handoff(address) {
  if (!/^https?:\/\//.test(address)) return;
  const w = window.open(address, '_blank', 'noopener,noreferrer');
  if (w) w.opener = null;
}

function refInAddress() {
  const p = location.pathname;
  const i = p.indexOf('/o/');
  if (i < 0) return null;
  try { return decodeURIComponent(p.slice(i + 3)); } catch (_) { return null; }
}

addEventListener('popstate', () => {
  const r = refInAddress();
  if (r) open(r, { fromHistory: true }); else closePane(false);
});

function say(crumb, title, line, link) {
  if (mounted) { mounted.stop(); mounted = null; }
  main.replaceChildren(h('div', { class: 'crumb' }, 'ce / ', h('b', {}, crumb)), h('h1', { class: 'display' }, title),
    h('p', { class: 'sub' }, line), link ? h('p', {}, h('a', { class: 'btn primary', href: link[1] }, link[0])) : null);
}

// The host of a view that declares `view = { cell, ... }`: its data comes from that one cell, through this gateway's
// relay (`api/cell/<cell>/<Op>`, as the signed-in person; the cell's own gate decides), never from a URL the view builds.
// call(op, req) -> Promise of the answer (rejects with an Error that has .status and .refused); events(op, req, on)
// a server-sent stream of the same cell;
// watch(op, req, onDelta) long-polls (req may be a function giving the current request): onDelta(answer) for each
// answer, onDelta(null, error) for a failure, then retries with backoff; returns stop().
// open(target): through host.open of the ce-net browser (open.with, the pane); never a URL.
const sleep = ms => new Promise(r => setTimeout(r, ms));
function hostFor(view, tab, notes) {
  const host = { h, tab };
  // the notes of a view that asks for them (view.notes), for a viewer who may write them (tabs.json `notes` names the
  // cell, only then): Put, List and Close through this site's relay, as the viewer
  if (view && view.notes === true && typeof notes === 'string' && /^[a-z0-9.-]+$/.test(notes)) {
    host.notes = {
      call: async (op, req) => {
        if (!/^[A-Za-z]{1,32}$/.test(op)) throw new Error('bad op');
        let r;
        try {
          r = await fetch('api/cell/' + notes + '/' + op, { method: 'POST', headers: { 'content-type': 'application/json' }, credentials: 'same-origin', cache: 'no-store', body: JSON.stringify(req == null ? null : req) });
        } catch (e) { throw Object.assign(new Error('no connection'), { status: 0 }); }
        let body = null;
        try { body = await r.json(); } catch (_) {}
        if (!r.ok) throw Object.assign(new Error((body && body.error) || 'HTTP ' + r.status), { status: r.status, refused: r.status === 403 });
        return body;
      },
    };
  }
  if (!view || typeof view.cell !== 'string' || !/^[a-z0-9.-]+$/.test(view.cell)) return host;
  const cell = view.cell;
  const callOf = cell => async (op, req, signal) => {
    if (!/^[A-Za-z]{1,32}$/.test(op)) throw new Error('bad op');
    let r;
    try {
      r = await fetch('api/cell/' + cell + '/' + op, { method: 'POST', headers: { 'content-type': 'application/json' }, credentials: 'same-origin', cache: 'no-store', body: JSON.stringify(req == null ? null : req), signal });
    } catch (e) { if (signal && signal.aborted) throw e; throw Object.assign(new Error('no connection'), { status: 0 }); }
    let body = null;
    try { body = await r.json(); } catch (_) {}
    if (!r.ok) throw Object.assign(new Error((body && body.error) || 'HTTP ' + r.status), { status: r.status, refused: r.status === 403 });
    return body;
  };
  const call = callOf(cell);
  // `view.also`: other cells this view's layout talks to (the usage strip's conv.usage), each through the same relay
  // as the viewer; the face's relay decides which of them it passes
  host.other = {};
  for (const n of Array.isArray(view.also) ? view.also : []) {
    if (typeof n !== 'string' || !/^[a-z0-9.-]+$/.test(n)) continue;
    const c = callOf(n);
    host.other[n] = { h, call: (op, req) => c(op, req) };
  }
  host.call = (op, req) => call(op, req);
  host.watch = (op, req, onDelta) => {
    let stopped = false, ctl = null, back = 1000;
    (async () => {
      while (!stopped) {
        const t0 = Date.now();
        ctl = new AbortController();
        try {
          const d = await call(op, typeof req === 'function' ? req() : req, ctl.signal);
          back = 1000;
          if (!stopped) onDelta(d);
          const spent = Date.now() - t0;
          if (spent < 200) await sleep(200 - spent);
        } catch (e) {
          if (stopped) return;
          onDelta(null, e);
          await sleep(back);
          back = Math.min(back * 2, 30000);
        }
      }
    })();
    return () => { stopped = true; if (ctl) ctl.abort(); };
  };
  // events(op, req, onEvent): one stream of the view's own cell as server-sent events (GET api/cell/<cell>/<op>?<req>,
  // as the signed-in person; the face passes ai.session Events only). onEvent(event) for each; onEvent(null, error)
  // when it drops (error.again: the browser reconnects by itself) or ends; returns stop().
  host.events = (op, req, onEvent) => {
    if (!/^[A-Za-z]{1,32}$/.test(op) || typeof EventSource !== 'function') { onEvent(null, Object.assign(new Error('no events here'), { status: 0 })); return () => {}; }
    const q = new URLSearchParams();
    for (const [k, v] of Object.entries(req || {})) q.set(k, String(v));
    const es = new EventSource('api/cell/' + cell + '/' + op + '?' + q.toString());
    es.onmessage = m => { let e = null; try { e = JSON.parse(m.data); } catch (_) {} if (e != null) onEvent(e); };
    es.addEventListener('end', () => { es.close(); onEvent(null, Object.assign(new Error('ended'), { status: 0, ended: true })); });
    es.onerror = () => onEvent(null, Object.assign(new Error(es.readyState === 2 ? 'closed' : 'reconnecting'), { status: 0, again: es.readyState !== 2 }));
    return () => es.close();
  };
  // open(target): a ce-ref (or an http address, which becomes a web: ref) opens through open.with in the pane
  host.open = (target, opts) => {
    const s = String(target || '');
    if (s) open(/^https?:\/\//.test(s) ? 'web:' + s : s, opts);
  };
  host.ref = { parse: x => CeRef.parse(x), find: x => CeRef.find(x), show: (r, o) => refChip(r, o) };
  return host;
}

const icon = () => {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 20 20'); s.setAttribute('fill', 'none'); s.setAttribute('stroke', 'currentColor'); s.setAttribute('stroke-width', '1.4');
  const r = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
  r.setAttribute('x', '4'); r.setAttribute('y', '4'); r.setAttribute('width', '12'); r.setAttribute('height', '12');
  s.append(r); return s;
};

let first = true;
async function tick() {
  let r;
  try { r = await fetch('tabs.json', { cache: 'no-store', credentials: 'same-origin' }); } catch (_) { return; }
  if (r.status === 401) return say('sign in', 'Sign in.', 'Sign in to see what is open to you.', ['Sign in', '/login?next=' + encodeURIComponent(location.pathname)]);
  if (!r.ok) return;
  const d = await r.json();
  here = d.here; tabsNow = d.tabs;
  // a tab whose route the gate does not serve yet carries `href`: where its view opens meanwhile (under another tab)
  const at = location.pathname + location.search;
  const cur = t => t.href ? at === t.href : t.route === d.here && !d.tabs.some(x => x.href && at === x.href);
  const key = JSON.stringify(d.tabs.map(t => [t.route, t.href, t.title, t.badge, cur(t)]));
  if (key !== last) {
    last = key;
    rail.replaceChildren(...d.tabs.map(t => h('a', { href: t.href || t.route, 'aria-current': cur(t) ? 'page' : false },
      icon(), t.title, t.badge ? h('span', { class: 'bdg' }, t.badge) : null)));
  }
  if (first) {
    first = false;
    // old Search links (?ref= / ?rec=) and /o/<ref> open in the pane
    const q = new URLSearchParams(location.search);
    const start = refInAddress() || q.get('ref') || q.get('rec');
    const sr = start && CeRef.fromBar(start);
    if (sr) { openNow = CeRef.short(sr); history.replaceState({ o: openNow }, '', addressOf(openNow)); open(sr, { fromHistory: true }); }
  }
  const tab = d.tabs.find(t => t.route === d.here);
  if (!d.tabs.length) return say('apps', 'Nothing open.', 'No tab is open to you here. Ask for access, and it appears here when it is granted.');
  if (!tab || !tab.view) {
    if (mounted || !main.dataset.closed) { say('apps', 'Closed here.', 'This tab is not open to you now, or none of its cells answers. Your open tabs are on the left.'); main.dataset.closed = '1'; }
    return;
  }
  delete main.dataset.closed;
  // an extra view this tab hosts (`?view=<name>`, tabs.json `views`): only one this viewer may open; its module is the
  // cell's own and gets that cell's relay
  const want = new URLSearchParams(location.search).get('view');
  const extra = want && /^[a-z]{1,32}$/.test(want) && (d.views || []).includes(want) ? "v/" + want + ".js" : null;
  if (want && !extra) {
    if (mounted || !main.dataset.closed) { if (mounted) { mounted.stop(); mounted = null; } say(tab.title.toLowerCase(), 'Not open here.', 'This view is not open to you. Ask for access, and it opens here when it is granted.'); main.dataset.closed = '1'; }
    return;
  }
  const path = extra || tab.view;
  if (mounted && mounted.view === path) return;
  if (mounted) { mounted.stop(); mounted = null; }
  let mod;
  try { mod = await import('../' + path); } catch (_) { return say(tab.title.toLowerCase(), 'Not answering.', 'This view did not load; trying again.'); }
  main.replaceChildren();
  // a tab view of this site gets its own ops; a module a cell ships (view = { cell }) gets that cell's relay
  const host = !extra && TAB_OPS[path] ? makeHost(TAB_OPS[path], { tab, views: d.views || [] }) : hostFor(mod.view, tab, d.notes);
  const stop = mod.mount(main, host);
  mounted = { view: path, stop: typeof stop === 'function' ? stop : () => {} };
}

tick();
setInterval(tick, 3000);

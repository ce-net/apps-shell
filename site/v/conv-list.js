// conv.list (UI cell, left column of the Conversations tab): ai.session List as the viewer. Filters by kind (server
// side), Live (state not Closed), node, source and words; the Nodes block is ai.session Coverage (section 13.8): per
// node the rows beside the real count on its disk, so a node that imports nothing is a line, not an absence. A viewer
// without ai.session:coverage sees the cell's own totals per source and node (List with limit 1) instead. Every tmux pane the viewer may
// see is listed live too: term.session List on each node (as the viewer; its session scope decides), merged with the
// imported tmux rows, so a pane opened a minute ago is there before the next import.
export const view = { cell: 'ai.session', name: 'conv.list', speaks: [1], kit: 1 };

// the cost line `tokens . $cost . time` of a conversation comes from the usage strip's module (conv.usage, ai.usage
// Conversation{id}); it is served only to a viewer holding ai.usage:read, so without it the line is simply absent
let usageP = null;
export const usageMod = () => usageP || (usageP = import('./conv-usage.js').catch(() => null));
const COST_TIP = 'tokens . $ cost . time';
/** show a conversation's cost line in `span` (data-cost=<id>) when the usage module has it; ask for it when it has not */
export async function showCost(span) {
  const m = await usageMod();
  if (!m) return;
  const id = span.dataset.cost, k = m.costOf(id);
  if (k) { span.textContent = k.known ? k.text : ''; span.title = k.known ? k.tip.join('\n') : 'no usage recorded for this conversation'; }
  else m.wantCosts([id]);
}

const KINDS = [['All', null], ['Live', 'live'], ['Lanes', 'lane'], ['Workers', 'worker'], ['Sessions', 'session'], ['Chats', 'chat'], ['Terminals', 'terminal']];
const PAGE = 300;

const req = (o = {}) => ({ provider: null, account: null, node: null, kind: null, source: null, parent: null, q: null, limit: PAGE, ...o });
export const stateName = s => (typeof s === 'string' ? s : Object.keys(s || {})[0] || '?');
export const isLive = c => !['Closed', 'Lost'].includes(stateName(c.state));

export function ago(ms) {
  if (!ms) return '';
  const s = Math.max(0, (Date.now() - ms) / 1000);
  if (s < 90) return 'now';
  if (s < 3600) return Math.round(s / 60) + ' min';
  if (s < 86400) return Math.round(s / 3600) + ' h';
  return Math.round(s / 86400) + ' d';
}

function group(c) {
  if (c.provider === 'term.session') return 'Live terminals';
  if (isLive(c)) return 'Live';
  const d = new Date(c.last_ms), now = new Date();
  const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = (day(now) - day(d)) / 86400000;
  return diff < 1 ? 'Today' : diff < 2 ? 'Yesterday' : 'Earlier';
}

// The Nodes block (repo:ce docs/AI-AS-CELLS-2026-11-18.md section 13.8, design by Fable 5.2): ai.session Coverage as
// the viewer, one line per node: what the list holds of the real number on that node's disk, and the age of its last
// good import. A line is quiet when the two match and the import is fresh, and says why in words when not (a gap, a
// stale import, no importer, not answering). A source with no count reads "not counted", never a guess.
const SRC = { 'claude-code': 'Claude Code', 'lean-worker': 'lean-worker', tmux: 'tmux', opencode: 'opencode', 'agent-deepseek': 'DeepSeek lanes', 'ai.session': 'chats here' };
const fmt = n => Number(n || 0).toLocaleString('en-US');
const have = r => r.listed + r.own + r.elsewhere;
export function srcLine(r) {
  const name = SRC[r.source] || r.source;
  if (r.source === 'tmux') return name + ' ' + fmt(r.listed) + (r.real == null ? ' live' : ' of ' + fmt(r.real) + ' live');
  if (r.real == null) return name + ' ' + fmt(r.listed) + (r.source === 'ai.session' ? '' : ', not counted');
  return name + ' ' + fmt(have(r)) + ' of ' + fmt(r.real) + (r.elsewhere ? ' (' + fmt(r.elsewhere) + ' shown under another node)' : '');
}
export function nodeLines(cov) {
  const by = new Map();
  for (const r of (cov && cov.rows) || []) { if (!by.has(r.node)) by.set(r.node, []); by.get(r.node).push(r); }
  const out = [...by].map(([node, rs]) => {
    const sources = rs.filter(r => r.source);
    const counted = sources.filter(r => r.real != null);
    const real = counted.reduce((a, r) => a + r.real, 0), got = counted.reduce((a, r) => a + have(r), 0);
    const rows = sources.reduce((a, r) => a + r.listed, 0);
    const st = rs.map(r => stateName(r.state)), ok = Math.max(0, ...rs.map(r => r.last_ok_ms || 0));
    const since = ok ? ', last import ' + ago(ok) + (ago(ok) === 'now' ? '' : ' ago') : '';
    let mark = 'ok', note = fmt(got) + ' of ' + fmt(real);
    if (!rows && st.every(x => x === 'NoAdapter')) { mark = 'none'; note = 'no importer'; }
    else if (st.includes('Unreachable')) { mark = 'off'; note = (counted.length ? note + ', ' : fmt(rows) + ' rows, ') + 'not answering' + since; }
    else if (st.includes('Refused')) { mark = 'off'; note = fmt(rows) + ' rows, importer refused'; }
    else if (!counted.length) { mark = 'stale'; note = fmt(rows) + ' rows, not counted' + since; }
    else if (got !== real) mark = 'gap';
    else if (st.includes('Stale')) { mark = 'stale'; note += since; }
    return { node, rows, real, got, mark, note, age: ok ? ago(ok) : '', sources };
  });
  // nodes with sessions first, the biggest on top; nodes with no importer last
  return out.sort((a, b) => (a.mark === 'none') - (b.mark === 'none') || b.rows - a.rows || a.node.localeCompare(b.node));
}

// the tmux panes term.session lists on these nodes (each node's own instance, as the viewer), as rows; a node without
// term.session or a viewer without term.session:read gives none
const termRow = (node, p) => ({ id: 'term:' + node + ':' + p.session + ':' + p.window + '.' + p.index, title: p.session + ' · ' + p.cmd, provider: 'term.session', account: '', model: '',
  node, state: 'Running', kind: p.cmd === 'claude' ? 'lane' : 'terminal', source: 'tmux', cwd: p.cwd, turns: 0, last_ms: Date.now(),
  native: { provider: 'term.session', node, id: 'tmux:' + p.session + ':' + p.window + '.' + p.index } });
// the nodes where lanes run in tmux, asked even before a row names them
const LANE_NODES = ['foreman-ops-1', 'leif-desk'];
export async function livePanes(host, nodes) {
  const term = host.other && host.other['term.session'];
  if (!term) return [];
  const per = await Promise.all(nodes.map(n => term.call('List', { node: n }).then(ps => (ps || []).map(p => termRow(n, p))).catch(() => [])));
  return per.flat();
}

export function mountList(root, host, { onOpen, onOpened }) {
  const { h } = host;
  let kind = null, node = null, source = null, q = '', current = null, stopped = false, timer = 0, seq = 0;
  // Coverage: null until asked, false for a viewer without ai.session:coverage (the totals line as before)
  let cov = null, covAt = 0, open = false;
  const n = h('span', { class: 'cv-n' }, '');
  const search = h('input', { class: 'cv-q', type: 'search', placeholder: 'Search titles, folders, ids', 'aria-label': 'Search conversations' });
  const chips = h('div', { class: 'cv-chips', role: 'group', 'aria-label': 'Kind' });
  const nodes = h('div', { class: 'cv-chips', role: 'group', 'aria-label': 'Node' });
  const counts = h('div', { class: 'cv-count', 'data-test': 'counts' });
  const rows = h('div', { class: 'cv-rows' });
  // New: a bare conversation with claude.terminal (ai.session Open as the viewer), opened in the centre
  const newB = h('button', { type: 'button', class: 'cv-btn primary', 'data-test': 'new-chat', onclick: async () => {
    newB.disabled = true;
    try {
      const o = await host.call('Open', { provider: { Named: 'claude.terminal' }, model: null, account: null, system: null, profile: null, tools: [], harness: 'Bare', place: null, title: 'chat ' + new Date().toLocaleString(), parent: null, budget: { max_tokens: 0, max_cost_usd: 0, max_wall_s: 0 } });
      if (onOpened) onOpened(o.conversation);
    } catch (e) { n.textContent = e.refused ? 'New chats are not open to you' : 'New chat: ' + e.message; }
    newB.disabled = false;
  } }, 'New chat');
  root.replaceChildren(h('div', { class: 'cv-head' }, h('div', { class: 'cv-title' }, h('h2', {}, 'Conversations'), n), onOpened ? newB : '', search, chips, nodes, counts), rows);

  const chip = (label, pressed, on) => h('button', { type: 'button', class: 'cv-chip', 'aria-pressed': pressed ? 'true' : 'false', onclick: on }, label);
  const drawChips = () => chips.replaceChildren(...KINDS.map(([l, k]) => chip(l, kind === k, () => { kind = k; drawChips(); load(); })));
  drawChips();
  let knownNodes = [];
  const pick = (x, src) => { node = x; source = src || null; drawNodes(); load(); };
  const drawNodes = () => nodes.replaceChildren(chip('Every node', node === null, () => pick(null)),
    ...knownNodes.map(x => chip(x, node === x && !source, () => pick(x))),
    ...(source ? [chip((SRC[source] || source) + ' only', true, () => pick(node))] : []));
  search.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(() => { q = search.value.trim(); load(); }, 250); });

  // the Nodes block: a summary line with the session count, then (opened) one line per node and its sources; a node
  // or a source filters the list on the server (List{node, source}); asked at most once a minute
  async function coverage(all) {
    if (cov !== false && Date.now() - covAt > 60000) {
      try { cov = await host.call('Coverage', null); covAt = Date.now(); }
      catch (e) { cov = e.refused || e.status === 404 ? false : cov; covAt = Date.now(); }
    }
    if (!cov) return totals(all);
    const lines = nodeLines(cov);
    const bad = lines.filter(l => l.mark !== 'ok' && l.mark !== 'none').length;
    const sum = h('summary', { 'data-test': 'nodes-summary' }, h('span', {}, 'Nodes' + (bad ? ' · ' + bad + ' need a look' : '')), h('span', {}, fmt(cov.total_rows) + ' sessions'));
    const body = lines.map(l => h('div', { class: 'cv-nline' },
      h('button', { type: 'button', class: 'cv-node', 'data-node': l.node, 'data-mark': l.mark, 'aria-pressed': node === l.node && !source ? 'true' : 'false', onclick: () => pick(l.node) },
        h('span', {}, l.node), h('span', { class: 'cv-nst' }, l.note), h('span', {}, l.mark === 'ok' ? l.age : '')),
      l.sources.length ? h('div', { class: 'cv-src' }, ...l.sources.map(r => h('button', { type: 'button', class: 'cv-chip', 'data-source': r.source, 'aria-pressed': node === l.node && source === r.source ? 'true' : 'false', onclick: () => pick(l.node, r.source) }, srcLine(r)))) : ''));
    const d = h('details', { class: 'cv-nodes', 'data-test': 'nodes' }, sum, ...body);
    if (open) d.open = true;
    d.addEventListener('toggle', () => { open = d.open; });
    counts.replaceChildren(d);
  }

  async function totals(all) {
    // the cell's own count per node (List with limit 1 answers the total)
    const per = await Promise.all(knownNodes.map(x => host.call('List', req({ node: x, limit: 1 })).then(r => [x, r.total]).catch(() => [x, '?'])));
    counts.replaceChildren(
      h('span', {}, 'total ' + all.total),
      ...(all.by_source || []).map(b => h('span', {}, b.key + ' ' + b.n)),
      ...per.map(([x, t]) => h('span', { 'data-node': x }, x + ' ' + t)));
  }

  async function load() {
    const my = ++seq;
    const k = kind === 'live' ? null : kind;
    n.textContent = '…';
    let r;
    try {
      // Live is a state, not a List field: the newest 1000 rows (List's most) are filtered here
      r = await host.call('List', req({ kind: k, node, source, q: q || null, limit: kind === 'live' ? 1000 : PAGE }));
    } catch (e) {
      if (my !== seq) return;
      rows.replaceChildren(h('p', { class: 'cv-empty' }, e.refused ? 'Conversations are not open to you.' : 'ai.session is not answering: ' + e.message));
      n.textContent = '';
      return;
    }
    if (my !== seq || stopped) return;
    let list = r.conversations || [];
    // live panes on every node Coverage names that runs an importer (not a fixed pair)
    const covNodes = cov ? nodeLines(cov).filter(l => l.mark !== 'none').map(l => l.node) : [];
    const ns0 = [...new Set([...LANE_NODES, ...covNodes, ...knownNodes, ...list.map(c => c.node)])].filter(Boolean).sort();
    if (!['worker', 'session', 'chat'].includes(kind) && (!source || source === 'tmux')) {
      const panes = await livePanes(host, node ? [node] : ns0);
      if (my !== seq || stopped) return;
      const have = new Set(list.filter(c => c.source === 'tmux' && c.native).map(c => c.node + ' ' + c.native.id));
      const ql = (q || '').toLowerCase();
      const add = panes.filter(c => !have.has(c.node + ' ' + c.native.id) && (!kind || kind === 'live' || c.kind === kind) && (!ql || (c.title + ' ' + c.cwd).toLowerCase().includes(ql)));
      list = [...add, ...list];
      r = { ...r, total: r.total + add.length };
    }
    if (kind === 'live') list = list.filter(isLive);
    const ns = [...new Set((r.conversations || []).map(c => c.node))].sort();
    if (!node && !q && !kind) { knownNodes = ns; drawNodes(); }
    n.textContent = (kind === 'live' ? list.length + ' live of ' : '') + r.total;
    if (!q && !kind) coverage(r);
    if (!list.length) { rows.replaceChildren(h('p', { class: 'cv-empty' }, q ? 'Nothing matches “' + q + '”.' : 'No conversations here yet.')); return; }
    const out = [];
    let last = '';
    for (const c of list) {
      const g = group(c);
      if (g !== last) { out.push(h('div', { class: 'cv-group' }, g)); last = g; }
      out.push(rowOf(c));
    }
    rows.replaceChildren(...out);
    if (kind !== 'live') more(r, list, my);
    watchCosts();
  }
  // "Load 300 older": ai.session ListBefore with the same filter, from the oldest row shown; the footer always says
  // how many of the cell's total are shown
  function more(r, list, my) {
    const shown = list.length, olds = list.filter(c => c.provider !== 'term.session');
    const foot = h('p', { class: 'cv-more', 'data-test': 'more' }, fmt(shown) + ' of ' + fmt(r.total));
    if (shown < r.total && olds.length) {
      const b = h('button', { type: 'button', class: 'cv-btn', 'data-test': 'load-older', onclick: async () => {
        b.disabled = true;
        const k = kind === 'live' ? null : kind;
        try {
          const before = olds[olds.length - 1].last_ms;
          const p = await host.call('ListBefore', { list: req({ kind: k, node, source, q: q || null }), before_ms: before });
          if (my !== seq || stopped) return;
          const next = (p.conversations || []).filter(c => !list.some(x => x.id === c.id));
          if (!next.length) { b.remove(); return; }
          list.push(...next);
          olds.push(...next);
          foot.remove();
          let last = '';
          for (const c of next) {
            const g = group(c);
            if (g !== last) { rows.append(h('div', { class: 'cv-group' }, g)); last = g; }
            rows.append(rowOf(c));
          }
          more(r, list, my);
          watchCosts();
        } catch (e) { b.disabled = false; foot.textContent = 'Older rows: ' + e.message; }
      } }, 'Load ' + PAGE + ' older');
      foot.prepend(b, ' ');
    }
    rows.append(foot);
  }
  function rowOf(c) {
    const live = isLive(c);
    return h('button', { type: 'button', class: 'cv-row', 'data-id': c.id, 'aria-current': current === c.id ? 'true' : 'false', onclick: () => { current = c.id; mark(); onOpen(c); } },
      h('b', {}, h('span', { class: 'cv-dot' + (live ? ' live' : ''), 'aria-hidden': 'true' }), c.title || c.id),
      h('span', { class: 'cv-meta' }, h('span', {}, c.kind), h('span', {}, c.node), h('span', {}, c.turns + ' msg'), h('span', {}, c.model || c.provider), h('span', {}, ago(c.last_ms))),
      h('span', { class: 'cv-meta' }, h('span', { class: 'cv-cost', 'data-cost': c.id, 'aria-label': COST_TIP }, '')));
  }
  // cost lines for the rows in view only (one ai.usage Conversation call each, batched), refreshed when they arrive
  let seen = null, offCosts = () => {};
  async function watchCosts() {
    const m = await usageMod();
    if (!m || stopped) return;
    offCosts(); offCosts = m.onCosts(() => rows.querySelectorAll('.cv-cost[data-cost]').forEach(showCost));
    if (seen) seen.disconnect();
    if (typeof IntersectionObserver === 'function') {
      seen = new IntersectionObserver(es => { const ids = es.filter(e => e.isIntersecting).map(e => e.target.querySelector('.cv-cost')); ids.forEach(showCost); }, { root: rows, rootMargin: '200px' });
      rows.querySelectorAll('.cv-row').forEach(r => seen.observe(r));
    } else rows.querySelectorAll('.cv-cost[data-cost]').forEach(showCost);
  }
  const mark = () => rows.querySelectorAll('.cv-row').forEach(b => b.setAttribute('aria-current', b.dataset.id === current ? 'true' : 'false'));

  load();
  const every = setInterval(() => { if (!document.hidden) load(); }, 30000);
  return {
    stop() { stopped = true; clearInterval(every); clearTimeout(timer); offCosts(); if (seen) seen.disconnect(); },
    focus(id) { const b = rows.querySelector('[data-id="' + id + '"]'); if (b) b.focus(); },
    refresh(id) { if (id) current = id; load(); },
  };
}

// mounted alone (a slot): the list, and a click opens the conversation in the pane
export function mount(root, host) {
  const l = mountList(root, host, { onOpen: c => host.open && host.open('view:ai.session@1::Thread?c=' + c.id) });
  return () => l.stop();
}

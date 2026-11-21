// conv.list (UI cell, left column of the Conversations tab): ai.session List as the viewer. Filters by kind (server
// side), Live (state not Closed), node and words; counts per source and per node are the cell's own totals (List with
// limit 1), so they match the rows ai.store holds for this viewer, not the page shown.
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
  if (isLive(c)) return 'Live';
  const d = new Date(c.last_ms), now = new Date();
  const day = x => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = (day(now) - day(d)) / 86400000;
  return diff < 1 ? 'Today' : diff < 2 ? 'Yesterday' : 'Earlier';
}

export function mountList(root, host, { onOpen }) {
  const { h } = host;
  let kind = null, node = null, q = '', current = null, stopped = false, timer = 0, seq = 0;
  const n = h('span', { class: 'cv-n' }, '');
  const search = h('input', { class: 'cv-q', type: 'search', placeholder: 'Search titles, folders, ids', 'aria-label': 'Search conversations' });
  const chips = h('div', { class: 'cv-chips', role: 'group', 'aria-label': 'Kind' });
  const nodes = h('div', { class: 'cv-chips', role: 'group', 'aria-label': 'Node' });
  const counts = h('div', { class: 'cv-count', 'data-test': 'counts' });
  const rows = h('div', { class: 'cv-rows' });
  root.replaceChildren(h('div', { class: 'cv-head' }, h('div', { class: 'cv-title' }, h('h2', {}, 'Conversations'), n), search, chips, nodes, counts), rows);

  const chip = (label, pressed, on) => h('button', { type: 'button', class: 'cv-chip', 'aria-pressed': pressed ? 'true' : 'false', onclick: on }, label);
  const drawChips = () => chips.replaceChildren(...KINDS.map(([l, k]) => chip(l, kind === k, () => { kind = k; drawChips(); load(); })));
  drawChips();
  let knownNodes = [];
  const drawNodes = () => nodes.replaceChildren(chip('Every node', node === null, () => { node = null; drawNodes(); load(); }),
    ...knownNodes.map(x => chip(x, node === x, () => { node = x; drawNodes(); load(); })));
  search.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(() => { q = search.value.trim(); load(); }, 250); });

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
      r = await host.call('List', req({ kind: k, node, q: q || null, limit: kind === 'live' ? 1000 : PAGE }));
    } catch (e) {
      if (my !== seq) return;
      rows.replaceChildren(h('p', { class: 'cv-empty' }, e.refused ? 'Conversations are not open to you.' : 'ai.session is not answering: ' + e.message));
      n.textContent = '';
      return;
    }
    if (my !== seq || stopped) return;
    let list = r.conversations || [];
    if (kind === 'live') list = list.filter(isLive);
    const ns = [...new Set((r.conversations || []).map(c => c.node))].sort();
    if (!node && !q && !kind) { knownNodes = ns; drawNodes(); }
    n.textContent = (kind === 'live' ? list.length + ' live of ' : '') + r.total;
    if (!node && !q && !kind) totals(r);
    if (!list.length) { rows.replaceChildren(h('p', { class: 'cv-empty' }, q ? 'Nothing matches “' + q + '”.' : 'No conversations here yet.')); return; }
    const out = [];
    let last = '';
    for (const c of list) {
      const g = group(c);
      if (g !== last) { out.push(h('div', { class: 'cv-group' }, g)); last = g; }
      const live = isLive(c);
      out.push(h('button', { type: 'button', class: 'cv-row', 'data-id': c.id, 'aria-current': current === c.id ? 'true' : 'false', onclick: () => { current = c.id; mark(); onOpen(c); } },
        h('b', {}, h('span', { class: 'cv-dot' + (live ? ' live' : ''), 'aria-hidden': 'true' }), c.title || c.id),
        h('span', { class: 'cv-meta' }, h('span', {}, c.kind), h('span', {}, c.node), h('span', {}, c.turns + ' msg'), h('span', {}, c.model || c.provider), h('span', {}, ago(c.last_ms))),
        h('span', { class: 'cv-meta' }, h('span', { class: 'cv-cost', 'data-cost': c.id, 'aria-label': COST_TIP }, ''))));
    }
    if (r.total > list.length && kind !== 'live') out.push(h('p', { class: 'cv-more' }, 'Newest ' + list.length + ' of ' + r.total + ' shown; search or filter for the rest.'));
    rows.replaceChildren(...out);
    watchCosts();
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
  };
}

// mounted alone (a slot): the list, and a click opens the conversation in the pane
export function mount(root, host) {
  const l = mountList(root, host, { onOpen: c => host.open && host.open('view:ai.session@1::Thread?c=' + c.id) });
  return () => l.stop();
}

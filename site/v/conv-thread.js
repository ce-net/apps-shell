// conv.thread (UI cell, centre of the Conversations tab): one conversation, read only. ai.session Get for its row,
// then Read in pages of 50 turns; an imported Claude Code session is read in place on the node that holds it (nothing
// is copied). Tool calls fold to one line each, thinking is left out, workers show their row only. A tmux pane opens
// live (conv.term: term.session on its node: the screen as it changes, typing, keys, Interrupt) for a viewer who may.
import { ago, stateName, showCost, usageMod } from './conv-list.js';

export const view = { cell: 'ai.session', name: 'conv.thread', speaks: [1], kit: 1 };

const PAGE = 50;

function partsOf(h, m) {
  const out = [];
  for (const p of m.parts || []) {
    const [k, v] = typeof p === 'string' ? [p, null] : Object.entries(p)[0] || [];
    if (k === 'Text' && v) out.push(h('p', { class: 'cv-text' }, v));
    else if (k === 'ToolCall') out.push(h('div', { class: 'cv-tool', title: v.arg }, '> ' + v.op + ' ' + (v.arg || '').slice(0, 140)));
    else if (k === 'ToolResult') out.push(h('div', { class: 'cv-tool' + (v.ok ? '' : ' bad'), title: v.out }, (v.ok ? '< ' : '< error ') + (v.out || '').replace(/\s+/g, ' ').slice(0, 140)));
    else if (k === 'Attachment') out.push(h('div', { class: 'cv-tool' }, '[' + v + ']'));
  }
  return out;
}

export function mountThread(root, host, conv, { onBack }) {
  const { h } = host;
  if (!conv) {
    root.replaceChildren(h('p', { class: 'cv-empty' }, 'Pick a conversation on the left to read it.'));
    return () => {};
  }
  let stopped = false, next = 0, loading = false;
  const head = h('div', { class: 'cv-th' });
  const turns = h('div', { class: 'cv-turns', 'aria-live': 'polite' });
  const more = h('button', { type: 'button', class: 'cv-btn', onclick: () => page() }, 'Load more');
  root.replaceChildren(head, turns);

  const drawHead = c => head.replaceChildren(
    onBack ? h('button', { type: 'button', class: 'cv-btn cv-back', onclick: onBack }, 'Back') : '',
    h('h2', {}, c.title || c.id),
    h('div', { class: 'cv-meta' }, h('span', {}, c.provider), h('span', {}, c.account || ''), h('span', {}, c.node), h('span', {}, c.kind + ' · ' + c.source), h('span', {}, stateName(c.state)), h('span', {}, c.turns + ' msg'), h('span', {}, ago(c.last_ms))),
    h('div', { class: 'cv-meta' }, h('span', { class: 'cv-cost', 'data-cost': c.id }, '')),
    c.cwd ? h('div', { class: 'cv-meta' }, h('span', {}, c.cwd)) : '',
    h('div', { class: 'cv-meta' }, h('span', {}, 'ai.session ' + c.id), c.native ? h('span', {}, 'native ' + c.native.id) : ''));

  async function page() {
    if (loading || next === null || stopped) return;
    loading = true;
    more.remove();
    let t;
    try {
      t = await host.call('Read', { conversation: conv.id, from: next, limit: PAGE });
    } catch (e) {
      loading = false;
      if (stopped) return;
      turns.append(h('p', { class: 'cv-empty' }, e.status === 502 ? 'The transcript could not be read where it lives: ' + e.message : 'Read failed: ' + e.message));
      return;
    }
    loading = false;
    if (stopped) return;
    const list = t.turns || [];
    if (!list.length && next === 0) {
      const why = (t.refs || []).find(r => / only$/.test(r.label));
      turns.append(h('p', { class: 'cv-empty' }, why ? 'This ' + conv.kind + ' is listed by its row only (' + why.label + ').' : 'No turns yet.'));
    }
    // a record with nothing to show (thinking only) is left out; its number stays, so the gap is visible
    turns.append(...list.map((m, i) => [m, i, partsOf(h, m)]).filter(([, , p]) => p.length).map(([m, i, p]) => h('div', { class: 'cv-turn', 'data-n': next + i },
      h('div', { class: 'cv-who' }, h('b', {}, m.role === 'Assistant' ? (m.by || 'assistant') : m.role === 'Tool' ? 'tool' : 'you'), h('span', {}, '#' + (next + i)), h('span', {}, m.at_ms ? new Date(m.at_ms).toLocaleString() : '')),
      ...p)));
    next = t.next == null ? null : t.next;
    if (next !== null) turns.append(more);
  }

  // the cost line of the header (tokens . $ . time; tooltip model, account, device) from ai.usage, when the viewer may
  const cost = () => { const sp = head.querySelector('.cv-cost'); if (sp) showCost(sp); };
  let offCosts = () => {}, offTerm = () => {};
  // a tmux row: the pane itself, live, when the viewer holds term.session:read (else its module is not served)
  async function openTerm(c) {
    const term = host.other && host.other['term.session'];
    const m = term && await import('./conv-term.js').catch(() => null);
    const pane = m && m.paneOf(c);
    if (!m || !pane || !c.node || stopped) return false;
    offTerm = m.mountTerm(turns, term, { node: c.node, pane });
    return true;
  }
  usageMod().then(m => { if (m && !stopped) offCosts = m.onCosts(cost); });
  (async () => {
    drawHead(conv); cost();
    if (conv.provider !== 'term.session') try { const c = await host.call('Get', { conversation: conv.id }); if (!stopped) { conv = c; drawHead(c); cost(); } } catch (_) {}
    if (conv.source === 'tmux' && await openTerm(conv)) return;
    page();
  })();
  return () => { stopped = true; offCosts(); offTerm(); };
}

export function mount(root, host) {
  const id = host.props && host.props.c;
  return mountThread(root, host, id ? { id, title: id, provider: '', node: '', kind: '', source: '', state: 'Idle', turns: 0 } : null, {});
}

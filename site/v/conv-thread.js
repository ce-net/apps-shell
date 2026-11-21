// conv.thread (UI cell, centre of the Conversations tab): one conversation. ai.session Get for its row, then Read in
// pages of 50 turns; an imported Claude Code session is read in place on the node that holds it (nothing is copied).
// Tool calls fold to one line each, thinking is left out, workers show their row only. A tmux pane opens live
// (conv.term: term.session on its node: the screen as it changes, typing, keys, Interrupt) for a viewer who may.
// Slice 3 (repo:ce docs/AI-AS-CELLS-2026-11-18.md section 12.1): an ai.session conversation takes a message (Send,
// never waiting), streams its answer live (Events), can be interrupted, forked at its last turn and closed; an
// imported Claude Code session is continued in place first (Continue). Every call is the viewer's, through the relay.
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

const refusal = e => e.refused ? 'Not open to you.' : e.status === 409 ? e.message : e.status === 0 ? 'No connection.' : e.message;

export function mountThread(root, host, conv, { onBack, onOpened }) {
  const { h } = host;
  if (!conv) {
    root.replaceChildren(h('p', { class: 'cv-empty' }, 'Pick a conversation on the left to read it.'));
    return () => {};
  }
  let stopped = false, next = 0, loading = false, shown = 0, state = conv.state, offEvents = () => {}, draft = null;
  const head = h('div', { class: 'cv-th' });
  const turns = h('div', { class: 'cv-turns', 'aria-live': 'polite' });
  const more = h('button', { type: 'button', class: 'cv-btn', onclick: () => page() }, 'Load more');
  const note = h('p', { class: 'cv-note', role: 'status' }, '');
  const box = h('textarea', { class: 'cv-in', rows: 2, placeholder: 'Message', 'aria-label': 'Message', maxlength: 32768 });
  const sendB = h('button', { type: 'button', class: 'cv-btn primary', onclick: () => send() }, 'Send');
  const stopB = h('button', { type: 'button', class: 'cv-btn', onclick: () => interrupt() }, 'Interrupt');
  const forkB = h('button', { type: 'button', class: 'cv-btn', onclick: () => fork() }, 'Fork');
  const closeB = h('button', { type: 'button', class: 'cv-btn', onclick: () => close() }, 'Close');
  const contB = h('button', { type: 'button', class: 'cv-btn primary', onclick: () => cont() }, 'Continue here');
  const bar = h('div', { class: 'cv-bar' });
  const compose = h('div', { class: 'cv-compose' }, note, bar);
  box.addEventListener('keydown', e => { if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); send(); } });
  root.replaceChildren(head, turns, compose);
  const say = t => { note.textContent = t || ''; };
  // which controls this row has: an ai.session row takes messages; an imported Claude Code row is continued first
  function drawBar() {
    const own = conv.source === 'ai.session' && stateName(state) !== 'Closed';
    const busy = stateName(state) === 'Busy';
    sendB.disabled = busy;
    if (own) bar.replaceChildren(box, h('div', { class: 'cv-acts' }, sendB, busy ? stopB : '', conv.turns > 0 || shown > 0 ? forkB : '', closeB));
    else if (conv.source === 'claude-code') bar.replaceChildren(h('div', { class: 'cv-acts' }, contB, shown > 0 || conv.turns > 0 ? forkB : ''));
    else bar.replaceChildren();
    compose.hidden = !bar.childElementCount && !note.textContent;
  }
  const turnEl = (m, n) => {
    const p = partsOf(h, m);
    return p.length ? h('div', { class: 'cv-turn', 'data-n': n },
      h('div', { class: 'cv-who' }, h('b', {}, m.role === 'Assistant' ? (m.by || 'assistant') : m.role === 'Tool' ? 'tool' : 'you'), h('span', {}, '#' + n), h('span', {}, m.at_ms ? new Date(m.at_ms).toLocaleString() : '')),
      ...p) : null;
  };
  // live: the answer as it is written (Delta), each finished turn, each state change
  function live() {
    offEvents();
    offEvents = host.events ? host.events('Events', { conversation: conv.id }, (e, err) => {
      if (stopped) return;
      if (!e) { if (err && err.again) say('Reconnecting the live stream...'); return; }
      if (e.Delta) {
        if (!draft) { draft = h('p', { class: 'cv-text' }, ''); turns.append(h('div', { class: 'cv-turn cv-draft', 'data-live': '1' }, h('div', { class: 'cv-who' }, h('b', {}, 'writing...')), draft)); }
        draft.textContent += e.Delta.text;
        turns.scrollTop = turns.scrollHeight;
      } else if (e.Turn) {
        turns.querySelectorAll('.cv-draft').forEach(d => d.remove());
        draft = null;
        const el = turnEl(e.Turn.message, shown);
        shown += 1;
        if (el) turns.append(el);
        turns.scrollTop = turns.scrollHeight;
      } else if (e.State !== undefined) {
        state = e.State;
        if (stateName(state) !== 'Busy') { turns.querySelectorAll('.cv-draft').forEach(d => d.remove()); draft = null; }
        say(stateName(state) === 'Busy' ? 'Answering...' : '');
        drawBar();
      } else if (e.Refused) {
        const r = e.Refused, k = typeof r === 'string' ? r : Object.keys(r)[0];
        say(k === 'Policy' && r.Policy && r.Policy.reason === 'interrupted' ? 'Interrupted.' : 'Refused: ' + (typeof r === 'string' ? r : k + ' ' + JSON.stringify(r[k])));
      }
    }) : () => {};
  }
  async function send() {
    const text = box.value.trim();
    if (!text || sendB.disabled) return;
    sendB.disabled = true;
    try {
      await host.call('Send', { conversation: conv.id, parts: [{ Text: text }], expect: null, wait: false });
      box.value = '';
      const el = turnEl({ role: 'User', parts: [{ Text: text }], at_ms: Date.now() }, shown);
      shown += 1;
      if (el) turns.append(el);
      state = 'Busy';
      say('Answering...');
    } catch (e) { say('Not sent: ' + refusal(e)); }
    drawBar();
  }
  async function interrupt() {
    try { await host.call('Interrupt', { conversation: conv.id }); say('Interrupting...'); } catch (e) { say('Interrupt: ' + refusal(e)); }
  }
  async function fork() {
    const at = Math.max(0, Math.max(shown, conv.turns) - 1);
    try {
      const o = await host.call('Fork', { conversation: conv.id, at, provider: null, account: null, model: null, title: null });
      const how = (o.refs || []).find(r => /^fork /.test(r.label));
      say('Forked (' + (how ? how.label.slice(5) : 'new') + ').');
      if (onOpened) onOpened(o.conversation);
    } catch (e) { say('Fork: ' + refusal(e)); }
  }
  async function close() {
    try { state = await host.call('Close', { conversation: conv.id }); say('Closed.'); } catch (e) { say('Close: ' + refusal(e)); }
    drawBar();
  }
  // an imported Claude Code session: adopted in place on its node, then it takes messages here
  async function cont() {
    contB.disabled = true;
    try {
      await host.call('Continue', { conversation: conv.id });
      conv = await host.call('Get', { conversation: conv.id });
      state = conv.state;
      drawHead(conv);
      say('Continued here: the next message resumes it on ' + conv.node + '.');
      live();
    } catch (e) { say('Continue: ' + refusal(e)); }
    contB.disabled = false;
    drawBar();
  }

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
    turns.append(...list.map((m, i) => turnEl(m, next + i)).filter(Boolean));
    shown = next + list.length;
    next = t.next == null ? null : t.next;
    if (next !== null) turns.append(more);
    drawBar();
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
    state = conv.state;
    drawBar();
    if (conv.source === 'tmux' && await openTerm(conv)) return;
    await page();
    // the rest of the turns, then live from there
    while (next !== null && !stopped && conv.source === 'ai.session') await page();
    if (conv.source === 'ai.session' && stateName(state) !== 'Closed' && !stopped) live();
  })();
  return () => { stopped = true; offCosts(); offTerm(); offEvents(); };
}

export function mount(root, host) {
  const id = host.props && host.props.c;
  return mountThread(root, host, id ? { id, title: id, provider: '', node: '', kind: '', source: '', state: 'Idle', turns: 0 } : null, {});
}

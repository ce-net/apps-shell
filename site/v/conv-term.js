// conv.term (UI cell, centre of the Conversations tab for a tmux row): the pane live, through term.session on the
// node that holds it (term.session#<node>), as the viewer. Watch long-polls the cell's Attach stream (the screen each
// time it changes); a line typed here is Keys (text, then Enter), the key row sends tmux key names, Interrupt is
// Ctrl-C. term.session's gate and the viewer's session scope decide on that node: a pane outside them answers 404,
// typing without term.session:write answers 403 and the input row says so.
export const view = { cell: 'term.session', name: 'conv.term', speaks: [1], kit: 1 };

const CSS = `
.ct{display:flex;flex-direction:column;gap:var(--s-2);min-width:0}
.ct-bar{display:flex;flex-wrap:wrap;align-items:center;gap:var(--s-2);font-size:var(--t-xs);color:var(--ink-3)}
.ct-dot{width:8px;height:8px;border-radius:50%;background:var(--ink-3);display:inline-block}
.ct-dot.on{background:var(--ok, #2f9e5b)}
.ct-dot.bad{background:var(--bad)}
.ct-screen{margin:0;padding:var(--s-3);background:var(--well);color:var(--ink);border:1px solid var(--line);border-radius:var(--r-sm);font-family:var(--mono);font-size:12px;line-height:1.35;white-space:pre;overflow:auto;max-height:70vh;min-height:12em;tab-size:8}
.ct-form{display:flex;gap:var(--s-2)}
.ct-in{flex:1;min-width:0;padding:var(--s-2) var(--s-3);border:1px solid var(--line);border-radius:var(--r-sm);background:var(--well);color:var(--ink);font-family:var(--mono);font-size:var(--t-sm);min-height:36px;box-sizing:border-box}
.ct-keys{display:flex;flex-wrap:wrap;gap:var(--s-1)}
.ct-keys .cv-btn{font-family:var(--mono);font-size:var(--t-xs)}
.ct-int{border-color:var(--bad);color:var(--bad)}
.ct-note{font-size:var(--t-xs);color:var(--ink-3);margin:0}
.ct-note.bad{color:var(--bad)}
@media (max-width: 760px){.ct{padding-bottom:72px}}
`;

let sheet = null;
function style() {
  if (sheet) return;
  try {
    sheet = new CSSStyleSheet();
    sheet.replaceSync(CSS);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  } catch (_) { sheet = {}; }
}

// the pane of a tmux row: native id `tmux:<session>:<window>.<pane>` (claude.terminal's import row)
export function paneOf(conv) {
  const id = (conv && conv.native && conv.native.id) || '';
  return id.startsWith('tmux:') ? id.slice(5) : '';
}

const KEYS = [['Enter', 'Enter'], ['Up', 'Up'], ['Down', 'Down'], ['Tab', 'Tab'], ['Esc', 'Escape'], ['Backspace', 'BSpace'], ['C-d', 'C-d']];

// mountTerm(root, term, { node, pane }): `term` is the relay of term.session ({ h, call(op, req) }); returns stop()
export function mountTerm(root, term, { node, pane }) {
  style();
  const { h } = term;
  let stopped = false, hash = '', canWrite = true, lastAt = 0;
  const dot = h('span', { class: 'ct-dot', 'aria-hidden': 'true' });
  const state = h('span', {}, 'connecting');
  const size = h('span', {}, '');
  const where = h('span', {}, 'term.session#' + node + ' ' + pane);
  const screen = h('pre', { class: 'ct-screen', 'aria-label': 'Terminal screen', 'aria-live': 'off', tabindex: '0' }, '');
  const note = h('p', { class: 'ct-note', role: 'status' }, '');
  const input = h('input', { class: 'ct-in', type: 'text', autocomplete: 'off', spellcheck: 'false', 'aria-label': 'Type into the terminal', placeholder: 'Type a line, Enter sends it' });
  const send = h('button', { type: 'submit', class: 'cv-btn' }, 'Send');
  const form = h('form', { class: 'ct-form', onsubmit: e => { e.preventDefault(); typeLine(); } }, input, send);
  const intr = h('button', { type: 'button', class: 'cv-btn ct-int', title: 'Ctrl-C', onclick: () => act('Interrupt', {}) }, 'Interrupt');
  const keys = h('div', { class: 'ct-keys', role: 'group', 'aria-label': 'Keys' }, intr, ...KEYS.map(([label, k]) => h('button', { type: 'button', class: 'cv-btn', onclick: () => act('Keys', { keys: [k] }) }, label)));
  root.replaceChildren(h('div', { class: 'ct' }, h('div', { class: 'ct-bar' }, dot, state, size, where), screen, form, keys, note));

  const say = (m, bad) => { note.textContent = m || ''; note.classList.toggle('bad', !!bad); };
  const readOnly = () => {
    canWrite = false;
    for (const b of [input, send, ...keys.querySelectorAll('button')]) b.disabled = true;
    say('You may watch this pane, not type into it (term.session:write).', false);
  };
  async function act(op, extra) {
    if (!canWrite || stopped) return false;
    try {
      await term.call(op, { node, pane, ...extra });
      say('');
      return true;
    } catch (e) {
      if (e.status === 403) readOnly();
      else say((op === 'Interrupt' ? 'Interrupt' : 'Typing') + ' failed: ' + e.message, true);
      return false;
    }
  }
  async function typeLine() {
    const text = input.value;
    if (await act('Keys', { text, enter: true })) input.value = '';
    input.focus();
  }
  const paint = r => {
    // keep the reader's place unless they are at the bottom
    const atEnd = screen.scrollTop + screen.clientHeight >= screen.scrollHeight - 4;
    screen.textContent = (r.text || '').replace(/\s+$/, '');
    if (atEnd) screen.scrollTop = screen.scrollHeight;
    size.textContent = r.width && r.height ? r.width + 'x' + r.height : '';
    hash = r.hash || '';
    lastAt = Date.now();
  };
  const live = (on, m) => { dot.classList.toggle('on', on); dot.classList.toggle('bad', !on); state.textContent = m; };

  (async () => {
    let back = 1000;
    while (!stopped) {
      try {
        const r = await term.call('Watch', { node, pane, since: hash });
        if (stopped) return;
        if (!r.same) paint(r);
        live(true, 'live');
        back = 1000;
      } catch (e) {
        if (stopped) return;
        if (e.status === 404 || e.status === 403) {
          live(false, e.status === 404 ? 'gone' : 'refused');
          screen.textContent = '';
          say(e.status === 404 ? 'This pane is not there any more, or not open to you.' : 'This pane is not open to you (term.session:read).', true);
          for (const b of [input, send, ...keys.querySelectorAll('button')]) b.disabled = true;
          return;
        }
        live(false, 'reconnecting');
        say(e.message, true);
        await new Promise(r => setTimeout(r, back));
        back = Math.min(back * 2, 15000);
      }
    }
  })();
  const tick = setInterval(() => { if (lastAt && !stopped && dot.classList.contains('on')) state.textContent = 'live · ' + Math.round((Date.now() - lastAt) / 1000) + ' s since a change'; }, 1000);
  return () => { stopped = true; clearInterval(tick); };
}

export function mount(root, host) {
  const p = host.props || {};
  return mountTerm(root, host, { node: p.node || '', pane: p.pane || '' });
}


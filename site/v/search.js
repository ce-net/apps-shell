// The Search tab: one box over everything. Private: search.hybrid as this viewer (repos, talk log, mail, goals, rooms,
// inbox, calls, people; only what the viewer's own exact grants read). Network: search.mesh's merged answer from every
// node's own index of repos, cells and docs (internal). Public: what cells published for everyone. Each row says its source and machine and opens where
// it lives. ?q= in the address keeps a search shareable with whoever may read the same. Every hit is a ce-ref chip
// opened by host.open through open.with in the pane: a repo or file in ref.view::Text (ref.resolve and ref.pin as this
// viewer), a CRM person, promise or call, or a call recording in ref.view::Record, a cell in site.cellmap::Cell, an
// outside origin as a `web:` ref (the Outside view says so). Never a GitHub link, never an http link in this view.
// Network also lists cell.search's cells with their trust record, and which machines answered.
export const view = { cell: 'search.hybrid', name: 'Search', speaks: [1], kit: 1 };

// the ce-ref a row names: the opener paths the server gives (?ref=, ?rec=), a cell, or an outside origin as web:
export function refOf(x) {
  const o = x.open || '';
  if (o.startsWith('/search/?ref=')) return o.slice(13);
  if (o.startsWith('/search/?rec=')) return o.slice(13);
  if (o === '/core/' && x.kind === 'cell') return 'cell:' + String(x.title || '').split(' · ')[0];
  if (/^https:\/\/\S+$/.test(o)) return 'web:' + o;
  return null;
}

export function mount(root, host) {
  const { h } = host;
  if (!document.querySelector('link[data-search-css]')) document.head.append(h('link', { rel: 'stylesheet', href: '_k/search.css', 'data-search-css': true }));
  const params = new URLSearchParams(location.search);
  let stop = false, seq = 0, show = 'all';
  const input = h('input', { type: 'search', name: 'q', class: 'q', placeholder: 'Repos, docs, people, goals, mail, cells', autocomplete: 'off', 'aria-label': 'Search', maxlength: '300' });
  input.value = params.get('q') || '';
  const form = h('form', { class: 'glass ticks searchbox', role: 'search', onsubmit: e => { e.preventDefault(); go(); } },
    input, h('button', { class: 'btn primary', type: 'submit' }, 'Search'));
  const pills = h('div', { class: 'pills', role: 'group', 'aria-label': 'Show' });
  const out = h('div', { class: 'results', 'aria-live': 'polite' });
  let last = null;
  const pill = (k, label) => h('button', { type: 'button', 'aria-pressed': String(show === k), onclick: () => { show = k; draw(); } }, label);
  root.replaceChildren(h('div', { class: 'crumb' }, 'ce / ', h('b', {}, 'Search'), ' / everything'), h('h1', { class: 'display' }, 'Search.'), form, pills, out);

  const REC = { person: 'CRM person', promise: 'promise', call: 'call', meet: 'transcript' };
  const recKind = x => (x.open || '').startsWith('/search/?rec=') ? (x.open.slice(13).split(':')[0]) : '';
  const title = x => x.title || x.path;
  const row = x => h('div', { class: 'row hit' + (x.group ? ' best' : '') },
    h('div', {},
      h('div', { class: 'l' }, refOf(x) ? host.ref.show(refOf(x), { title: title(x) }) : x.open && x.open.startsWith('/') ? h('a', { href: x.open }, title(x)) : title(x)),
      x.snippet ? h('div', { class: 'snip' }, x.snippet.slice(0, 260)) : null,
      h('div', { class: 'm' }, [x.path, x.also && x.also.length ? 'also on ' + x.also.join(', ') : '', x.trust ? x.trust.join(', ') : '', x.group ? 'in ' + x.group.join(', ') : ''].filter(Boolean).join(' · '))),
    h('div', { class: 'r on' }, h('span', { class: 'tag' + (recKind(x) ? ' crm' : '') }, REC[recKind(x)] || x.kind || x.source || x.from), x.machine ? h('span', { class: 'tag acc' }, x.machine) : null));
  const group = (title, rows, empty, extra) => h('div', { class: 'glass list sys' },
    h('h2', {}, h('b', {}, title), h('span', {}, rows.length + ' hit' + (rows.length === 1 ? '' : 's'))),
    extra || null,
    ...(rows.length ? rows.map(row) : [h('div', { class: 'empty' }, empty)]));
  // which machines search.mesh asked, and which answered (green with hits, red with why not)
  const nodes = me => me.nodes && me.nodes.length ? h('div', { class: 'nodes', 'aria-label': 'Machines asked' },
    ...me.nodes.map(n => h('span', { class: 'node ' + (n.ok ? 'ok' : 'no'), title: n.ok ? n.hits + ' hits in ' + n.took_ms + ' ms' : (n.err || 'no answer') },
      h('i', { class: n.ok ? 'g' : 'r' }), (n.node || '?') + (n.ok ? ' · ' + n.hits : ' · ' + ((n.err || 'no answer').split(':')[0].slice(0, 40)))))) : null;
  const stat = (label, n, sub, dot, acc) => h('div', { class: 'glass stat' + (acc ? ' ticks' : '') }, h('div', { class: 'label' }, h('i', { class: dot || '' }), label), h('div', { class: 'num' + (acc ? ' acc' : '') }, n), h('div', { class: 'sub' }, sub));

  function draw() {
    pills.replaceChildren(pill('all', 'All'), pill('private', 'Private'), pill('network', 'Network'), pill('public', 'Public'));
    if (!last) { out.replaceChildren(h('p', { class: 'note' }, 'Private: your repos, docs, talk log, mail, goals, rooms, inbox, CRM people, calls, promises and call transcripts, only what your grants read. Network: every machine\'s own index of repos, cells and docs, and every registered cell. Public: what cells publish for everyone.')); return; }
    if (last.error) { out.replaceChildren(h('p', { class: 'note' }, last.error)); return; }
    const d = last;
    const hy = d.notes.find(n => n.cell === 'search.hybrid') || {}, me = d.notes.find(n => n.cell === 'search.mesh') || {}, cs = d.notes.find(n => n.cell === 'cell.search') || {};
    const cells = d.cells || [], crm = d.crm || [];
    const cr = d.notes.find(n => n.cell === 'search.hybrid' && n.ask === 'crm') || {};
    const down = d.notes.filter(n => !n.ok && !n.pending && n.err !== 'not open to you');
    const waiting = d.notes.filter(n => n.pending).map(n => n.ask || n.cell);
    const best = d.best || [], convs = d.conversations || [], papers = d.papers || [];
    out.replaceChildren(
      h('div', { class: 'stats' },
        stat('Private', d.private.length + crm.length, hy.ok ? 'from search.hybrid in ' + hy.took_ms + ' ms' : 'search.hybrid: ' + (hy.err || 'no answer'), hy.ok ? 'g' : 'r', true),
        stat('Network', d.network.length + cells.length, me.ok ? me.answered + ' of ' + me.asked + ' machines answered' + (cs.ok ? ', ' + cells.length + ' cells' : '') : me.err === 'not open to you' ? 'not open to you' : 'search.mesh: ' + (me.err || 'no answer'), me.ok && me.answered === me.asked ? 'g' : 'r', false),
        stat('Public', d.public.length, 'published by cells for everyone', null, false)),
      show === 'all' && best.length ? group('Best', best, '', h('div', { class: 'm sub' }, 'One list over every group below, by each row\'s place in its own group.')) : null,
      (show === 'all' || show === 'private') && convs.length ? group('Private · conversations', convs, '', h('div', { class: 'm sub' }, 'Conversations ai.session lists for you whose title, folder or id hold the words.')) : null,
      (show === 'all' || show === 'private') && (crm.length || cr.ok) ? group('Private · CRM and calls', crm, 'No CRM person, call, promise or call transcript matches.', h('div', { class: 'm sub' }, 'People, calls and promises from the CRM cells, and what was said in recorded calls. Each opens its record.')) : null,
      show === 'all' || show === 'private' ? group('Private', d.private, 'Nothing private matches, or nothing private is open to you.') : null,
      show === 'all' || show === 'network' ? group('Network', d.network, me.ok ? 'Nothing on the machines\' own indexes matches.' : 'The network search is not open to you or did not answer.', nodes(me)) : null,
      (show === 'all' || show === 'network') && (cells.length || cs.ok) ? group('Network · cells', cells, 'No cell matches.', h('div', { class: 'm sub' }, cs.ok ? 'cell.search: ' + cs.indexed + ' cells you may see, ranked by meaning and text, each with who signed it and its review' : '')) : null,
      show === 'all' || show === 'public' ? group('Public', d.public, 'Nothing published matches.') : null,
      (show === 'all' || show === 'public') && papers.length ? group('Public · papers', papers, '') : null,
      down.length ? h('p', { class: 'note' }, down.map(n => n.cell + ' did not answer: ' + n.err).join(' · ')) : null,
      h('p', { class: 'note' }, d.partial ? 'First rows in ' + d.took_ms + ' ms; still asking ' + waiting.join(', ') + '.' : 'Answered in ' + d.took_ms + ' ms.'));
  }

  async function go() {
    const q = input.value.trim();
    const u = new URL(document.URL);
    if (q) u.searchParams.set('q', q); else u.searchParams.delete('q');
    history.replaceState(null, '', u);
    if (!q) { last = null; draw(); return; }
    const my = ++seq;
    out.replaceChildren(h('p', { class: 'note' }, 'Searching every source.'));
    // each group is painted as its ask comes back (S5 of repo:ce docs/SPEED-AND-SEARCH-2026-11-21.md)
    const onPart = d => { if (stop || my !== seq) return; last = d; draw(); };
    const r = host.parts ? await host.parts('search', { q }, onPart) : await host.call('search', { q });
    const d = r.ok ? r.data : { error: r.status === 403 ? 'Search is not open to you.' : r.timeout ? 'Search did not answer in 25 s; try again.' : 'Search did not answer (' + r.status + '); try again.' };
    if (stop || my !== seq) return;
    last = d; draw();
  }
  draw();
  if (input.value) go(); else input.focus();
  return () => { stop = true; };
}

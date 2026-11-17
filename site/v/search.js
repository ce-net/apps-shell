// The Search tab: one box over everything. Private: search.hybrid as this viewer (repos, talk log, mail, goals, rooms,
// inbox, calls, people; only what the viewer's own exact grants read). Network: search.mesh's merged answer from every
// node's own index of repos, cells and docs (internal). Public: what cells published for everyone. Each row says its source and machine and opens where
// it lives. ?q= in the address keeps a search shareable with whoever may read the same. A repo or file opens here, in
// the ce-ref viewer (?ref=repo:<repo>/<path>): ref.resolve says where it lives and its hash, ref.pin hands over only
// those bytes, both asked as this viewer. Never a GitHub link. A CRM person, promise or call, or a call recording, opens
// in the record screen (?rec=person:<id>, promise:, call:, meet:): read as this viewer from the cell that owns it
// (crm.people, crm.promises, crm.calls; a recording's passage from search.hybrid). Network also lists cell.search's
// cells with their trust record, and which machines answered.
export function mount(root, { h }) {
  if (!document.querySelector('link[data-search-css]')) document.head.append(h('link', { rel: 'stylesheet', href: '_k/search.css', 'data-search-css': true }));
  const params = new URLSearchParams(location.search);
  if (params.get('ref')) return viewRef(root, h, params.get('ref'));
  if (params.get('rec')) return viewRec(root, h, params.get('rec'), params.get('q') || '');
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
  const href = x => recKind(x) && last && last.q ? x.open + '&q=' + encodeURIComponent(last.q) : x.open;
  const row = x => h('div', { class: 'row hit' },
    h('div', {},
      h('div', { class: 'l' }, x.open && (x.open.startsWith('/') || x.open.startsWith('https://')) ? h('a', { href: href(x), target: x.open.startsWith('/') ? false : '_blank', rel: 'noopener' }, x.title || x.path) : (x.title || x.path)),
      x.snippet ? h('div', { class: 'snip' }, x.snippet.slice(0, 260)) : null,
      h('div', { class: 'm' }, [x.path, x.also && x.also.length ? 'also on ' + x.also.join(', ') : '', x.trust ? x.trust.join(', ') : ''].filter(Boolean).join(' · '))),
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
    const down = d.notes.filter(n => !n.ok && n.err !== 'not open to you');
    out.replaceChildren(
      h('div', { class: 'stats' },
        stat('Private', d.private.length + crm.length, hy.ok ? 'from search.hybrid in ' + hy.took_ms + ' ms' : 'search.hybrid: ' + (hy.err || 'no answer'), hy.ok ? 'g' : 'r', true),
        stat('Network', d.network.length + cells.length, me.ok ? me.answered + ' of ' + me.asked + ' machines answered' + (cs.ok ? ', ' + cells.length + ' cells' : '') : me.err === 'not open to you' ? 'not open to you' : 'search.mesh: ' + (me.err || 'no answer'), me.ok && me.answered === me.asked ? 'g' : 'r', false),
        stat('Public', d.public.length, 'published by cells for everyone', null, false)),
      (show === 'all' || show === 'private') && (crm.length || cr.ok) ? group('Private · CRM and calls', crm, 'No CRM person, call, promise or call transcript matches.', h('div', { class: 'm sub' }, 'People, calls and promises from the CRM cells, and what was said in recorded calls. Each opens its record.')) : null,
      show === 'all' || show === 'private' ? group('Private', d.private, 'Nothing private matches, or nothing private is open to you.') : null,
      show === 'all' || show === 'network' ? group('Network', d.network, me.ok ? 'Nothing on the machines\' own indexes matches.' : 'The network search is not open to you or did not answer.', nodes(me)) : null,
      (show === 'all' || show === 'network') && (cells.length || cs.ok) ? group('Network · cells', cells, 'No cell matches.', h('div', { class: 'm sub' }, cs.ok ? 'cell.search: ' + cs.indexed + ' cells you may see, ranked by meaning and text, each with who signed it and its review' : '')) : null,
      show === 'all' || show === 'public' ? group('Public', d.public, 'Nothing published matches.') : null,
      down.length ? h('p', { class: 'note' }, down.map(n => n.cell + ' did not answer: ' + n.err).join(' · ')) : null,
      h('p', { class: 'note' }, 'Answered in ' + d.took_ms + ' ms.'));
  }

  async function go() {
    const q = input.value.trim();
    const u = new URL(location.href);
    if (q) u.searchParams.set('q', q); else u.searchParams.delete('q');
    history.replaceState(null, '', u);
    if (!q) { last = null; draw(); return; }
    const my = ++seq;
    out.replaceChildren(h('p', { class: 'note' }, 'Searching every source.'));
    let d;
    try {
      const r = await fetch('api/search?q=' + encodeURIComponent(q), { cache: 'no-store' });
      d = r.ok ? await r.json() : { error: r.status === 403 ? 'Search is not open to you.' : 'Search did not answer (' + r.status + '); try again.' };
    } catch (_) { d = { error: 'Search did not answer; try again.' }; }
    if (stop || my !== seq) return;
    last = d; draw();
  }
  draw();
  if (input.value) go(); else input.focus();
  return () => { stop = true; };
}

// The ce-ref viewer: one file through ref.resolve and ref.pin, as this viewer.
function viewRef(root, h, ref) {
  let stop = false;
  const back = h('a', { class: 'btn', href: '/search/' + (document.referrer.includes('/search/?q=') ? new URL(document.referrer).search : '') }, 'Back to search');
  const body = h('div', { class: 'refview', 'aria-live': 'polite' }, h('p', { class: 'note' }, 'Resolving ' + ref + ' through ref.resolve.'));
  const name = ref.replace(/^repo:/, '');
  root.replaceChildren(h('div', { class: 'crumb' }, 'ce / ', h('a', { href: '/search/' }, 'Search'), ' / ', h('b', {}, name)), h('h1', { class: 'display refname' }, name.split('/').pop()), body);
  (async () => {
    let d, st = 0;
    try {
      const r = await fetch('api/ref?r=' + encodeURIComponent(ref), { cache: 'no-store' });
      st = r.status; d = await r.json().catch(() => ({ error: 'no answer (' + r.status + ')' }));
    } catch (_) { d = { error: 'The viewer did not answer; try again.' }; }
    if (stop) return;
    if (st !== 200) { body.replaceChildren(h('p', { class: 'note' }, d.error || ('No answer (' + st + ').')), back); return; }
    const kb = n => n < 1024 ? n + ' B' : (n / 1024).toFixed(1) + ' KB';
    body.replaceChildren(
      h('div', { class: 'glass list sys' },
        h('h2', {}, h('b', {}, d.repo + '/' + d.path), h('span', {}, kb(d.size))),
        h('div', { class: 'row' }, h('div', {}, h('div', { class: 'm' }, 'ce-ref'), h('div', { class: 'l mono' }, d.id))),
        h('div', { class: 'row' }, h('div', {}, h('div', { class: 'm' }, 'pinned'), h('div', { class: 'l mono' }, d.ref))),
        h('div', { class: 'row' }, h('div', {}, h('div', { class: 'm' }, 'lives on'), h('div', { class: 'l' }, d.node + ' · ' + d.source + ' · read by ' + d.via.join(' and '))))),
      d.binary ? h('p', { class: 'note' }, 'A binary file (' + kb(d.size) + '); its bytes match ' + d.sha256 + '.')
        : h('pre', { class: 'glass code', tabindex: '0' }, d.text),
      d.truncated ? h('p', { class: 'note' }, 'Showing the first 512 KB.') : null,
      back);
  })();
  return () => { stop = true; };
}

// The record screen: one CRM person, promise or call, or a call recording's passage, read as this viewer from the cell
// that owns it.
function viewRec(root, h, rec, q) {
  let stop = false;
  const back = h('a', { class: 'btn', href: '/search/' + (q ? '?q=' + encodeURIComponent(q) : '') }, 'Back to search');
  const body = h('div', { class: 'refview', 'aria-live': 'polite' }, h('p', { class: 'note' }, 'Reading ' + rec + '.'));
  const KIND = { person: 'CRM person', promise: 'Promise', call: 'Call', meet: 'Call recording' };
  const kind = rec.split(':')[0];
  root.replaceChildren(h('div', { class: 'crumb' }, 'ce / ', h('a', { href: '/search/' + (q ? '?q=' + encodeURIComponent(q) : '') }, 'Search'), ' / ', h('b', {}, KIND[kind] || 'Record')), h('h1', { class: 'display refname' }, KIND[kind] || 'Record'), body);
  (async () => {
    let d, st = 0;
    try {
      const r = await fetch('api/rec?r=' + encodeURIComponent(rec) + (q ? '&q=' + encodeURIComponent(q) : ''), { cache: 'no-store' });
      st = r.status; d = await r.json().catch(() => ({ error: 'no answer (' + r.status + ')' }));
    } catch (_) { d = { error: 'The record did not answer; try again.' }; }
    if (stop) return;
    if (st !== 200) { body.replaceChildren(h('p', { class: 'note' }, d.error || ('No answer (' + st + ').')), back); return; }
    root.querySelector('h1').textContent = d.title || KIND[kind];
    const more = d.search ? h('a', { class: 'btn', href: '/search/?q=' + encodeURIComponent(d.search) }, 'Everything about ' + d.search) : null;
    body.replaceChildren(
      h('div', { class: 'glass list sys' },
        h('h2', {}, h('b', {}, KIND[kind] || 'Record'), h('span', {}, 'from ' + d.cell)),
        ...(d.fields || []).map(([k, v]) => h('div', { class: 'row' }, h('div', {}, h('div', { class: 'm' }, k), h('div', { class: 'l' }, /^https:\/\//.test(v) ? h('a', { href: v, target: '_blank', rel: 'noopener' }, v) : v))))),
      d.text ? h('pre', { class: 'glass code prose', tabindex: '0' }, d.text) : null,
      d.list && d.list.length ? h('div', { class: 'glass list sys' }, h('h2', {}, h('b', {}, d.list_title || 'List'), h('span', {}, String(d.list.length))), ...d.list.map(t => h('div', { class: 'row' }, h('div', {}, h('div', { class: 'l' }, t))))) : null,
      d.note ? h('p', { class: 'note' }, d.note) : null,
      h('div', { class: 'acts' }, back, more));
  })();
  return () => { stop = true; };
}

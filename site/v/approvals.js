// The Approvals tab: approve.inbox's rows for this viewer (their chain's row caveats applied by the cell), and one
// bundle's card. Signing is one passkey assertion at the bundle's sign_at; a session never approves.
const until = s => s ? new Date(s * 1000).toISOString().slice(0, 16).replace('T', ' ') + ' UTC' : '';

export const view = { cell: 'approve.inbox', name: 'Approvals', speaks: [1], kit: 1 };

// sign_at is our own passkey page: kept as a path on this site; anything else is the outside web, a web: ref
const signPath = u => { const m = /^https:\/\/apps\.ce-net\.com(\/[^\s]*)$/.exec(u || ''); return m ? m[1] : (u && u.startsWith('/') ? u : null); };

export function mount(root, host) {
  const { h } = host;
  const el = { replaceChildren: (...k) => root.replaceChildren(...k.filter(x => x != null && x !== false)) };
  let stop = false, timer = 0;
  const head = (crumb, title) => [h('div', { class: 'crumb' }, 'ce / ', h('b', {}, 'Approvals'), ' / ', crumb), h('h1', { class: 'display' }, title)];
  const card = async (id) => {
    const r = await host.call('card', { b: id });
    if (!r.ok) { el.replaceChildren(...head('Card', 'No such bundle.'), h('p', {}, h('a', { class: 'btn', href: './' }, 'Back to the inbox'))); return; }
    const c = r.data;
    const waiting = c.rows.filter(x => x.state === 'waiting').length;
    el.replaceChildren(...head('Card', c.title.endsWith('.') ? c.title : c.title + '.'),
      h('div', { class: 'pills' }, h('span', { class: 'tag' }, c.id), c.dry_run ? h('span', { class: 'tag acc' }, 'dry run') : null, h('span', { class: 'tag' }, waiting + ' waiting')),
      h('div', { class: 'glass ticks list', style: false }, ...c.rows.map(x => h('div', { class: 'row' },
        h('div', {}, h('div', { class: 'l' }, x.n + '. ' + x.line), h('div', { class: 'm' }, [x.kind, x.owner, x.node, x.requester && ('to ' + x.requester), until(x.until)].filter(Boolean).join(' · ')),
          h('pre', { class: 'signed' }, x.text)),
        h('div', { class: 'r' }, h('span', { class: 'tag ' + (x.state === 'waiting' ? 'acc' : x.state.startsWith('ended') ? 'bad' : 'good') }, x.state))))),
      h('p', { class: 'note' }, 'The text above is exactly what one passkey signature approves. A session alone never approves.'),
      h('p', {}, waiting ? (signPath(c.sign_at) ? h('a', { class: 'btn primary', href: signPath(c.sign_at) }, 'Sign with passkey') : host.ref.show('web:' + c.sign_at, { title: 'Sign with passkey' })) : null, ' ', h('a', { class: 'btn quiet', href: './' }, 'Back to the inbox')));
  };
  const inbox = async () => {
    const r = await host.call('inbox');
    if (stop) return;
    if (!r.ok) { el.replaceChildren(...head('Inbox', 'Approvals.'), h('p', { class: 'note' }, r.status === 503 ? 'The inbox cell is not answering; trying again.' : 'Not open to you.')); return; }
    const rows = r.data;
    const grants = rows.filter(x => x.kind === 'grant').length;
    const bundles = [...new Set(rows.filter(x => x.bundle).map(x => x.bundle))];
    el.replaceChildren(...head('Inbox', 'Approvals.'),
      h('div', { class: 'stats' },
        h('div', { class: 'glass ticks stat' }, h('div', { class: 'label' }, h('i', {}), 'Waiting'), h('div', { class: 'num acc' }, rows.length), h('div', { class: 'sub' }, 'for you to decide')),
        h('div', { class: 'glass stat' }, h('div', { class: 'label' }, h('i', {}), 'Grants'), h('div', { class: 'num' }, grants), h('div', { class: 'sub' }, bundles.length + ' bundle' + (bundles.length === 1 ? '' : 's'))),
        h('div', { class: 'glass stat' }, h('div', { class: 'label' }, h('i', {}), 'Decisions'), h('div', { class: 'num' }, rows.length - grants), h('div', { class: 'sub' }, 'open sheets'))),
      rows.length ? h('div', { class: 'glass list sys' }, ...rows.map(x => h('div', { class: 'row' },
        h('div', {}, h('div', { class: 'l' }, x.line), h('div', { class: 'm' }, [x.title, x.requester && ('to ' + x.requester), x.node, until(x.until)].filter(Boolean).join(' · '), x.dry_run ? h('span', { class: 'tag acc' }, 'dry run') : null)),
        h('div', { class: 'r' }, x.bundle ? h('a', { class: 'btn', href: '?b=' + encodeURIComponent(x.bundle) }, 'Open card') : h('span', { class: 'tag' }, x.kind)))))
        : h('div', { class: 'glass empty sys' }, 'Nothing waits for you.'));
  };
  const b = new URLSearchParams(location.search).get('b');
  const run = async () => { if (stop) return; await (b ? card(b) : inbox()); if (!stop) timer = setTimeout(run, 5000); };
  run();
  return () => { stop = true; clearTimeout(timer); };
}

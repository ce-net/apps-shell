// apps.shell::Mount: a `view:` ref mounts that view. Slice 1 carries approve.inbox's Card (one bundle), Pending and
// Request (the ask a refusal points to).
export const view = { cell: 'apps.shell', name: 'Mount', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const r = host.ref.parse(host.opened.ref);
  const t = r ? r.target : '';
  const [path, qs] = t.split('?');
  const q = new URLSearchParams(qs || '');
  const view = path.split('::')[1] || '';
  if (path.startsWith('approve.inbox@1::') && view === 'Request') {
    const need = q.get('need') || '';
    root.replaceChildren(h('div', { class: 'crumb' }, 'approve.inbox · request'), h('h1', { class: 'display refname' }, 'Ask for ' + need),
      h('p', { class: 'note' }, 'The right ' + need + ' opens this for you. Ask whoever gave you access on ce-net for it; when it is granted, it arrives as an approval and this opens without a reload.'),
      h('p', { class: 'note' }, 'Asking from this page is not built yet (approve.inbox has no Ask op).'));
    return () => {};
  }
  if (path.startsWith('approve.inbox@1::') && view === 'Card') {
    let stop = false;
    (async () => {
      const res = await host.call('card', { b: q.get('b') || '' });
      if (stop || !host.kit.state(root, res, 'This card')) return;
      const c = res.data;
      root.replaceChildren(h('div', { class: 'crumb' }, 'approve.inbox · card ', h('b', {}, c.id)), h('h1', { class: 'display refname' }, c.title),
        h('div', { class: 'glass list sys' }, ...c.rows.map(x => h('div', { class: 'row' },
          h('div', {}, h('div', { class: 'l' }, x.n + '. ' + x.line), h('pre', { class: 'signed' }, x.text)),
          h('div', { class: 'r' }, h('span', { class: 'tag ' + (x.state === 'waiting' ? 'acc' : 'good') }, x.state))))),
        h('p', { class: 'note' }, 'Signing is one passkey assertion on the Approvals tab; a session never approves.'));
    })();
    return () => { stop = true; };
  }
  if (path.startsWith('approve.inbox@1::') && view === 'Pending') {
    return host.watch('inbox', {}, res => {
      if (!host.kit.state(root, res, 'Approvals')) return;
      root.replaceChildren(h('h1', { class: 'display refname' }, 'Pending'),
        h('div', { class: 'chipline' }, ...res.data.map(x => host.ref.show(x.bundle ? 'view:approve.inbox@1::Card?b=' + x.bundle : 'view:approve.inbox@1::Pending', { title: x.line }))));
    });
  }
  root.replaceChildren(h('p', { class: 'note' }, 'This shell does not carry the view ' + t + ' yet.'));
  return () => {};
}

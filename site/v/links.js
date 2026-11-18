// Home slot: the viewer's pinned refs (home.links rows on the node). Any kind; each opens through open.with.
export const view = { cell: 'home.shell', name: 'Links', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const f = host.frame;
  return host.watch('links', {}, res => {
    if (!host.kit.state(f, res, 'Links')) return;
    const rows = res.data.links || [];
    f.count(rows.length);
    if (!rows.length) { f.state('empty', 'No pinned refs yet.'); f.body.replaceChildren(); return; }
    f.state('ok', '');
    f.body.replaceChildren(...rows.map(l => host.ref.show(l.ref, { title: l.title, sub: l.note, block: true })));
  }, 60000);
}

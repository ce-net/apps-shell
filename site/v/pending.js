// Home slot (and the Core tab's strip): what waits for this viewer in approve.inbox, rows cut by the cell's own gate.
// Each row opens its card as a `view:` ref through open.with; a row about a record also shows that record's ref.
export const view = { cell: 'approve.inbox', name: 'Pending', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const f = host.frame;
  return host.watch('inbox', {}, res => {
    if (!host.kit.state(f, res, 'Approvals')) return;
    const rows = res.data || [];
    f.count(rows.length);
    if (!rows.length) { f.state('empty', 'Nothing waits for you.'); f.body.replaceChildren(); return; }
    f.state('ok', '');
    f.body.replaceChildren(...rows.slice(0, 8).map(x => h('div', { class: 'item' },
      h('span', { class: 'when' }, x.kind || ''),
      host.ref.show(x.bundle ? 'view:approve.inbox@1::Card?b=' + x.bundle : 'view:approve.inbox@1::Pending', { title: x.line, sub: x.requester ? 'to ' + x.requester : '' }),
      host.ref.parse(x.about || '') ? host.ref.show(x.about) : h('span', {}))),
      rows.length > 8 ? h('p', { class: 'note' }, (rows.length - 8) + ' more in Approvals.') : '');
  }, 15000);
}

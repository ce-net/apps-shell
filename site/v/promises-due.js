// Home slot: promises due and overdue (crm.promises List, status Open, as this viewer). Each is a `promise:` ref.
export const view = { cell: 'crm.promises', name: 'PromisesDue', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const f = host.frame;
  const date = ms => new Date(ms).toISOString().slice(5, 10);
  return host.watch('promises', {}, res => {
    if (!host.kit.state(f, res, 'Promises')) return;
    const rows = res.data.promises || [];
    const overdue = rows.filter(p => p.overdue).length;
    f.count(rows.length);
    if (!rows.length) { f.state('empty', 'Nothing promised is due this week.'); f.body.replaceChildren(); return; }
    f.state('ok', overdue ? overdue + ' overdue' : '');
    f.body.replaceChildren(...rows.map(p => h('div', { class: 'item' + (p.overdue ? ' overdue' : '') },
      h('span', { class: 'when' }, date(p.due)),
      host.ref.show('promise:' + p.id, { title: p.text, sub: p.to_name || p.to || p.company }),
      p.overdue ? h('span', { class: 'tag bad' }, 'overdue') : h('span', { class: 'tag' }, 'due'))));
  }, 60000);
}

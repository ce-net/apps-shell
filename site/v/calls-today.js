// Home slot: today's calls (crm.calls List as this viewer). Each call is a `call:` ref opened through open.with.
export const view = { cell: 'crm.calls', name: 'CallsToday', speaks: [2], kit: 1 };

const STATUS = { Booked: 'good', Held: '', Missed: 'bad', Cancelled: 'bad', Moved: '', Proposed: 'acc', Event: '' };

export function mount(root, host) {
  const { h } = host;
  const f = host.frame;
  const day = () => { const d = new Date(); d.setHours(0, 0, 0, 0); const from = Math.floor(d.getTime() / 1000); return { from, to: from + 86400 }; };
  const hm = s => new Date(s * 1000).toTimeString().slice(0, 5);
  return host.watch('calls', day(), res => {
    if (!host.kit.state(f, res, 'Calls')) return;
    const rows = res.data.calls || [];
    f.count(rows.length);
    if (!rows.length) { f.state('empty', 'No calls today.'); f.body.replaceChildren(); return; }
    f.state('ok', '');
    f.body.replaceChildren(...rows.map(c => h('div', { class: 'item' },
      h('span', { class: 'when' }, hm(c.at_s)),
      host.ref.show('call:' + c.id, { title: c.person + (c.company ? ' · ' + c.company : ''), sub: c.channel }),
      h('span', { class: 'tag ' + (STATUS[c.status] || '') }, c.status))));
  }, 30000);
}

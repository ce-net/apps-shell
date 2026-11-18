// Home slot: site.cellmap Summary. Nodes up, replicas live against wanted, the red cells; each a `cell:` or `node:` ref.
export const view = { cell: 'site.cellmap', name: 'Summary', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const f = host.frame;
  return host.watch('cellmap', {}, res => {
    if (!host.kit.state(f, res, 'The cell map')) return;
    const m = res.data;
    const cells = m.systems.flatMap(s => s.cells);
    const live = cells.reduce((a, c) => a + Math.min(c.live, c.want), 0), want = cells.reduce((a, c) => a + c.want, 0);
    const down = cells.filter(c => c.live === 0);
    f.count(down.length || null);
    f.state('ok', 'As of ' + new Date(m.as_of_ms).toTimeString().slice(0, 8) + ', from each node\'s cell.install.');
    f.body.replaceChildren(
      h('div', { class: 'stats' },
        h('div', { class: 'stat' }, h('div', { class: 'label' }, 'Nodes up'), h('div', { class: 'num' }, m.nodes.filter(n => n.up).length, h('small', {}, '/ ' + m.nodes.length))),
        h('div', { class: 'stat' }, h('div', { class: 'label' }, 'Replicas'), h('div', { class: 'num' }, live, h('small', {}, '/ ' + want))),
        h('div', { class: 'stat' }, h('div', { class: 'label' }, 'Not running'), h('div', { class: 'num' + (down.length ? ' acc' : '') }, down.length))),
      h('div', { class: 'chipline' }, ...m.nodes.map(n => host.ref.show('node:' + n.name, { title: n.name, sub: n.up ? (n.rtt_ms != null ? n.rtt_ms + ' ms' : 'up') : 'down' }))),
      down.length ? h('div', { class: 'chipline' }, ...down.slice(0, 12).map(c => host.ref.show('cell:' + c.cell + '@' + c.major, { title: c.cell, sub: 'not running' }))) : '');
  }, 20000);
}

// site.cellmap::Cell and ::Node: "browse all cells". One cell's replicas per node, or one node's cells, from
// site.cellmap as this viewer; every cell and node named is itself a ref.
export const view = { cell: 'site.cellmap', name: 'Cell', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const { cell, node } = host.props;
  return host.watch('cellmap', {}, res => {
    if (!host.kit.state(root, res, 'The cell map')) return;
    const m = res.data;
    const rows = m.systems.flatMap(s => s.cells.map(c => ({ ...c, system: s.name })));
    if (cell) {
      const c = rows.find(x => x.cell === cell);
      if (!c) { root.replaceChildren(h('h1', { class: 'display refname' }, cell), h('p', { class: 'note' }, 'No cell system of ce-net places ' + cell + '.')); return; }
      root.replaceChildren(
        h('div', { class: 'crumb' }, 'cell · system ', h('b', {}, c.system)),
        h('h1', { class: 'display refname' }, c.cell + '@' + c.major),
        h('p', { class: 'sub' }, c.live + ' of ' + c.want + ' replicas live' + (c.drift ? ' · drift' : '')),
        h('div', { class: 'glass list sys' }, ...(c.on.length ? c.on.map(i => h('div', { class: 'row' },
          h('div', {}, host.ref.show('node:' + i.node, { title: i.node })),
          h('div', { class: 'r' }, h('span', { class: 'tag ' + (i.processes > 0 ? 'good' : 'bad') }, i.processes > 0 ? i.source + ' ' + (i.sha256 || '').slice(0, 7) : 'stopped'))))
          : [h('div', { class: 'empty' }, 'Running nowhere.')])),
        h('p', { class: 'note' }, 'As of ' + new Date(m.as_of_ms).toTimeString().slice(0, 8) + '.'));
    } else {
      const n = m.nodes.find(x => x.name === node);
      const here = rows.filter(c => c.on.some(i => i.node === node));
      root.replaceChildren(
        h('div', { class: 'crumb' }, 'node'),
        h('h1', { class: 'display refname' }, node),
        h('p', { class: 'sub' }, n ? (n.up ? 'up' + (n.rtt_ms != null ? ', ' + n.rtt_ms + ' ms' : '') : 'down' + (n.why ? ': ' + n.why : '')) + ' · ' + (n.id || '').slice(0, 16) : 'not a node of ce-net'),
        h('div', { class: 'chipline' }, ...here.map(c => host.ref.show('cell:' + c.cell + '@' + c.major, { title: c.cell, sub: c.on.find(i => i.node === node).processes > 0 ? 'running' : 'stopped' }))));
    }
  }, 15000);
}

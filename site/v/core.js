// The Core tab: site.cellmap, every cell of every cell system this viewer may see, per node; and a Pending strip
// from approve.inbox when the viewer may open Approvals.
export const view = { cell: 'site.cellmap', name: 'Core', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const el = { replaceChildren: (...k) => root.replaceChildren(...k.filter(x => x != null && x !== false)) };
  let stop = false, timer = 0;
  const sha = s => s ? s.slice(0, 7) : '';
  const run = async () => {
    if (stop) return;
    const r = await host.call('cellmap');
    const p = await host.call('inbox');
    const pending = p.ok ? p.data : null;
    if (stop) return;
    const head = [h('div', { class: 'crumb' }, 'ce / ', h('b', {}, 'Core'), ' / cells'), h('h1', { class: 'display' }, 'Core cells.')];
    // the Architecture view, for a viewer who may open it (tabs.json views)
    if ((host.views || []).includes('architecture')) head.push(h('div', { class: 'glass list sys' }, h('div', { class: 'row' },
      h('div', {}, h('div', { class: 'l' }, 'Architecture'), h('div', { class: 'm' }, 'the whole network drawn from what its cells report: layers, systems, machines')),
      h('div', { class: 'r' }, h('a', { class: 'btn', href: '?view=architecture' }, 'Open')))));
    if (!r.ok) { el.replaceChildren(...head, h('p', { class: 'note' }, 'The cell map is not answering; trying again.')); timer = setTimeout(run, 5000); return; }
    const m = r.data;
    const cells = m.systems.flatMap(s => s.cells);
    const live = cells.reduce((a, c) => a + Math.min(c.live, c.want), 0), want = cells.reduce((a, c) => a + c.want, 0);
    const up = m.nodes.filter(n => n.up).length;
    el.replaceChildren(...head,
      h('div', { class: 'stats' },
        h('div', { class: 'glass ticks stat' }, h('div', { class: 'label' }, h('i', { class: 'g' }), 'Replicas live'), h('div', { class: 'num acc' }, live, h('small', {}, ' / ' + want)), h('div', { class: 'sub' }, cells.length + ' cells in ' + m.systems.length + ' systems')),
        h('div', { class: 'glass stat' }, h('div', { class: 'label' }, h('i', {}), 'Systems whole'), h('div', { class: 'num' }, m.systems.filter(s => s.ok).length, h('small', {}, ' / ' + m.systems.length)), h('div', { class: 'sub' }, 'every cell at its replica count')),
        h('div', { class: 'glass stat' }, h('div', { class: 'label' }, h('i', { class: up < m.nodes.length ? 'r' : 'g' }), 'Nodes up'), h('div', { class: 'num' }, up, h('small', {}, ' / ' + m.nodes.length)), h('div', { class: 'sub' }, m.nodes.filter(n => !n.up).map(n => n.name).join(', ') || 'all answer'))),
      pending && pending.length ? h('div', { class: 'glass list sys' }, h('h2', {}, h('b', {}, 'Pending'), h('a', { class: 'tag acc', href: '/approvals/' }, pending.length + ' waiting')),
        ...pending.slice(0, 3).map(x => h('div', { class: 'row' }, h('div', {}, h('div', { class: 'l' }, x.line), h('div', { class: 'm' }, [x.about, x.requester && ('to ' + x.requester)].filter(Boolean).join(' · ')))))) : null,
      ...m.systems.map(s => h('div', { class: 'glass list sys' },
        h('h2', {}, h('b', {}, s.name), h('span', {}, (s.tab || 'no tab') + ' · ', h('span', { class: 'tag ' + (s.ok ? 'good' : 'bad') }, s.live + ' / ' + s.want))),
        ...s.cells.map(c => h('div', { class: 'row' },
          h('div', {}, h('div', { class: 'l' }, host.ref.show('cell:' + c.cell + '@' + c.major, { title: c.cell + '@' + c.major })), h('div', { class: 'm' }, c.live + ' of ' + c.want + ' replicas live', c.drift ? h('span', { class: 'tag bad' }, 'drift') : null)),
          h('div', { class: 'r on' }, ...(c.on.length ? c.on.map(i => h('span', { class: 'tag ' + (i.processes > 0 ? 'good' : 'bad') }, i.node + ' ' + (i.processes > 0 ? i.source + ' ' + sha(i.sha256) : 'stopped'))) : [h('span', { class: 'tag bad' }, 'nowhere')])))))),
      h('div', { class: 'glass list sys' }, h('h2', {}, h('b', {}, 'Nodes')), ...m.nodes.map(n => h('div', { class: 'row' },
        h('div', {}, h('div', { class: 'l' }, host.ref.show('node:' + n.name, { title: n.name })), h('div', { class: 'm' }, n.id.slice(0, 16) + (n.why ? ' · ' + n.why.slice(0, 90) : ''))),
        h('div', { class: 'r' }, h('span', { class: 'tag ' + (n.up ? 'good' : 'bad') }, n.up ? (n.rtt_ms != null ? n.rtt_ms + ' ms' : 'up') : 'down'))))),
      h('p', { class: 'note' }, 'As of ' + new Date(m.as_of_ms).toISOString().slice(11, 19) + ' UTC, from each node\'s cell.install.'));
    timer = setTimeout(run, 5000);
  };
  run();
  return () => { stop = true; clearTimeout(timer); };
}

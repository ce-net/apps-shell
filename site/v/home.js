// The Home tab: home.shell@1 Layout for this viewer and device, each slot another cell's view mounted by the shell in
// a SlotFrame (host.slot). This view draws no panel itself. A slot with nothing for this viewer is not in the layout;
// one that gets something appears at the next ask (every 20 s) without a reload.
export const view = { cell: 'home.shell', name: 'Home', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  let stop = false, timer = 0, key = '';
  const mountedSlots = new Map(); // name -> element
  const lede = h('div', { class: 'lede' }, h('div', {}, h('div', { class: 'crumb' }, 'ce / ', h('b', {}, 'Home')), h('h1', { class: 'display' }, 'Today.')),
    h('p', { class: 'sub' }));
  const grid = h('div', { class: 'home' });
  const cols = { main: h('div', { class: 'col', 'data-area': 'main' }), side: h('div', { class: 'col', 'data-area': 'side' }) };
  const note = h('p', { class: 'note' }, 'Asking home.shell for your layout.');
  root.replaceChildren(lede, note, grid);
  const dayRange = () => { const d = new Date(); d.setHours(0, 0, 0, 0); const from = Math.floor(d.getTime() / 1000); return { from, to: from + 86400 }; };

  const run = async () => {
    if (stop) return;
    const res = await host.call('layout', { device: host.device, ...dayRange() });
    if (stop) return;
    if (!res.ok) {
      note.hidden = false;
      note.textContent = res.status === 403 ? 'Home is not open to you.' : res.timeout ? 'home.shell did not answer in 25 s; asking again.' : 'home.shell is not answering (' + ((res.data && res.data.error) || res.status) + '); asking again.';
      timer = setTimeout(run, 5000);
      return;
    }
    note.hidden = true;
    const slots = res.data.slots || [];
    lede.querySelector('.sub').textContent = slots.length + ' slot' + (slots.length === 1 ? '' : 's') + ' for you, laid out by home.shell' + (res.data.took_ms != null ? ' in ' + res.data.took_ms + ' ms' : '') + '.';
    const k = JSON.stringify(slots.map(s => [s.name, s.view, s.area]));
    if (k !== key) {
      key = k;
      const one = host.device === 'Phone' || !slots.some(s => s.area === 'Side');
      grid.className = 'home' + (one ? ' one' : '');
      const want = new Set(slots.map(s => s.name));
      for (const [n, el] of mountedSlots) if (!want.has(n)) { el.unmount && el.unmount(); el.remove(); mountedSlots.delete(n); }
      cols.main.replaceChildren(); cols.side.replaceChildren();
      for (const s of slots) {
        let el = mountedSlots.get(s.name);
        if (!el) { el = host.slot(s); mountedSlots.set(s.name, el); }
        (s.area === 'Side' && !one ? cols.side : cols.main).append(el);
      }
      grid.replaceChildren(cols.main, one ? '' : cols.side);
      if (!slots.length) grid.replaceChildren(h('div', { class: 'glass empty' }, 'Nothing is waiting for you, and nothing is open to you here yet.'));
    }
    timer = setTimeout(run, 20000);
  };
  run();
  return () => { stop = true; clearTimeout(timer); for (const el of mountedSlots.values()) el.unmount && el.unmount(); };
}

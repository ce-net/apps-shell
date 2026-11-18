// ref.view::Record: one CRM person, promise or call, or a call recording's passage, read as this viewer from the cell
// that owns it (crm.people, crm.promises, crm.calls; search.hybrid for a recording). Moved here from the Search tab.
// Values that are outside addresses become `web:` refs; refs written in the text become chips.
export const view = { cell: 'ref.view', name: 'Record', speaks: [1], kit: 1 };

const KIND = { person: 'CRM person', promise: 'Promise', call: 'Call', meet: 'Call recording' };

export function mount(root, host) {
  const { h } = host;
  let stop = false;
  const { kind, id } = host.props;
  root.replaceChildren(h('p', { class: 'note' }, 'Reading ' + kind + ':' + id + '.'));
  const value = v => /^https?:\/\/\S+$/.test(v) ? host.ref.show('web:' + v) : host.ref.parse(v) ? host.ref.show(v) : v;
  const withRefs = text => {
    const found = host.ref.find(text);
    if (!found.length) return [text];
    const out = []; let at = 0;
    for (const f of found) { out.push(text.slice(at, f.start), host.ref.show(f.ref)); at = f.end; }
    out.push(text.slice(at));
    return out;
  };
  (async () => {
    const res = await host.call('rec', { r: kind + ':' + id });
    if (stop) return;
    if (!host.kit.state(root, res, 'This record')) return;
    const d = res.data;
    root.replaceChildren(
      h('div', { class: 'crumb' }, KIND[kind] || 'Record', ' · from ', h('b', {}, d.cell)),
      h('h1', { class: 'display refname' }, d.title || KIND[kind]),
      h('dl', { class: 'kv glass', style: 'padding:12px' }, ...(d.fields || []).flatMap(([k, v]) => [h('dt', {}, k), h('dd', {}, value(String(v)))])),
      d.text ? h('pre', { class: 'glass code prose', tabindex: '0' }, ...withRefs(d.text)) : '',
      d.list && d.list.length ? h('div', { class: 'glass list sys' }, h('h2', {}, h('b', {}, d.list_title || 'List'), h('span', {}, String(d.list.length))),
        ...d.list.map(t => h('div', { class: 'row' }, h('div', {}, h('div', { class: 'l' }, ...withRefs(t)))))) : '',
      d.note ? h('p', { class: 'note' }, d.note) : '');
  })();
  return () => { stop = true; };
}

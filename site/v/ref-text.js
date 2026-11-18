// ref.view::Text: one file through the cell reference system, as this viewer (ref.resolve says where it lives and its
// hash, ref.pin hands over only those bytes). Code, text and markdown. Moved here from the Search tab.
export const view = { cell: 'ref.view', name: 'Text', speaks: [1], kit: 1 };

export const kb = n => n < 1024 ? n + ' B' : (n / 1024).toFixed(1) + ' KB';

export function meta(h, d) {
  return h('dl', { class: 'kv glass', style: 'padding:12px' },
    h('dt', {}, 'file'), h('dd', {}, d.repo + '/' + d.path),
    h('dt', {}, 'size'), h('dd', {}, kb(d.size)),
    h('dt', {}, 'ce-ref id'), h('dd', { class: 'mono' }, d.id),
    h('dt', {}, 'pinned'), h('dd', { class: 'mono' }, d.ref),
    h('dt', {}, 'lives on'), h('dd', {}, d.node + ' · ' + d.source + ' · read by ' + d.via.join(' and ')));
}

export function mount(root, host) {
  const { h } = host;
  let stop = false;
  const r = host.props.ref;
  root.replaceChildren(h('p', { class: 'note' }, 'Reading ' + r + ' through ref.resolve and ref.pin.'));
  (async () => {
    const res = await host.call('ref', { r });
    if (stop) return;
    if (!host.kit.state(root, res, 'This file')) return;
    const d = res.data;
    root.replaceChildren(
      h('h1', { class: 'display refname' }, d.path.split('/').pop()),
      meta(h, d),
      d.binary ? h('p', { class: 'note' }, 'A binary file (' + kb(d.size) + '); its bytes match ' + d.sha256 + '.')
        : h('pre', { class: 'glass code', tabindex: '0' }, d.text),
      d.truncated ? h('p', { class: 'note' }, 'Showing the first 512 KB.') : '');
  })();
  return () => { stop = true; };
}

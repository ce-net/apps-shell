// ref.view::Raw, the floor: for any type no other opener takes, what the cell reference system knows of the thing
// (name, size, hash, holder), never its bytes drawn as something they are not.
import { kb, meta } from './ref-text.js';
export const view = { cell: 'ref.view', name: 'Raw', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  let stop = false;
  const r = host.props.ref;
  const p = host.ref.parse(r);
  const head = [h('h1', { class: 'display refname' }, p ? (p.kind === 'repo' ? p.target.split('/').pop() : p.target) : r),
    host.props.cached ? h('p', { class: 'note' }, 'Shown from this page\'s cache: open.with is not answering.') : ''];
  if (!p || p.kind !== 'repo') {
    root.replaceChildren(...head, h('dl', { class: 'kv glass', style: 'padding:12px' },
      h('dt', {}, 'kind'), h('dd', {}, p ? p.kind : '?'), h('dt', {}, 'names'), h('dd', {}, p ? p.target : r),
      h('dt', {}, 'ce-ref'), h('dd', { class: 'mono' }, r)),
      h('p', { class: 'note' }, 'No view of this kind answers for you now; this is what the ref itself says.'));
    return () => {};
  }
  root.replaceChildren(...head, h('p', { class: 'note' }, 'Asking ref.resolve and ref.pin.'));
  (async () => {
    const res = await host.call('ref', { r });
    if (stop) return;
    if (!host.kit.state(root, res, 'This file')) return;
    const d = res.data;
    root.replaceChildren(...head, meta(h, d),
      h('p', { class: 'note' }, (d.binary ? 'Binary' : 'Not a type any view here draws') + ': ' + kb(d.size) + ', sha256 ' + d.sha256 + '.'));
  })();
  return () => { stop = true; };
}

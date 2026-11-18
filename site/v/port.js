// apps.shell::Port: a static site of ours framed under its real ref (assembly:huebot, map:...) until its real view
// exists (cad.view, maps.view at rank 50 take over; no ref changes). Same sign-in, same origin for huebot.
export const view = { cell: 'apps.shell', name: 'Port', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const p = host.props;
  if (!p.frame) { root.replaceChildren(h('p', { class: 'note' }, 'No site of ours shows ' + host.opened.ref + ' yet.')); return () => {}; }
  const fr = h('iframe', { class: 'portframe', src: p.frame, title: p.site, loading: 'lazy', referrerpolicy: 'no-referrer', sandbox: 'allow-scripts allow-same-origin allow-forms allow-popups' });
  root.replaceChildren(
    h('div', { class: 'crumb' }, 'port · ', h('b', {}, p.site)),
    p.step ? h('p', { class: 'sub' }, 'Step ' + p.step) : '',
    fr,
    h('p', { class: 'note' }, 'Shown by its own site until a cell view of ' + host.opened.ty + ' exists. The ref stays ' + host.opened.ref + '.'));
  return () => { fr.remove(); };
}

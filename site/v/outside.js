// apps.shell::Outside: the outside web, the one kind on which a person sees an address. Names the site, says it is
// outside ce-net, and hands the address to the browser only when asked (host.handoff, the shell's one such path).
export const view = { cell: 'apps.shell', name: 'Outside', speaks: [1], kit: 1 };

export function mount(root, host) {
  const { h } = host;
  const { address, site } = host.props;
  root.replaceChildren(h('div', { class: 'outside' },
    h('div', { class: 'crumb' }, 'outside ce-net'),
    h('div', { class: 'site' }, site || 'an outside site'),
    h('p', { class: 'note' }, 'This is on the outside web, not in ce-net: nothing here is checked by your capabilities, and the site sees your browser.'),
    h('code', { class: 'mono', style: 'overflow-wrap:anywhere' }, address),
    h('button', { type: 'button', class: 'btn primary', onclick: () => host.handoff(address) }, 'Open outside, in a new browser tab')));
  return () => {};
}

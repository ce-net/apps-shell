// apps.shell's page: the tab bar is whatever tabs.json says this viewer has now (allowed by their chain, and a
// replica of the tab's shell answering). Polled every 3 s, so a grant, an end or a last replica going down changes
// the bar on an open page without a reload. This file names no tab and no system.
const h = (t, a = {}, ...kids) => {
  const e = document.createElement(t);
  for (const [k, v] of Object.entries(a)) {
    if (k === 'class') e.className = v; else if (k.startsWith('on')) e.addEventListener(k.slice(2), v); else if (v !== false && v != null) e.setAttribute(k, v === true ? '' : v);
  }
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k.nodeType ? k : document.createTextNode(String(k)));
  return e;
};
window.ceh = h;

const root = document.documentElement;
const themeBtn = document.getElementById('theme');
function setTheme(t) {
  if (t === 'light') root.setAttribute('data-theme', 'light'); else root.removeAttribute('data-theme');
  themeBtn.textContent = t === 'light' ? 'Dark' : 'Light';
  try { localStorage.setItem('ce-theme', t); } catch (_) {}
}
let saved = 'dark';
try { saved = localStorage.getItem('ce-theme') || 'dark'; } catch (_) {}
setTheme(saved);
themeBtn.addEventListener('click', () => setTheme(root.getAttribute('data-theme') === 'light' ? 'dark' : 'light'));

const rail = document.getElementById('tabs');
const main = document.getElementById('main');
let mounted = null; // {view, stop}
let last = '';

const icon = () => {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 20 20'); s.setAttribute('fill', 'none'); s.setAttribute('stroke', 'currentColor'); s.setAttribute('stroke-width', '1.4');
  const r = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
  r.setAttribute('x', '4'); r.setAttribute('y', '4'); r.setAttribute('width', '12'); r.setAttribute('height', '12');
  s.append(r); return s;
};

function say(crumb, title, line, link) {
  if (mounted) { mounted.stop(); mounted = null; }
  main.replaceChildren(h('div', { class: 'crumb' }, 'ce / ', h('b', {}, crumb)), h('h1', { class: 'display' }, title),
    h('p', { class: 'sub' }, line), link ? h('p', {}, h('a', { class: 'btn primary', href: link[1] }, link[0])) : null);
}

// The host of a view that declares `view = { cell, ... }`: its data comes from that one cell, through this gateway's
// relay (`api/cell/<cell>/<Op>`, as the signed-in person; the cell's own gate decides), never from a URL the view builds.
// call(op, req) -> Promise of the answer (rejects with an Error that has .status and .refused);
// watch(op, req, onDelta) long-polls (req may be a function giving the current request): onDelta(answer) for each
// answer, onDelta(null, error) for a failure, then retries with backoff; returns stop().
// open(target): https links open in a new tab, a ce-ref opens in the reference viewer (the search tab's).
const sleep = ms => new Promise(r => setTimeout(r, ms));
function hostFor(view, tab) {
  const host = { h, tab };
  if (!view || typeof view.cell !== 'string' || !/^[a-z0-9.-]+$/.test(view.cell)) return host;
  const cell = view.cell;
  const call = async (op, req, signal) => {
    if (!/^[A-Za-z]{1,32}$/.test(op)) throw new Error('bad op');
    let r;
    try {
      r = await fetch('api/cell/' + cell + '/' + op, { method: 'POST', headers: { 'content-type': 'application/json' }, credentials: 'same-origin', cache: 'no-store', body: JSON.stringify(req == null ? null : req), signal });
    } catch (e) { if (signal && signal.aborted) throw e; throw Object.assign(new Error('no connection'), { status: 0 }); }
    let body = null;
    try { body = await r.json(); } catch (_) {}
    if (!r.ok) throw Object.assign(new Error((body && body.error) || 'HTTP ' + r.status), { status: r.status, refused: r.status === 403 });
    return body;
  };
  host.call = (op, req) => call(op, req);
  host.watch = (op, req, onDelta) => {
    let stopped = false, ctl = null, back = 1000;
    (async () => {
      while (!stopped) {
        const t0 = Date.now();
        ctl = new AbortController();
        try {
          const d = await call(op, typeof req === 'function' ? req() : req, ctl.signal);
          back = 1000;
          if (!stopped) onDelta(d);
          const spent = Date.now() - t0;
          if (spent < 200) await sleep(200 - spent);
        } catch (e) {
          if (stopped) return;
          onDelta(null, e);
          await sleep(back);
          back = Math.min(back * 2, 30000);
        }
      }
    })();
    return () => { stopped = true; if (ctl) ctl.abort(); };
  };
  host.open = target => {
    const t = String(target || '');
    if (/^https:\/\//.test(t)) window.open(t, '_blank', 'noopener,noreferrer');
    else if (t) location.assign('/search/?' + (t.startsWith('repo:') ? 'ref=' : 'q=') + encodeURIComponent(t));
  };
  return host;
}

async function tick() {
  let r;
  try { r = await fetch('tabs.json', { cache: 'no-store', credentials: 'same-origin' }); } catch (_) { return; }
  if (r.status === 401) return say('sign in', 'Sign in.', 'Sign in to see what is open to you.', ['Sign in', '/login?next=' + encodeURIComponent(location.pathname)]);
  if (!r.ok) return;
  const d = await r.json();
  const key = JSON.stringify(d.tabs.map(t => [t.route, t.title, t.badge]));
  if (key !== last) {
    last = key;
    rail.replaceChildren(...d.tabs.map(t => h('a', { href: t.route, 'aria-current': t.route === d.here ? 'page' : false },
      icon(), t.title, t.badge ? h('span', { class: 'bdg' }, t.badge) : null)));
  }
  const here = d.tabs.find(t => t.route === d.here);
  if (!d.tabs.length) return say('apps', 'Nothing open.', 'No tab is open to you here. Ask for access, and it appears here when it is granted.');
  if (!here || !here.view) {
    if (mounted || !main.dataset.closed) { say('apps', 'Closed here.', 'This tab is not open to you now, or none of its cells answers. Your open tabs are on the left.'); main.dataset.closed = '1'; }
    return;
  }
  delete main.dataset.closed;
  if (mounted && mounted.view === here.view) return;
  if (mounted) { mounted.stop(); mounted = null; }
  const mod = await import('../' + here.view);
  main.replaceChildren();
  mounted = { view: here.view, stop: mod.mount(main, hostFor(mod.view, here)) };
}

tick();
setInterval(tick, 3000);

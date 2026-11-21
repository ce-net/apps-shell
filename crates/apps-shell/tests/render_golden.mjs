// Paints the golden Strips (tests/golden/*.json) with site/v/conv-usage.js against a minimal DOM and prints the markup,
// one element a line. tests/render.rs compares it with tests/golden/render-<case>.html (UPDATE_GOLDEN=1 rewrites them).
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
class El {
  constructor(tag) { this.nodeType = 1; this.tag = tag; this.attrs = []; this.kids = []; this.dataset = {}; }
  setAttribute(k, v) { this.attrs.push([k, String(v)]); }
  addEventListener() {}
  set className(v) { this.attrs.unshift(['class', v]); }
  append(...k) { this.kids.push(...k); }
}
class Tx { constructor(t) { this.t = t; this.nodeType = 3; } }
globalThis.document = {
  createElement: t => new El(t),
  createElementNS: (_, t) => new El(t),
  createTextNode: t => new Tx(t),
};
const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
function ser(n, d = 0) {
  const pad = '  '.repeat(d);
  if (n instanceof Tx) return pad + esc(n.t) + '\n';
  const a = n.attrs.map(([k, v]) => ` ${k}="${esc(v)}"`).join('');
  if (!n.kids.length) return `${pad}<${n.tag}${a}></${n.tag}>\n`;
  if (n.kids.every(k => k instanceof Tx)) return `${pad}<${n.tag}${a}>${n.kids.map(k => esc(k.t)).join('')}</${n.tag}>\n`;
  return `${pad}<${n.tag}${a}>\n${n.kids.map(k => ser(k, d + 1)).join('')}${pad}</${n.tag}>\n`;
}
const h = (t, a = {}, ...kids) => {
  const e = document.createElement(t);
  for (const [k, v] of Object.entries(a)) {
    if (k === 'class') e.className = v; else if (k.startsWith('on')) e.addEventListener(k.slice(2), v); else if (v !== false && v != null) e.setAttribute(k, v === true ? '' : v);
  }
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k.nodeType ? k : document.createTextNode(String(k)));
  return e;
};
const { paint } = await import(pathToFileURL(path.join(here, '../../../site/v/conv-usage.js')).href);
const out = {};
for (const name of ['normal', 'warn80', 'bad90', 'capped', 'not-offered', 'stale']) {
  const strip = JSON.parse(readFileSync(path.join(here, 'golden', name + '.json'), 'utf8'));
  out[name] = paint(h, strip).map(n => ser(n)).join('');
}
if (process.argv[2] === 'write') for (const [k, v] of Object.entries(out)) writeFileSync(path.join(here, 'golden', 'render-' + k + '.html'), v);
else process.stdout.write(JSON.stringify(out));

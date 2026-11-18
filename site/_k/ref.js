// ce-ref in the page (host.ref.parse / find / show): the same grammar as repo:open-with crates/ce-ref-text. Short
// `kind:target[@pin][#frag]` and full `ce-ref://kind/target[@pin][#frag]` parse to one ref; short() writes the short
// form, uri() the full one. A ref names a thing, never a place: an http address is only ever `web:<address>`.
const KINDS = ['repo', 'blob', 'media', 'doc', 'person', 'promise', 'call', 'meet', 'contract', 'cell', 'op', 'view', 'node', 'assembly', 'part',
  'connection', 'map', 'app', 'skill', 'standard', 'supplier', 'endpoint', 'port', 'cap', 'web', 'file'];
const SCHEME = 'ce-ref://';

function pinOf(s) {
  const hex = x => /^[0-9a-f]+$/.test(x);
  if (s.startsWith('sha256-')) return s.length === 71 && hex(s.slice(7)) ? { Sha256: s.slice(7) } : null;
  if (s.startsWith('asof-')) return /^[A-Za-z0-9._-]+$/.test(s.slice(5)) ? { AsOf: s.slice(5) } : null;
  const parts = s.split('.');
  const version = s.length > 0 && s.length <= 32 && parts.length <= 3 && parts.every(p => /^[0-9]+$/.test(p));
  if (version && (parts.length > 1 || s.length <= 3)) return { Version: s };
  return s.length >= 7 && s.length <= 40 && hex(s) ? { Commit: s } : null;
}
const pinText = p => !p ? '' : p.Commit || (p.Sha256 ? 'sha256-' + p.Sha256 : p.Version || (p.AsOf ? 'asof-' + p.AsOf : ''));

function parse(s, anyKind) {
  if (typeof s !== 'string') return null;
  s = s.trim();
  if (!s || s.length > 4096 || /[\s\x00-\x1f]/.test(s)) return null;
  let kind, rest;
  if (s.startsWith(SCHEME)) { const f = s.slice(SCHEME.length); const i = f.indexOf('/'); if (i < 0) return null; kind = f.slice(0, i); rest = f.slice(i + 1); }
  else { const i = s.indexOf(':'); if (i < 0) return null; kind = s.slice(0, i); rest = s.slice(i + 1); }
  if (!/^[a-z][a-z0-9-]{0,23}$/.test(kind)) return null;
  if (!anyKind && !KINDS.includes(kind)) return null;
  if (kind === 'web') return /^https?:\/\//.test(rest) ? { kind, target: rest, pin: null, frag: null } : null;
  let body = rest, frag = null;
  const h = rest.indexOf('#');
  if (h >= 0) { body = rest.slice(0, h); frag = rest.slice(h + 1) || null; }
  let target = body, pin = null;
  const a = body.lastIndexOf('@');
  if (a > 0) { const p = pinOf(body.slice(a + 1)); if (p) { target = body.slice(0, a); pin = p; } }
  if (!target || target.includes('://')) return null;
  return { kind, target, pin, frag };
}

const tail = r => r.target + (r.pin ? '@' + pinText(r.pin) : '') + (r.frag ? '#' + r.frag : '');

export const CeRef = {
  KINDS,
  parse: s => parse(s, false),
  short: r => r ? r.kind + ':' + tail(r) : '',
  uri: r => r ? SCHEME + r.kind + '/' + tail(r) : '',
  // what the Bar takes: a ce-ref, a cell path (crm.calls, crm.calls@2), or an http address (as web:)
  fromBar(s) {
    s = (s || '').trim();
    const r = parse(s, false);
    if (r) return r;
    if (/^https?:\/\/\S+$/.test(s)) return { kind: 'web', target: s, pin: null, frag: null };
    const m = /^([a-z][a-z0-9-]*(?:\.[a-z0-9-]+)+)(?:@(\d+))?$/.exec(s);
    if (m) return { kind: 'cell', target: m[1], pin: m[2] ? { Version: m[2] } : null, frag: null };
    return null;
  },
  // the wire form open.with answers with (serde of open-with-api CeRef)
  fromWire: w => w ? { kind: w.kind, target: w.target, pin: w.pin || null, frag: w.frag || null } : null,
  // a short, human title: the last path segment of a file, the id of a record, the site of the outside web
  title(r) {
    if (!r) return '';
    if (r.kind === 'web') return r.target.replace(/^https?:\/\//, '').split(/[/?#]/)[0];
    if (r.kind === 'repo' || r.kind === 'file') return r.target.split('/').pop() + (r.pin && r.pin.Commit ? ' @' + r.pin.Commit.slice(0, 7) : '');
    return r.target + (r.frag ? ' #' + r.frag : '');
  },
  // ranges of text that parse as a ref of a known kind: [{start, end, ref}]
  find(text) {
    const out = [];
    const re = /(^|[^A-Za-z0-9/-])([a-z][a-z0-9-]*:[^\s<>"`)\]]+)/g;
    let m;
    while ((m = re.exec(text || ''))) {
      let cand = m[2].replace(/[.,;:!?']+$/, '');
      const r = parse(cand, false);
      if (r) { const start = m.index + m[1].length; out.push({ start, end: start + cand.length, ref: r }); }
    }
    return out;
  },
};

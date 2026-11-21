// The Approvals tab. First the decisions: decision.card's board for this viewer (repo:decision-card; repo:ce
// docs/CRITICAL-DECISIONS-2026-11-21.md section 8): at most 5 cards, ranked, each with its options, the recommended
// one, why, and what happens without an answer; one line for what closed with a receipt; one for the sources. The
// cell decides every word and number; this module only paints the Board. Then, behind a link, approve.inbox's rows
// and one bundle's card as before. Signing is one passkey assertion where the card says; a session never approves.
const until = s => s ? new Date(s * 1000).toISOString().slice(0, 16).replace('T', ' ') + ' UTC' : '';
const day = s => s ? new Date(s * 1000).toISOString().slice(0, 10) : '';
const ago = s => { const d = Math.floor((Date.now() / 1000 - s) / 86400); return d <= 0 ? 'today' : d === 1 ? '1 day ago' : d + ' days ago'; };
const CLASS = { Money: 'money', Law: 'law', Message: 'message to a person', Delete: 'delete', Reversible: 'standing rule' };

export const view = { cell: 'approve.inbox', name: 'Approvals', speaks: [1], kit: 1 };

const CSS = `
.dc{display:flex;flex-direction:column;gap:var(--s-3);min-width:0}
.dc-card{border:1px solid var(--line);border-radius:var(--r);background:var(--surface);padding:var(--s-3) var(--s-4);display:flex;flex-direction:column;gap:var(--s-2);min-width:0}
.dc-card[data-first=true]{border-color:var(--acc, var(--line))}
.dc-top{display:flex;flex-wrap:wrap;gap:var(--s-2);align-items:baseline;font-size:var(--t-xs);color:var(--ink-3)}
.dc-q{font-size:var(--t-lg, 1.1rem);font-weight:600;color:var(--ink);overflow-wrap:anywhere;margin:0}
.dc-opts{display:flex;flex-direction:column;gap:var(--s-1);margin:0;padding:0;list-style:none}
.dc-opt{display:flex;gap:var(--s-2);align-items:baseline;font-size:var(--t-sm);color:var(--ink-2);overflow-wrap:anywhere;min-width:0}
.dc-opt b{font-family:var(--mono);color:var(--ink-3);flex:none}
.dc-opt[data-rec=true]{color:var(--ink)}
.dc-rec{border:1px solid var(--ok);color:var(--ok);border-radius:var(--r-pill);padding:0 var(--s-2);font-size:var(--t-2xs);line-height:16px;white-space:nowrap;flex:none}
.dc-why,.dc-no{font-size:var(--t-sm);color:var(--ink-2);overflow-wrap:anywhere}
.dc-no b{color:var(--warn);font-weight:500}
.dc-rank{font-family:var(--mono);font-variant-numeric:tabular-nums}
.dc-lines{font-size:var(--t-xs);color:var(--ink-3);margin:0;padding-left:var(--s-4);overflow-wrap:anywhere}
.dc-lines li{margin:2px 0}
.dc-act{display:flex;flex-wrap:wrap;gap:var(--s-2);align-items:center}
.dc-line{font-size:var(--t-sm);color:var(--ink-2);overflow-wrap:anywhere}
.dc-src{display:flex;flex-wrap:wrap;gap:var(--s-1) var(--s-3);font-size:var(--t-xs);color:var(--ink-3)}
.dc-src i{display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--faint);margin-right:4px}
.dc-src [data-s=live] i{background:var(--live)}.dc-src [data-s=stale] i{background:var(--warn)}.dc-src [data-s=down] i{background:var(--bad)}
@media (max-width:760px){.dc-card{padding:var(--s-3) 16px}}`;

let sheet = null;
function style() {
  if (sheet) return;
  try {
    sheet = new CSSStyleSheet();
    sheet.replaceSync(CSS);
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
  } catch (_) { sheet = {}; }
}

// sign_at is our own passkey page: kept as a path on this site; anything else is the outside web, a web: ref
const signPath = u => { const m = /^https:\/\/apps\.ce-net\.com(\/[^\s]*)$/.exec(u || ''); return m ? m[1] : (u && u.startsWith('/') ? u : null); };

export function mount(root, host) {
  const { h } = host;
  style();
  const el = { replaceChildren: (...k) => root.replaceChildren(...k.filter(x => x != null && x !== false)) };
  let stop = false, timer = 0;
  const head = (crumb, title) => [h('div', { class: 'crumb' }, 'ce / ', h('b', {}, 'Approvals'), ' / ', crumb), h('h1', { class: 'display' }, title)];
  const signLink = (u, label, primary) => u ? (signPath(u) ? h('a', { class: 'btn' + (primary ? ' primary' : ''), href: signPath(u) }, label) : host.ref.show('web:' + u, { title: label })) : null;

  const cardOf = (c, i) => h('div', { class: 'dc-card', 'data-first': String(i === 0), 'data-kind': c.kind },
    h('div', { class: 'dc-top' },
      h('span', { class: 'tag' + (c.kind === 'policy' ? ' acc' : '') }, CLASS[c.class] || c.class),
      h('span', { class: 'dc-rank', title: 'impact x urgency x (1 + blocked)' }, 'rank ' + c.rank.score + ' = ' + c.rank.impact + ' x ' + c.rank.urgency + ' x (1 + ' + c.rank.blocked + ')'),
      h('span', {}, c.asks.length + ' ask' + (c.asks.length === 1 ? '' : 's') + ', oldest ' + ago(c.oldest))),
    h('h2', { class: 'dc-q' }, c.question),
    h('ol', { class: 'dc-opts' }, ...c.options.map((o, j) => h('li', { class: 'dc-opt', 'data-rec': String(j === c.recommended) },
      h('b', {}, String.fromCharCode(65 + j)), h('span', {}, o.label), j === c.recommended ? h('span', { class: 'dc-rec' }, 'recommended') : null))),
    h('div', { class: 'dc-why' }, c.because),
    h('div', { class: 'dc-no' }, h('b', {}, 'No answer by ' + day(c.no_answer.by) + ': '), c.no_answer.then + '. ' + c.no_answer.cost + '.'),
    h('details', {}, h('summary', { class: 'dc-line' }, 'What each option signs, ask by ask'), h('ul', { class: 'dc-lines' }, ...c.lines.map(l => h('li', {}, l)))),
    h('div', { class: 'dc-act' }, signLink(c.sign_at, 'Answer with passkey', i === 0),
      h('span', { class: 'note' }, 'Today you sign where the asks were made; one tap on this card comes with decision.answer.')));

  const sourceOf = s => {
    const st = typeof s.state === 'string' ? 'live' : s.state.Stale ? 'stale' : 'down';
    const why = s.state && s.state.Down ? ': ' + s.state.Down.why : s.state && s.state.Stale ? ' ' + s.state.Stale.age_s + ' s old' : '';
    return h('span', { 'data-s': st, title: s.ms + ' ms' }, h('i', {}), s.cell + ' on ' + s.node + ' ' + st + why + ' (' + s.asks + ')');
  };

  const decisions = async () => {
    const r = await host.call('decisions');
    if (stop) return true;
    if (r.status === 403) return false;
    if (!r.ok) { el.replaceChildren(...head('Decisions', 'Decisions.'), h('p', { class: 'note' }, 'decision.card is not answering; trying again.'), h('p', {}, h('a', { class: 'btn quiet', href: '?all=1' }, 'Every waiting row'))); return true; }
    const b = r.data;
    const n = b.cards.length + b.more;
    el.replaceChildren(...head('Decisions', n ? n + (n === 1 ? ' decision waits' : ' decisions wait') + ' for you.' : 'Nothing waits for you.'),
      h('div', { class: 'dc' },
        ...b.cards.map(cardOf),
        b.more ? h('div', { class: 'dc-line' }, b.more + ' more, ranked lower; they show here as these are answered.') : null,
        h('div', { class: 'dc-line' }, b.closed + ' closed automatically with a receipt (dry runs, rows a newer signed bundle replaced, duplicates); '
          + b.waiting_on_rule + ' wait on the standing rule; ' + b.returned + ' returned to their lanes as not an owner decision; ' + b.asks + ' asks read.'),
        h('div', { class: 'dc-src' }, ...b.sources.map(sourceOf)),
        b.not_connected.length ? h('div', { class: 'dc-line' }, 'Not collected yet: ' + b.not_connected.join(', ') + '.') : null,
        h('p', {}, h('a', { class: 'btn quiet', href: '?all=1' }, 'Every waiting row'))));
    return true;
  };

  const card = async (id) => {
    const r = await host.call('card', { b: id });
    if (!r.ok) { el.replaceChildren(...head('Card', 'No such bundle.'), h('p', {}, h('a', { class: 'btn', href: './' }, 'Back to the decisions'))); return; }
    const c = r.data;
    const waiting = c.rows.filter(x => x.state === 'waiting').length;
    el.replaceChildren(...head('Card', c.title.endsWith('.') ? c.title : c.title + '.'),
      h('div', { class: 'pills' }, h('span', { class: 'tag' }, c.id), c.dry_run ? h('span', { class: 'tag acc' }, 'dry run') : null, h('span', { class: 'tag' }, waiting + ' waiting')),
      h('div', { class: 'glass ticks list', style: false }, ...c.rows.map(x => h('div', { class: 'row' },
        h('div', {}, h('div', { class: 'l' }, x.n + '. ' + x.line), h('div', { class: 'm' }, [x.kind, x.owner, x.node, x.requester && ('to ' + x.requester), until(x.until)].filter(Boolean).join(' · ')),
          h('pre', { class: 'signed' }, x.text)),
        h('div', { class: 'r' }, h('span', { class: 'tag ' + (x.state === 'waiting' ? 'acc' : x.state.startsWith('ended') ? 'bad' : 'good') }, x.state))))),
      h('p', { class: 'note' }, 'The text above is exactly what one passkey signature approves. A session alone never approves.'),
      h('p', {}, waiting ? signLink(c.sign_at, 'Sign with passkey', true) : null, ' ', h('a', { class: 'btn quiet', href: './' }, 'Back to the decisions')));
  };
  const inbox = async () => {
    const r = await host.call('inbox');
    if (stop) return;
    if (!r.ok) { el.replaceChildren(...head('Inbox', 'Approvals.'), h('p', { class: 'note' }, r.status === 503 ? 'The inbox cell is not answering; trying again.' : 'Not open to you.')); return; }
    const rows = r.data;
    const grants = rows.filter(x => x.kind === 'grant').length;
    const bundles = [...new Set(rows.filter(x => x.bundle).map(x => x.bundle))];
    el.replaceChildren(...head('Inbox', 'Every waiting row.'),
      h('div', { class: 'stats' },
        h('div', { class: 'glass ticks stat' }, h('div', { class: 'label' }, h('i', {}), 'Waiting'), h('div', { class: 'num acc' }, rows.length), h('div', { class: 'sub' }, 'rows, before triage')),
        h('div', { class: 'glass stat' }, h('div', { class: 'label' }, h('i', {}), 'Grants'), h('div', { class: 'num' }, grants), h('div', { class: 'sub' }, bundles.length + ' bundle' + (bundles.length === 1 ? '' : 's'))),
        h('div', { class: 'glass stat' }, h('div', { class: 'label' }, h('i', {}), 'Decisions'), h('div', { class: 'num' }, rows.length - grants), h('div', { class: 'sub' }, 'open sheets'))),
      rows.length ? h('div', { class: 'glass list sys' }, ...rows.map(x => h('div', { class: 'row' },
        h('div', {}, h('div', { class: 'l' }, x.line), h('div', { class: 'm' }, [x.title, x.requester && ('to ' + x.requester), x.node, until(x.until)].filter(Boolean).join(' · '), x.dry_run ? h('span', { class: 'tag acc' }, 'dry run') : null)),
        h('div', { class: 'r' }, x.bundle ? h('a', { class: 'btn', href: '?b=' + encodeURIComponent(x.bundle) }, 'Open card') : h('span', { class: 'tag' }, x.kind)))))
        : h('div', { class: 'glass empty sys' }, 'Nothing waits for you.'),
      h('p', {}, h('a', { class: 'btn quiet', href: './' }, 'Back to the decisions')));
  };
  const q = new URLSearchParams(location.search);
  const b = q.get('b'), all = q.get('all');
  const run = async () => {
    if (stop) return;
    if (b) await card(b);
    else if (all || !(await decisions())) await inbox();
    if (!stop) timer = setTimeout(run, b ? 5000 : 15000);
  };
  run();
  return () => { stop = true; clearTimeout(timer); };
}

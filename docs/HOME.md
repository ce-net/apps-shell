# The Home tab and the ce-net browser (apps.ce-net.com/home/)

Slice 1 of repo:ce `docs/CE-REF-OPENERS-AND-HOME-2026-11-18.md` (section 8 B), built 2026-11-18 by foreman/win-2c3d.
Home is the first tab. It draws no panel itself: home.shell@1 (repo:home-shell) says which slots this viewer gets,
each slot is a view module of a cell, every item is a ce-ref drawn as a RefChip, and a click opens it in the Pane
through open.with@1 (repo:open-with). No view holds a URL.

## Who does what

| piece | where | what |
|---|---|---|
| layout | home.shell `Layout{device}` | slot lines of repo:ce-net-net `HOME_SLOTS` the viewer's chain holds |
| offers | `GET /api/layout` | each slot's `when` op asked as the viewer at once (crm.calls today, crm.promises due, approve.inbox Badge, 3 s): a slot with nothing is dropped; order area, urgency, order |
| slots | `site/v/calls-today.js`, `promises-due.js`, `pending.js`, `cells-summary.js`, `links.js` | `GET /api/calls`, `/api/promises`, `/api/inbox`, `/api/cellmap`, `/api/links`, each as the viewer |
| links | state.store `home/links` | rows `{ref, title, note}`, read with apps.shell's collection cap `state-home-links.hex` (`state.home.links.read`) |
| opener | `GET /api/open?r=` | open.with `Open` with a grant wire.ws-door minted for the viewer: Target, Refused{need, ask}, NoOpener, Unknown |
| views | `site/v/ref-text.js`, `ref-record.js`, `ref-raw.js`, `cell.js`, `port.js`, `outside.js`, `mount.js` | ref.view::Text/Record/Raw (moved out of search.js), site.cellmap::Cell/Node, apps.shell::Port/Outside/Mount |
| browser | `site/_k/shell.js`, `ref.js` | `host.open(ref, {mode, where})`, `host.ref.parse/find/show`, Bar, Pane (side on Desk, full on Phone), `/o/<ref>`, back and forward; the bar shows the short ref, Copy gives `ce-ref://` |
| kit | `site/_k/components.css` | RefChip (live, as of, pinned, refused, unknown, outside), Pane, Bar, SlotFrame |

Rights: the Home tab needs `home.shell:open`; each slot its data right (`crm.calls:read`, `crm.promises:read`,
`approve.inbox:read`, `site.cellmap:read`, `home.shell:links`); opening needs `open.with:use` plus the opener's right.
`/api/open` and `/api/ref` answer 403 to a viewer without `open.with:use` and a tab. The only path to an http address is
`handoff` in shell.js, reached only from the Outside view after it named the site and said "outside ce-net".

`tests/views.rs` is the ui-lint: no `href="http`, `window.open`, `location.href`, `fetch(` or `_blank` in `site/v/*.js`.

## Installed (foreman-ops-1)

`~/.local/bin/apps-shell` (previous `apps-shell.bak-20261118-win2c3d.home`), face 8885 = `/home/`; grant
`cell-keys/apps.shell/grant.hex` reissued with GRANT (previous `grant.hex.bak-20261118-win2c3d.home`) plus
`state-home-links.hex`. Gate route `/home/` (repo:ce-fleet 4bfabd85, snapshot `4bfabd85e82a-16c2dd95730f` on debian,
foreman-ops-1, leif-desk). Swap back: copy the `.home` backups over and `ce cell restart apps.shell`.

Proof: repo:open-with `PROOF.md`.

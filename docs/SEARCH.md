# The Search tab (apps.ce-net.com/search/)

One box over everything a viewer may read, in three groups. Built 2026-11-17 by foreman/win-2c3d (Leif 05:54 CET:
the search engine and the global cell search on apps.ce-net.com, private and global).

| group | cell | what | who sees it |
|---|---|---|---|
| Private | search.hybrid `Find` (BM25 + EmbeddingGemma + rerank) | repos, talk log (`words`), mail, goals, rooms, inbox, calls, people | only labels the viewer's own exact grant names (`search.hybrid:read:<label>`, `read:repos`) |
| Network | search.mesh `Mesh` | every node's own index of repos, cells, docs, merged; which machines answered | only with `search.mesh:find` (internal) |
| Public | search.hybrid `Find`, acl `public` | what cells published for everyone (cells.catalog, Trana, twins) | anyone with `search.hybrid:find` |

Each group shows its own 10 (`k=` 1 to 50 per group). For a viewer with private reads, search.hybrid is asked twice:
once as them, and once with `search.hybrid:find` alone (public only). Before that, an owner's private hits filled one
shared top 20 and Public showed 0.

Each row shows its source and machine, and opens where it lives: a cell in `/core/`, a goal, room or inbox item in
`/foreman/`, and a repo or file in this tab's **ce-ref viewer** `/search/?ref=repo:<repo>/<path>@<commit>`. Never a
guessed GitHub link (Leif: references go through our cell reference system). Mail has no screen yet (no link).

## The ce-ref viewer

- `GET /api/ref?r=repo:<repo>/<path>[@<commit>]`: ref.resolve `Resolve` says where the file lives, its
  `ce-ref://repo/<sha256 of origin>/<path>` id and its content hash. ref.pin `Fetch` then hands over only the bytes with
  that hash. Both are asked as the viewer (a grant minted by wire.ws-door; apps.shell holds `mint-for:ref.resolve` and
  `mint-for:ref.pin`), so the viewer needs `ref.resolve:read` and `ref.pin:read`, and a stranger gets 403. Both
  contracts come from repo:live-refs `contracts/`, copied verbatim; schemas `0aa2e2474931699b`/`966357c40f1b9fff` are
  pinned by a test.
- ref.pin reads `git show <commit>:<path>` in `~/dev/<repo>` on foreman-ops-1. So at search time apps.shell keeps a
  viewer link only when that checkout holds the file at HEAD (one `git rev-parse` and one `git cat-file --batch-check`
  per repo), and pins the link to that commit. Otherwise the row opens at its real origin if search.mesh gave one,
  or has no link. A file that is only in search.hybrid's mirror of another machine (a plain copy, no git) is not
  something ref.pin can read.
- Proof 2026-11-17 (scoped test person, ended after): 720 rows over 12 queries, 0 GitHub links. All 205 distinct viewer
  links opened (200) through Debian's gate, which hops to ops-1. Bad refs (`..`, `.git/`, `@HEAD`) are refused with 400
  before any cell is asked; an unknown repo or file gives 404 with ref.resolve's reason. Rollback check: the old
  binary gave private 20, public 0 and 29 GitHub links for `ce-ref`; the new one gives 10/10/10, 0 GitHub links and
  17 viewer links.

## How it is wired

- System `SEARCH` in repo:ce-net-net (5c8a3fd): `tab "Search" route "/search/" order 15 needs "search.hybrid:find"
  shell search.hybrid@1`. The tab is live exactly while search.hybrid answers HealthOp.
- apps.shell (this repo) serves the view `site/v/search.js` on its face `127.0.0.1:8883` and `GET /api/search?q=`.
  The calls go with the viewer's grant minted by wire.ws-door (apps.shell's own grant holds
  `wire.ws-door:mint-for:search.hybrid`, `..:search.mesh`, `search.hybrid:health`), so each search cell's own gate
  filters by the viewer, never by apps.shell. The two contracts are copied verbatim; a test pins their schema hashes.
- Viewer abilities: `~/.config/apps-shell/people.json` on foreman-ops-1 (leif: find, mesh find, the eight reads,
  `ref.resolve:read`, `ref.pin:read`).
- Gate: repo:ce-fleet `PORT_PATHS["search"] = 8883`, a `CELL_FACES` route: a gate whose face port *refuses* hops to
  ops-1 over `CE_GATE_WORK_HOP`. Debian has one since 2026-11-17 (`gate-ops1-hop.service`, repo:ce-fleet
  docs/GATE-DEPLOY.md). Its 502 came from mac-port-forward forwarding :8883 to the Mac, which accepted the connection and
  then dropped it, so the gate never hopped. mac-port-forward now never forwards a cell face's port.

## Proof (foreman-ops-1, scoped test persons, ended after)

- `t-search-owner` (leif's search abilities): private 20 (goals, mail), network 20 (2 of 5 nodes answered), in 0.6-1.8 s.
- `t-search-stranger` (`search.hybrid:find` only): private 0, network not asked, public 20.
- No signed person: 401; a person without `search.hybrid:find`: 403, `v/search.js` 404.

## Still on Debian

- The private sources are made on Debian: its cell.tick `search-hybrid-refresh` writes `ui.jsonl` (goals, rooms,
  inbox, calls, people), `log.jsonl` (words), `mail.jsonl`, and mirrors Debian's, leif-desk's and the Mac's `~/dev` to
  ops-1. With Debian off, search keeps answering from the last index, but nothing private gets newer.
- leif-desk (2 cores, 3 GB) cannot hold search.hybrid; its Search tab hops to ops-1.

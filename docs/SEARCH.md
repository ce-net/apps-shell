# The Search tab (apps.ce-net.com/search/)

One box over everything a viewer may read, in three groups. Built 2026-11-17 by foreman/win-2c3d (Leif 05:54 CET:
the search engine and the global cell search on apps.ce-net.com, private and global).

| group | cell | what | who sees it |
|---|---|---|---|
| Private | search.hybrid `Find` (BM25 + EmbeddingGemma + rerank) | repos, talk log (`words`), mail, goals, rooms, inbox, calls, people | only labels the viewer's own exact grant names (`search.hybrid:read:<label>`, `read:repos`) |
| Network | search.mesh `Mesh` | every node's own index of repos, cells, docs, merged; which machines answered | only with `search.mesh:find` (internal) |
| Public | search.hybrid `Find`, acl `public` | what cells published for everyone (cells.catalog, Trana, twins) | anyone with `search.hybrid:find` |

Each row shows its source and machine and opens where it lives: a cell in `/core/`, a goal, room or inbox item in
`/foreman/`, a repo file on GitHub. Mail has no screen yet (no link).

## How it is wired

- System `SEARCH` in repo:ce-net-net (5c8a3fd): `tab "Search" route "/search/" order 15 needs "search.hybrid:find"
  shell search.hybrid@1`. The tab is live exactly while search.hybrid answers HealthOp.
- apps.shell (this repo) serves the view `site/v/search.js` on its face `127.0.0.1:8883` and `GET /api/search?q=`.
  The calls go with the viewer's grant minted by wire.ws-door (apps.shell's own grant holds
  `wire.ws-door:mint-for:search.hybrid`, `..:search.mesh`, `search.hybrid:health`), so each search cell's own gate
  filters by the viewer, never by apps.shell. The two contracts are copied verbatim; a test pins their schema hashes.
- Viewer abilities: `~/.config/apps-shell/people.json` on foreman-ops-1 (leif: find, mesh find, the eight reads).
- Gate: repo:ce-fleet `PORT_PATHS["search"] = 8883`, a `CELL_FACES` route (a gate without a replica hops to ops-1).

## Proof (foreman-ops-1, scoped test persons, ended after)

- `t-search-owner` (leif's search abilities): private 20 (goals, mail), network 20 (2 of 5 nodes answered), in 0.6-1.8 s.
- `t-search-stranger` (`search.hybrid:find` only): private 0, network not asked, public 20.
- No signed person: 401; a person without `search.hybrid:find`: 403, `v/search.js` 404.

## Still on Debian

- The private sources are made on Debian: its cell.tick `search-hybrid-refresh` writes `ui.jsonl` (goals, rooms,
  inbox, calls, people), `log.jsonl` (words), `mail.jsonl`, and mirrors Debian's, leif-desk's and the Mac's `~/dev` to
  ops-1. With Debian off, search keeps answering from the last index, but nothing private gets newer.
- leif-desk (2 cores, 3 GB) cannot hold search.hybrid; its Search tab hops to ops-1.
